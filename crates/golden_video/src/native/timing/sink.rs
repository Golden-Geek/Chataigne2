use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, TryLockError};

use super::{Clock, CpuInterval, DiagnosticEvent, DiagnosticsConfig, DiagnosticsSnapshot};

struct Shared {
    rows: Mutex<VecDeque<DiagnosticEvent>>,
    capacity: usize,
    written: AtomicU64,
    drained: AtomicU64,
    dropped_full: AtomicU64,
    dropped_contended: AtomicU64,
    clock_failures: AtomicU64,
    clock: Clock,
}

#[cfg(test)]
mod tests;

/// Fixed-size sink shared only by native workers and an explicit diagnostics consumer.
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
            rows: Mutex::new(VecDeque::with_capacity(config.capacity.get())),
            capacity: config.capacity.get(),
            written: AtomicU64::new(0),
            drained: AtomicU64::new(0),
            dropped_full: AtomicU64::new(0),
            dropped_contended: AtomicU64::new(0),
            clock_failures: AtomicU64::new(0),
            clock: Clock::new(),
        }))
    }

    pub(crate) fn push(&self, event: DiagnosticEvent) {
        let mut rows = match self.0.rows.try_lock() {
            Ok(rows) => rows,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => {
                increment(&self.0.dropped_contended);
                return;
            }
        };
        if rows.len() == self.0.capacity {
            increment(&self.0.dropped_full);
            return;
        }
        rows.push_back(event);
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
        let mut rows = self.0.rows.lock().unwrap_or_else(|error| error.into_inner());
        let count = rows.len().min(budget.get());
        output.extend(rows.drain(..count));
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
            queued: self.0.rows.lock().unwrap_or_else(|error| error.into_inner()).len(),
            written: self.0.written.load(Ordering::Relaxed),
            drained: self.0.drained.load(Ordering::Relaxed),
            dropped_full: self.0.dropped_full.load(Ordering::Relaxed),
            dropped_contended: self.0.dropped_contended.load(Ordering::Relaxed),
            clock_failures: self.0.clock_failures.load(Ordering::Relaxed),
            qpc_frequency: self.0.clock.frequency,
        }
    }
}
