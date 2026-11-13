//! Process-local qualification opt-in; no authored setting or render-thread file I/O.
use std::{
    fs::{self, File},
    io::{self, BufWriter},
    num::{NonZeroU64, NonZeroUsize},
    path::PathBuf,
    time::Duration,
};

use golden_video::native::{D3d11Control, DiagnosticCapture, DiagnosticCaptureLimits, DiagnosticsConfig};

pub(super) struct Pending {
    path: PathBuf,
    file: File,
}

pub(super) fn prepare() -> Option<Pending> {
    let directory = std::env::var_os("MAPGYVER_NATIVE_DIAGNOSTICS")?;
    if directory.is_empty() {
        return None;
    }
    let result = (|| -> io::Result<Pending> {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("native-{}-{}.jsonl", std::process::id(), uuid::Uuid::new_v4()));
        let file = File::options().write(true).create_new(true).open(&path)?;
        Ok(Pending { path, file })
    })();
    match result {
        Ok(pending) => Some(pending),
        Err(error) => {
            eprintln!("[mapgyver-diagnostics] capture preparation failed: {error}");
            None
        }
    }
}

impl Pending {
    pub(super) fn config(&self) -> DiagnosticsConfig {
        DiagnosticsConfig {
            capacity: NonZeroUsize::new(16384).unwrap(),
        }
    }
    pub(super) fn start(
        self,
        native: D3d11Control,
        source_budget: Option<golden_video::native::GpuFrameBudget>,
    ) -> Option<Capture> {
        let limits = DiagnosticCaptureLimits {
            max_bytes: NonZeroU64::new(256 * 1024 * 1024).unwrap(),
            max_duration: Duration::from_secs(600),
        };
        match DiagnosticCapture::spawn(native, BufWriter::new(self.file), limits, source_budget) {
            Ok(task) => {
                eprintln!("[mapgyver-diagnostics] capture started: {}", self.path.display());
                Some(Capture { path: self.path, task })
            }
            Err(error) => {
                eprintln!("[mapgyver-diagnostics] capture start failed: {error}");
                None
            }
        }
    }
}

pub(super) struct Capture {
    path: PathBuf,
    task: DiagnosticCapture,
}
impl Capture {
    pub(super) fn finish(self) {
        match self.task.finish() {
            Ok(summary) => eprintln!(
                "[mapgyver-diagnostics] capture finished: {} events={} queued={} dropped={} duration_limit={}",
                self.path.display(),
                summary.events,
                summary.diagnostics.queued,
                summary.diagnostics.dropped_full,
                summary.duration_limit,
            ),
            Err(error) => eprintln!(
                "[mapgyver-diagnostics] capture failed: {}: {error}",
                self.path.display()
            ),
        }
    }
}
