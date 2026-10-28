//! A bounded reader shared by fixture and observer owners. Disconnect before child stop/join.

use super::OwnedProbeProcess;
use crate::{ProbeResult, wire::read_message};
use std::io::BufReader;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::JoinHandle;
use std::time::Duration;

pub struct ReportedProcess<R> {
    pub owned: OwnedProbeProcess,
    messages: Option<Receiver<Result<R, String>>>,
    reader: Option<JoinHandle<()>>,
}

impl<R: serde::de::DeserializeOwned + Send + 'static> ReportedProcess<R> {
    pub fn spawn(command: &mut Command, stop_line: Vec<u8>) -> ProbeResult<Self> {
        let mut owned = OwnedProbeProcess::spawn(command.stdout(Stdio::piped()).stderr(Stdio::inherit()), stop_line)?;
        let output = owned.child.stdout.take().ok_or("child report pipe absent")?;
        let (sender, messages) = mpsc::sync_channel(8);
        let reader = std::thread::Builder::new()
            .name("probe-child-report".into())
            .spawn(move || {
                let mut output = BufReader::new(output);
                loop {
                    match read_message(&mut output) {
                        Ok(Some(message)) => {
                            if sender.send(Ok(message)).is_err() {
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            let _ = sender.send(Err(error.to_string()));
                            break;
                        }
                    }
                }
            })?;
        Ok(Self {
            owned,
            messages: Some(messages),
            reader: Some(reader),
        })
    }

    pub fn child(&self) -> &Child {
        &self.owned.child
    }
    pub fn child_mut(&mut self) -> &mut Child {
        &mut self.owned.child
    }
    pub fn receive(&self, timeout: Duration) -> ProbeResult<R> {
        Ok(self
            .messages
            .as_ref()
            .ok_or("child report receiver closed")?
            .recv_timeout(timeout)??)
    }
    pub fn try_receive(&self) -> Result<Result<R, String>, TryRecvError> {
        self.messages
            .as_ref()
            .expect("report receiver remains open until Drop")
            .try_recv()
    }
    /// Call only after the terminal response has been consumed and the child has exited.
    pub fn join_reader(&mut self) -> ProbeResult<()> {
        if let Some(reader) = self.reader.take() {
            reader.join().map_err(|_| "child reader panicked")?;
        }
        Ok(())
    }
}

impl<R> Drop for ReportedProcess<R> {
    fn drop(&mut self) {
        self.messages.take();
        let _ = self.owned.stop();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
