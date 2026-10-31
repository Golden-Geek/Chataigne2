use golden_transport_server::UiServerShutdown;

#[cfg(test)]
mod tests;
use std::{io, path::PathBuf, sync::mpsc, thread, time::Duration};

pub(crate) struct ShutdownFileWatcher {
    shutdown: UiServerShutdown,
    finished: mpsc::Receiver<io::Result<()>>,
}

impl ShutdownFileWatcher {
    pub(crate) fn start(path: Option<PathBuf>, shutdown: UiServerShutdown) -> io::Result<Option<Self>> {
        let Some(path) = path else { return Ok(None) };
        let (sender, finished) = mpsc::sync_channel(1);
        let stop = shutdown.clone();
        thread::Builder::new()
            .name("golden-headless-shutdown".into())
            .spawn(move || {
                let result = loop {
                    if stop.is_requested() {
                        break Ok(());
                    }
                    match std::fs::metadata(&path) {
                        Ok(_) => {
                            eprintln!("automation shutdown requested by sentinel {}", path.display());
                            break Ok(());
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            stop.wait_timeout(Duration::from_millis(100));
                        }
                        Err(error) => {
                            break Err(io::Error::other(format!(
                                "cannot observe shutdown sentinel {}: {error}",
                                path.display()
                            )));
                        }
                    }
                };
                stop.request();
                let _ = sender.send(result);
            })?;
        Ok(Some(Self { shutdown, finished }))
    }

    pub(crate) fn finish(self) -> io::Result<()> {
        self.shutdown.request();
        self.finished
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "headless shutdown watcher did not stop"))?
    }
}

impl Drop for ShutdownFileWatcher {
    fn drop(&mut self) {
        self.shutdown.request();
    }
}
