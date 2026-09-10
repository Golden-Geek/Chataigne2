use std::{
    io,
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
    thread::{self, JoinHandle},
};

const DEFAULT_COMMAND_CAPACITY: usize = 256;

/// Owns a named IO worker thread and its command channel.
pub struct WorkerTask<C> {
    commands: SyncSender<C>,
    worker: Option<JoinHandle<()>>,
}

impl<C: Send + 'static> WorkerTask<C> {
    pub fn spawn<F>(name: impl Into<String>, run: F) -> io::Result<Self>
    where
        F: FnOnce(Receiver<C>) + Send + 'static,
    {
        Self::spawn_with_capacity(name, DEFAULT_COMMAND_CAPACITY, run)
    }

    /// Starts a worker with an explicit bounded command capacity.
    pub fn spawn_with_capacity<F>(name: impl Into<String>, command_capacity: usize, run: F) -> io::Result<Self>
    where
        F: FnOnce(Receiver<C>) + Send + 'static,
    {
        assert!(command_capacity > 0, "worker command capacity must be non-zero");
        let (commands, receiver) = mpsc::sync_channel(command_capacity);
        let worker = thread::Builder::new().name(name.into()).spawn(move || run(receiver))?;
        Ok(Self {
            commands,
            worker: Some(worker),
        })
    }

    /// Attempts to admit a command without blocking the caller.
    pub fn send(&self, command: C) -> Result<(), TrySendError<C>> {
        self.commands.try_send(command)
    }

    /// Requests an orderly stop and joins the worker exactly once.
    pub fn stop(&mut self, command: C) {
        let _ = self.commands.send(command);
        self.join();
    }

    pub fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    /// Disconnects command admission and returns the worker for retirement outside the caller.
    pub fn into_join_handle(self) -> Option<JoinHandle<()>> {
        let Self { commands, worker } = self;
        drop(commands);
        worker
    }

    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }
}
