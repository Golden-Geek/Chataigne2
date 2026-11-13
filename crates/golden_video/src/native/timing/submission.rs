//! Restricted source-worker access to the existing bounded diagnostic queue.
#[cfg(any(feature = "ffmpeg", test))]
use super::{DiagnosticEvent, SourceSubmissionTrace};
use super::{DiagnosticsSnapshot, Recorder};

/// Metadata-only sink. It retains no renderer, source image or native device.
/// Full queues drop rows and increment the same diagnostic health counters as renderer rows.
#[derive(Clone)]
pub struct SourceSubmissionSink(pub(crate) Recorder);

impl SourceSubmissionSink {
    /// Shared queue health, including source/renderer losses after the control owner retires.
    pub fn diagnostics_snapshot(&self) -> DiagnosticsSnapshot {
        self.0.snapshot()
    }

    #[cfg(any(feature = "ffmpeg", test))]
    pub(crate) fn record(&self, mut row: SourceSubmissionTrace) {
        row.observed_qpc = self.0.stamp();
        self.0.push(DiagnosticEvent::SourceSubmission(row));
    }
}
