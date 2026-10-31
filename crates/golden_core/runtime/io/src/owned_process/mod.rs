//! Owned executable trees. Create, observe, wait and release these on an IO/background owner.
//! No shell, process-name adoption, elevation fallback, readiness or restart policy is provided.
use std::{
    ffi::OsString,
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
use unsupported as platform;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProcessWindow {
    #[default]
    Hidden,
    Normal,
}

/// Explicit executable launch; arguments follow the Windows CRT/Rust argv convention.
#[derive(Clone, Debug)]
pub struct ProcessSpec {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub directory: Option<PathBuf>,
    /// None inherits the environment; Some replaces it, including Some(empty).
    pub environment: Option<Vec<(OsString, OsString)>>,
    pub window: ProcessWindow,
}
impl ProcessSpec {
    /// Validate launch encoding and bounds without creating a process or accessing an executable.
    pub fn validate(&self) -> io::Result<()> {
        platform::validate(self)
    }
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            arguments: vec![],
            directory: None,
            environment: None,
            window: ProcessWindow::Hidden,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootProcessState {
    Running,
    Exited(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessSnapshot {
    pub root_id: u32,
    pub root: RootProcessState,
    pub active_processes: u32,
    /// Process lifetimes reported by the job but never captured by an owned process handle.
    pub unobserved_processes: u32,
    /// Captured descendant handles that have not signalled completion yet.
    pub pending_observed_processes: u32,
}
impl ProcessSnapshot {
    pub fn tree_exited(self) -> bool {
        self.active_processes == 0
            && self.unobserved_processes == 0
            && self.pending_observed_processes == 0
            && matches!(self.root, RootProcessState::Exited(_))
    }
}

/// A suspended primary thread whose entire descendant tree is already in a private kill-on-close job.
/// Dropping this value cancels execution; resume consumes it, preserving the same ownership.
pub struct SuspendedProcess(OwnedProcess);
impl SuspendedProcess {
    pub fn create(spec: &ProcessSpec) -> io::Result<Self> {
        platform::Process::create(spec).map(|process| Self(OwnedProcess(process)))
    }
    pub fn id(&self) -> u32 {
        self.0.id()
    }
    pub fn snapshot(&self) -> io::Result<ProcessSnapshot> {
        self.0.snapshot()
    }
    pub fn resume(self) -> io::Result<OwnedProcess> {
        let mut process = self.0;
        process.0.resume()?;
        Ok(process)
    }
    pub fn terminate(&self, exit_code: u32) -> io::Result<()> {
        self.0.terminate(exit_code)
    }
    pub fn wait_tree(&self, timeout: Duration) -> io::Result<bool> {
        self.0.wait_tree(timeout)
    }
}

/// Sole ownership of a launched process tree, not proof that an app/sender is ready.
/// Drop requests tree termination without waiting. Keep the owner and use wait_tree to prove retirement.
pub struct OwnedProcess(platform::Process);
impl OwnedProcess {
    pub fn id(&self) -> u32 {
        self.0.id()
    }
    pub fn snapshot(&self) -> io::Result<ProcessSnapshot> {
        self.0.snapshot()
    }
    /// Post nonblocking WM_CLOSE requests to owned top-level windows only. The count is requests,
    /// not acknowledgements. Headless/console apps can have no windows and need an app-owned signal.
    pub fn request_close(&self) -> io::Result<u32> {
        self.0.request_close()
    }
    pub fn terminate(&self, exit_code: u32) -> io::Result<()> {
        self.0.terminate(exit_code)
    }
    /// Worker-only bounded wait. A missed transient descendant leaves retirement unverified rather
    /// than converting job accounting into a process-completion barrier. Snapshot exposes that gap.
    pub fn wait_tree(&self, timeout: Duration) -> io::Result<bool> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "process wait deadline overflow"))?;
        loop {
            if self.snapshot()?.tree_exited() {
                return Ok(true);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(false);
            }
            std::thread::sleep((deadline - now).min(Duration::from_millis(10)));
        }
    }
}
