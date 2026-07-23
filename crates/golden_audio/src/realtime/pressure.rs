use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueuePressureSnapshot {
    pub command_full: u64,
    pub plan_publish_full: u64,
    pub plan_return_full: u64,
    pub realtime_control_full: u64,
    pub voice_return_full: u64,
    pub analysis_free_empty: u64,
    pub analysis_ready_full: u64,
}

#[derive(Clone, Debug, Default)]
pub struct QueuePressureCounters {
    inner: Arc<QueuePressureAtoms>,
}

#[derive(Debug, Default)]
struct QueuePressureAtoms {
    command_full: AtomicU64,
    plan_publish_full: AtomicU64,
    plan_return_full: AtomicU64,
    realtime_control_full: AtomicU64,
    voice_return_full: AtomicU64,
    analysis_free_empty: AtomicU64,
    analysis_ready_full: AtomicU64,
}

impl QueuePressureCounters {
    #[must_use]
    pub fn snapshot(&self) -> QueuePressureSnapshot {
        QueuePressureSnapshot {
            command_full: self.inner.command_full.load(Ordering::Relaxed),
            plan_publish_full: self.inner.plan_publish_full.load(Ordering::Relaxed),
            plan_return_full: self.inner.plan_return_full.load(Ordering::Relaxed),
            realtime_control_full: self.inner.realtime_control_full.load(Ordering::Relaxed),
            voice_return_full: self.inner.voice_return_full.load(Ordering::Relaxed),
            analysis_free_empty: self.inner.analysis_free_empty.load(Ordering::Relaxed),
            analysis_ready_full: self.inner.analysis_ready_full.load(Ordering::Relaxed),
        }
    }

    #[inline]
    pub(crate) fn command_full(&self) {
        increment(&self.inner.command_full);
    }

    #[inline]
    pub(crate) fn plan_publish_full(&self) {
        increment(&self.inner.plan_publish_full);
    }

    #[inline]
    pub(crate) fn plan_return_full(&self) {
        increment(&self.inner.plan_return_full);
    }

    #[inline]
    pub(crate) fn realtime_control_full(&self) {
        increment(&self.inner.realtime_control_full);
    }

    #[inline]
    pub(crate) fn voice_return_full(&self) {
        increment(&self.inner.voice_return_full);
    }

    #[inline]
    pub(crate) fn analysis_free_empty(&self) {
        increment(&self.inner.analysis_free_empty);
    }

    #[inline]
    pub(crate) fn analysis_ready_full(&self) {
        increment(&self.inner.analysis_ready_full);
    }
}

#[inline]
fn increment(value: &AtomicU64) {
    value.fetch_add(1, Ordering::Relaxed);
}
