use std::sync::atomic::{AtomicU64, Ordering};

/// Lock-free counters shared by the control, compiler, scheduler, and effect planes.
#[derive(Debug, Default)]
pub struct RuntimeMetrics {
    control_received: AtomicU64,
    control_applied: AtomicU64,
    control_rejected: AtomicU64,
    control_queue_depth: AtomicU64,
    control_queue_peak: AtomicU64,
    control_wait_ns: AtomicU64,
    control_apply_ns: AtomicU64,
    compilation_requested: AtomicU64,
    compilation_applied: AtomicU64,
    compilation_rejected: AtomicU64,
    generation_id: AtomicU64,
    sparse_batches: AtomicU64,
    dense_batches: AtomicU64,
    work_units: AtomicU64,
    effects_committed: AtomicU64,
    effects_suppressed: AtomicU64,
    shadow_comparisons: AtomicU64,
    shadow_mismatches: AtomicU64,
}

/// One consistent-enough diagnostics sample of monotonic runtime counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeMetricsSnapshot {
    /// Control messages received.
    pub control_received: u64,
    /// Control messages successfully applied.
    pub control_applied: u64,
    /// Control messages rejected before application.
    pub control_rejected: u64,
    /// Current control queue depth.
    pub control_queue_depth: u64,
    /// Peak observed control queue depth.
    pub control_queue_peak: u64,
    /// Cumulative control queue wait time.
    pub control_wait_ns: u64,
    /// Cumulative control application time.
    pub control_apply_ns: u64,
    /// Compile requests admitted.
    pub compilation_requested: u64,
    /// Compiled generations accepted.
    pub compilation_applied: u64,
    /// Compile requests that failed.
    pub compilation_rejected: u64,
    /// Current published generation id.
    pub generation_id: u64,
    /// Sparse scheduler batches completed.
    pub sparse_batches: u64,
    /// Dense scheduler batches completed.
    pub dense_batches: u64,
    /// Scheduled work units completed.
    pub work_units: u64,
    /// Authoritative effects committed.
    pub effects_committed: u64,
    /// Shadow effects suppressed.
    pub effects_suppressed: u64,
    /// Authoritative results compared with the side-effect-free runtime plan.
    pub shadow_comparisons: u64,
    /// Semantic mismatches found by safe shadow comparison.
    pub shadow_mismatches: u64,
}

impl RuntimeMetrics {
    pub(crate) fn control_received(&self) {
        self.control_received.fetch_add(1, Ordering::Relaxed);
        let depth = self.control_queue_depth.fetch_add(1, Ordering::Relaxed) + 1;
        self.control_queue_peak.fetch_max(depth, Ordering::Relaxed);
    }

    pub(crate) fn control_started(&self, wait_ns: u64) {
        self.control_queue_depth.fetch_sub(1, Ordering::Relaxed);
        self.control_wait_ns.fetch_add(wait_ns, Ordering::Relaxed);
    }

    pub(crate) fn control_finished(&self, apply_ns: u64) {
        self.control_applied.fetch_add(1, Ordering::Relaxed);
        self.control_apply_ns.fetch_add(apply_ns, Ordering::Relaxed);
    }

    pub(crate) fn control_rejected(&self) {
        self.control_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn compilation_requested(&self) {
        self.compilation_requested.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn compilation_finished(&self, accepted: bool, generation_id: Option<u64>) {
        if accepted {
            self.compilation_applied.fetch_add(1, Ordering::Relaxed);
            if let Some(generation_id) = generation_id {
                self.generation_id.store(generation_id, Ordering::Release);
            }
        } else {
            self.compilation_rejected.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn batch_finished(&self, dense: bool, work_units: usize) {
        let counter = if dense {
            &self.dense_batches
        } else {
            &self.sparse_batches
        };
        counter.fetch_add(1, Ordering::Relaxed);
        self.work_units.fetch_add(work_units as u64, Ordering::Relaxed);
    }

    pub(crate) fn effects_finished(&self, committed: usize, suppressed: usize) {
        self.effects_committed.fetch_add(committed as u64, Ordering::Relaxed);
        self.effects_suppressed.fetch_add(suppressed as u64, Ordering::Relaxed);
    }

    /// Records side-effect-free semantic comparison results from a production adapter.
    pub fn shadow_compared(&self, comparisons: usize, mismatches: usize) {
        self.shadow_comparisons.fetch_add(comparisons as u64, Ordering::Relaxed);
        self.shadow_mismatches.fetch_add(mismatches as u64, Ordering::Relaxed);
    }

    /// Captures the current metrics without blocking runtime work.
    pub fn snapshot(&self) -> RuntimeMetricsSnapshot {
        RuntimeMetricsSnapshot {
            control_received: self.control_received.load(Ordering::Relaxed),
            control_applied: self.control_applied.load(Ordering::Relaxed),
            control_rejected: self.control_rejected.load(Ordering::Relaxed),
            control_queue_depth: self.control_queue_depth.load(Ordering::Relaxed),
            control_queue_peak: self.control_queue_peak.load(Ordering::Relaxed),
            control_wait_ns: self.control_wait_ns.load(Ordering::Relaxed),
            control_apply_ns: self.control_apply_ns.load(Ordering::Relaxed),
            compilation_requested: self.compilation_requested.load(Ordering::Relaxed),
            compilation_applied: self.compilation_applied.load(Ordering::Relaxed),
            compilation_rejected: self.compilation_rejected.load(Ordering::Relaxed),
            generation_id: self.generation_id.load(Ordering::Acquire),
            sparse_batches: self.sparse_batches.load(Ordering::Relaxed),
            dense_batches: self.dense_batches.load(Ordering::Relaxed),
            work_units: self.work_units.load(Ordering::Relaxed),
            effects_committed: self.effects_committed.load(Ordering::Relaxed),
            effects_suppressed: self.effects_suppressed.load(Ordering::Relaxed),
            shadow_comparisons: self.shadow_comparisons.load(Ordering::Relaxed),
            shadow_mismatches: self.shadow_mismatches.load(Ordering::Relaxed),
        }
    }
}
