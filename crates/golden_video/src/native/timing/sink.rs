use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use concurrent_queue::ConcurrentQueue;

use super::{Clock, CpuInterval, DiagnosticEvent, DiagnosticsConfig, DiagnosticsSnapshot};

struct Shared {
    rows: ConcurrentQueue<DiagnosticEvent>,
    capacity: usize,
    written: AtomicU64,
    drained: AtomicU64,
    dropped_full: AtomicU64,
    clock_failures: AtomicU64,
    clock: Clock,
}

#[cfg(test)]
mod tests;

/// Preallocated bounded MPMC sink. Reporting cannot hold a metadata lock against a producer;
/// a full queue still drops explicitly instead of waiting for the reporting worker.
#[derive(Clone)]
pub(crate) struct Recorder(Arc<Shared>);

fn increment(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}

impl Recorder {
    pub(crate) fn new(config: DiagnosticsConfig) -> Self {
        assert!(config.capacity.get() <= 65536, "validated diagnostics capacity");
        Self(Arc::new(Shared {
            rows: ConcurrentQueue::bounded(config.capacity.get()),
            capacity: config.capacity.get(),
            written: AtomicU64::new(0),
            drained: AtomicU64::new(0),
            dropped_full: AtomicU64::new(0),
            clock_failures: AtomicU64::new(0),
            clock: Clock::new(),
        }))
    }

    pub(crate) fn push(&self, event: DiagnosticEvent) {
        // This private queue is never closed. Its only rejected-push state is capacity pressure.
        if self.0.rows.push(event).is_err() {
            increment(&self.0.dropped_full);
            return;
        }
        increment(&self.0.written);
    }

    pub(crate) fn stamp(&self) -> Option<u64> {
        let stamp = self.0.clock.stamp();
        if stamp.is_none() {
            increment(&self.0.clock_failures);
        }
        stamp
    }

    pub(crate) fn interval(&self, begin: Option<u64>) -> Option<CpuInterval> {
        begin
            .zip(self.stamp())
            .filter(|(begin, end)| end >= begin)
            .map(|(begin, end)| CpuInterval { begin, end })
    }

    pub(crate) fn drain(&self, output: &mut Vec<DiagnosticEvent>, budget: NonZeroUsize) -> usize {
        let mut count = 0;
        while count < budget.get() {
            let Ok(row) = self.0.rows.pop() else {
                break;
            };
            output.push(row);
            count += 1;
        }
        let _ = self
            .0
            .drained
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_add(count as u64))
            });
        count
    }

    pub(crate) fn snapshot(&self) -> DiagnosticsSnapshot {
        DiagnosticsSnapshot {
            enabled: true,
            capacity: self.0.capacity,
            queued: self.0.rows.len(),
            written: self.0.written.load(Ordering::Relaxed),
            drained: self.0.drained.load(Ordering::Relaxed),
            dropped_full: self.0.dropped_full.load(Ordering::Relaxed),
            clock_failures: self.0.clock_failures.load(Ordering::Relaxed),
            qpc_frequency: self.0.clock.frequency,
        }
    }
}
