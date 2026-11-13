//! Bounded metadata capture on a dedicated I/O owner, never a rendering or UI worker.
use std::{
    io::{self, Write},
    num::{NonZeroU64, NonZeroUsize},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::Serialize;

use super::{D3d11Control, DiagnosticEvent, DiagnosticsSnapshot, GpuFrameBudget, timing::Clock};

/// Hard storage and elapsed-time bounds for one explicitly enabled capture.
#[derive(Clone, Copy, Debug)]
pub struct DiagnosticCaptureLimits {
    /// Maximum bytes accepted by the writer, including headers and the final record.
    pub max_bytes: NonZeroU64,
    /// Maximum capture duration; a zero duration is rejected.
    pub max_duration: Duration,
}

/// Terminal metadata. Ring loss remains explicit and prevents complete-coverage claims.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct DiagnosticCaptureSummary {
    /// Number of complete diagnostic event records written.
    pub events: u64,
    /// Number of sparse process/native snapshots written.
    pub snapshots: u64,
    /// Whether the duration ceiling ended capture before its owner requested completion.
    pub duration_limit: bool,
    /// Final ring health, including drops and undrained rows.
    pub diagnostics: DiagnosticsSnapshot,
}

/// Explicitly owned diagnostic I/O task. This metadata reader owns no native resources.
/// Finish/drop on an outer I/O host after rendering stops, never on a render/UI thread.
pub struct DiagnosticCapture {
    stop: mpsc::Sender<()>,
    worker: Option<JoinHandle<io::Result<DiagnosticCaptureSummary>>>,
}

impl DiagnosticCapture {
    /// Start a bounded JSONL capture using the existing diagnostic event envelope.
    /// The caller enables native diagnostics separately and owns output destination policy.
    /// A byte ceiling or partial record leaves an incomplete trace. Inspect `finish` as well
    /// as the final record: a failed flush cannot certify successful output persistence.
    pub fn spawn(
        control: D3d11Control,
        writer: impl Write + Send + 'static,
        limits: DiagnosticCaptureLimits,
        source_budget: Option<GpuFrameBudget>,
    ) -> io::Result<Self> {
        if limits.max_duration.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "capture duration must be nonzero",
            ));
        }
        let (stop, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("video-diagnostic-capture".into())
            .spawn(move || record(control, writer, limits, receiver, source_budget))?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }

    /// Stop, perform a bounded final drain and flush, and inspect the actual I/O outcome.
    /// Producers must be stopped first to establish a complete final ring observation.
    pub fn finish(mut self) -> io::Result<DiagnosticCaptureSummary> {
        self.join()
    }

    fn join(&mut self) -> io::Result<DiagnosticCaptureSummary> {
        let _ = self.stop.send(());
        self.worker
            .take()
            .expect("capture owner joined once")
            .join()
            .map_err(|_| io::Error::other("diagnostic capture worker panicked"))?
    }
}

impl Drop for DiagnosticCapture {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.join();
        }
    }
}

struct LimitedWriter<W> {
    writer: W,
    remaining: u64,
}
impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(io::Error::other("diagnostic capture byte limit reached"));
        }
        let written = self.writer.write(bytes)?;
        self.remaining -= written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn row(writer: &mut impl Write, value: impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, &value)?;
    writer.write_all(b"\n")
}

fn record(
    control: D3d11Control,
    writer: impl Write,
    limits: DiagnosticCaptureLimits,
    stop: mpsc::Receiver<()>,
    source_budget: Option<GpuFrameBudget>,
) -> io::Result<DiagnosticCaptureSummary> {
    let mut writer = LimitedWriter {
        writer,
        remaining: limits.max_bytes.get(),
    };
    let clock = Clock::new();
    row(
        &mut writer,
        serde_json::json!({
            "kind":"capture", "format_version":1, "process_id":std::process::id(),
            "qpc_frequency":clock.frequency, "begin_qpc":clock.stamp(),
            "max_bytes":limits.max_bytes, "max_duration_seconds":limits.max_duration.as_secs_f64(),
            "qualification_budgets_evaluated":false
        }),
    )?;
    let begin = Instant::now();
    let mut snapshot_at = begin;
    let mut rows = Vec::with_capacity(4096);
    let mut summary = DiagnosticCaptureSummary {
        events: 0,
        snapshots: 0,
        duration_limit: false,
        diagnostics: control.diagnostics_snapshot(),
    };
    loop {
        drain(&control, &mut writer, &mut rows, &mut summary)?;
        if Instant::now() >= snapshot_at {
            snapshot(&control, &mut writer, clock, source_budget.as_ref())?;
            summary.snapshots += 1;
            snapshot_at = Instant::now() + Duration::from_secs(1);
        }
        if begin.elapsed() >= limits.max_duration {
            summary.duration_limit = true;
            break;
        }
        match stop.recv_timeout(Duration::from_millis(25)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    // The ring itself has a fixed maximum; never chase an indefinitely active producer.
    let capacity = control.diagnostics_snapshot().capacity;
    for _ in 0..capacity.div_ceil(4096) {
        if drain(&control, &mut writer, &mut rows, &mut summary)? == 0 {
            break;
        }
    }
    snapshot(&control, &mut writer, clock, source_budget.as_ref())?;
    summary.snapshots += 1;
    summary.diagnostics = control.diagnostics_snapshot();
    row(
        &mut writer,
        serde_json::json!({"kind":"capture_end", "qpc":clock.stamp(), "summary":summary}),
    )?;
    writer.flush()?;
    Ok(summary)
}

fn drain(
    control: &D3d11Control,
    writer: &mut impl Write,
    rows: &mut Vec<DiagnosticEvent>,
    summary: &mut DiagnosticCaptureSummary,
) -> io::Result<usize> {
    rows.clear();
    let count = control.drain_diagnostics(rows, NonZeroUsize::new(4096).unwrap());
    for event in &*rows {
        #[derive(Serialize)]
        struct EventRow<'a> {
            kind: &'static str,
            event: &'a DiagnosticEvent,
        }
        row(
            writer,
            EventRow {
                kind: "diagnostic",
                event,
            },
        )?;
        summary.events += 1;
    }
    Ok(count)
}

fn snapshot(
    control: &D3d11Control,
    writer: &mut impl Write,
    clock: Clock,
    source_budget: Option<&GpuFrameBudget>,
) -> io::Result<()> {
    let native = control.snapshot();
    let memory = native.adapter.map(super::query_adapter_memory).transpose();
    let (memory, memory_error) = match memory {
        Ok(value) => (value, None),
        Err(error) => (None, Some(error.to_string())),
    };
    let (process, process_error) = match super::query_process_resources(std::process::id()) {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(error.to_string())),
    };
    row(
        writer,
        serde_json::json!({
            "kind":"snapshot", "qpc":clock.stamp(), "native":native,
            "outputs":control.output_snapshots(), "allocations":control.allocations_snapshot(),
            "diagnostics":control.diagnostics_snapshot(),
            "source_gpu":source_budget.map(GpuFrameBudget::snapshot),
            "process":process, "process_error":process_error,
            "adapter_memory":memory, "adapter_memory_error":memory_error
        }),
    )
}

#[cfg(test)]
mod tests;
