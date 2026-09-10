use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub(super) struct ConnectionLimiter {
    maximum: usize,
    active: AtomicUsize,
    rejected: AtomicU64,
}

impl ConnectionLimiter {
    pub(super) fn new(maximum: usize) -> Self {
        assert!(maximum > 0, "connection admission limit must be non-zero");
        Self {
            maximum,
            active: AtomicUsize::new(0),
            rejected: AtomicU64::new(0),
        }
    }

    pub(super) fn try_acquire(self: &Arc<Self>) -> Option<ConnectionPermit> {
        let acquired = self
            .active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.maximum).then_some(active + 1)
            })
            .is_ok();
        if acquired {
            Some(ConnectionPermit { limiter: self.clone() })
        } else {
            self.rejected.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    #[cfg(test)]
    pub(super) fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(super) fn rejected(&self) -> u64 {
        self.rejected.load(Ordering::Relaxed)
    }
}

pub(super) struct ConnectionPermit {
    limiter: Arc<ConnectionLimiter>,
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        let previous = self.limiter.active.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "connection permit count underflow");
    }
}
