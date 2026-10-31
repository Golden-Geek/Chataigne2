use std::{
    collections::HashMap,
    io,
    net::{Shutdown, TcpStream},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle, ThreadId},
    time::{Duration, Instant},
};
use tokio::sync::watch;

/// Shared, one-way cancellation for built-in UI server instances and their host watchers.
#[derive(Clone)]
pub struct UiServerShutdown(Arc<Signal>);

struct Signal {
    requested: AtomicBool,
    gate: Mutex<()>,
    changed: Condvar,
    event: watch::Sender<()>,
}

impl Default for UiServerShutdown {
    fn default() -> Self {
        Self(Arc::new(Signal {
            requested: AtomicBool::new(false),
            gate: Mutex::new(()),
            changed: Condvar::new(),
            event: watch::channel(()).0,
        }))
    }
}

impl UiServerShutdown {
    /// Request stopping without blocking on listeners, sockets, workers or domain callbacks.
    pub fn request(&self) {
        let _gate = self.0.gate.lock().unwrap();
        if !self.0.requested.swap(true, Ordering::AcqRel) {
            self.0.event.send_replace(());
            self.0.changed.notify_all();
        }
    }

    /// Whether cancellation has been requested; a stopped signal cannot be reset.
    pub fn is_requested(&self) -> bool {
        self.0.requested.load(Ordering::Acquire)
    }

    /// Wait for cancellation or a timeout, returning true when cancellation was requested.
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let gate = self.0.gate.lock().unwrap();
        let _wait = self
            .0
            .changed
            .wait_timeout_while(gate, timeout, |_| !self.is_requested())
            .unwrap();
        self.is_requested()
    }

    pub(super) async fn cancelled(&self) {
        let mut receiver = self.0.event.subscribe();
        while !self.is_requested() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct Connections {
    shutdown: UiServerShutdown,
    streams: Arc<Mutex<HashMap<ThreadId, TcpStream>>>,
}

impl Connections {
    fn new(shutdown: UiServerShutdown) -> Self {
        Self {
            shutdown,
            streams: Arc::default(),
        }
    }

    pub(super) fn track(&self, stream: &TcpStream) -> io::Result<Connection> {
        let mut streams = self.streams.lock().unwrap();
        if self.shutdown.is_requested() {
            let _ = stream.shutdown(Shutdown::Both);
            return Err(io::Error::new(io::ErrorKind::Interrupted, "UI server is stopping"));
        }
        let id = thread::current().id();
        streams.insert(id, stream.try_clone()?);
        Ok(Connection {
            connections: self.clone(),
            id,
        })
    }

    fn close(&self) {
        for stream in self.streams.lock().unwrap().values() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

pub(super) struct Connection {
    connections: Connections,
    id: ThreadId,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.connections.streams.lock().unwrap().remove(&self.id);
    }
}

pub(super) struct ServerWorkers {
    pub(super) shutdown: UiServerShutdown,
    pub(super) connections: Connections,
    pub(super) hub: Option<std::sync::mpsc::SyncSender<super::WsHubCommand>>,
    threads: Vec<JoinHandle<()>>,
}

impl ServerWorkers {
    pub(super) fn new(shutdown: UiServerShutdown) -> Self {
        Self {
            connections: Connections::new(shutdown.clone()),
            shutdown,
            hub: None,
            threads: Vec::new(),
        }
    }

    pub(super) fn spawn(&mut self, name: &str, task: impl FnOnce() + Send + 'static) -> io::Result<()> {
        self.reap()?;
        self.threads.push(thread::Builder::new().name(name.into()).spawn(task)?);
        Ok(())
    }

    fn reap(&mut self) -> io::Result<()> {
        let mut index = 0;
        while index < self.threads.len() {
            if self.threads[index].is_finished() {
                self.threads
                    .swap_remove(index)
                    .join()
                    .map_err(|_| io::Error::other("UI server worker panicked"))?;
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    fn stop(&self) {
        self.shutdown.request();
        self.connections.close();
        if let Some(hub) = &self.hub {
            let _ = hub.try_send(super::WsHubCommand::Shutdown);
        }
    }

    pub(super) fn finish(&mut self, timeout: Duration) -> io::Result<()> {
        self.stop();
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "UI shutdown timeout exceeds clock range"))?;
        loop {
            self.reap()?;
            if self.threads.is_empty() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "UI server workers did not stop before the deadline",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Drop for ServerWorkers {
    fn drop(&mut self) {
        self.stop();
    }
}
