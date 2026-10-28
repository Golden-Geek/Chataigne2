//! Bounded ownership for this harness's direct children; fixtures never spawn descendants.

use std::io::Write;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::ProbeResult;
mod reported;
pub use reported::ReportedProcess;

pub struct OwnedProbeProcess {
    pub child: Child,
    stop_line: Vec<u8>,
    pub forced_stop: bool,
}

impl OwnedProbeProcess {
    pub fn spawn(command: &mut Command, stop_line: Vec<u8>) -> ProbeResult<Self> {
        if stop_line.len() > 256 {
            return Err("oversized probe stop command".into());
        }
        let child = command.stdin(Stdio::piped()).spawn()?;
        Ok(Self {
            child,
            stop_line,
            forced_stop: false,
        })
    }

    #[cfg(feature = "desktop")]
    pub fn send(&mut self, line: &[u8]) -> ProbeResult<()> {
        if line.len() > 256 {
            return Err("oversized probe control command".into());
        }
        self.child
            .stdin
            .as_mut()
            .ok_or("probe control pipe is closed")?
            .write_all(line)?;
        Ok(())
    }

    pub fn stop(&mut self) -> ProbeResult<ExitStatus> {
        self.request_stop();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.forced_stop = true;
        self.child.kill()?;
        Ok(self.child.wait()?)
    }

    /// Close the command stream without waiting, so owners can drain bounded output concurrently.
    pub fn request_stop(&mut self) {
        if let Some(mut control) = self.child.stdin.take() {
            let _ = control.write_all(&self.stop_line);
        }
    }
}

impl Drop for OwnedProbeProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
