use std::{
    io,
    sync::mpsc::{self, Receiver, SendError},
    thread::{self, JoinHandle},
};

/// Owns a named IO worker thread and its command channel.
pub struct WorkerTask<C> {
    commands: mpsc::Sender<C>,
    worker: Option<JoinHandle<()>>,
}

impl<C: Send + 'static> WorkerTask<C> {
    pub fn spawn<F>(name: impl Into<String>, run: F) -> io::Result<Self>
    where
        F: FnOnce(Receiver<C>) + Send + 'static,
    {
        let (commands, receiver) = mpsc::channel();
        let worker = thread::Builder::new().name(name.into()).spawn(move || run(receiver))?;
        Ok(Self {
            commands,
            worker: Some(worker),
        })
    }

    pub fn send(&self, command: C) -> Result<(), SendError<C>> {
        self.commands.send(command)
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

    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }
}
