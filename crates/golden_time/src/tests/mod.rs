use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{Clock, MonotonicTime, TimeError};

#[derive(Clone)]
pub(crate) struct ManualClock(Arc<State>);

struct State {
    now: AtomicU64,
    fail: AtomicBool,
}

impl ManualClock {
    pub(crate) fn new(nanos: u64) -> Self {
        Self(Arc::new(State {
            now: AtomicU64::new(nanos),
            fail: AtomicBool::new(false),
        }))
    }

    pub(crate) fn set(&self, nanos: u64) {
        self.0.now.store(nanos, Ordering::Release);
    }

    pub(crate) fn fail(&self, failed: bool) {
        self.0.fail.store(failed, Ordering::Release);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        if self.0.fail.load(Ordering::Acquire) {
            return Err(TimeError::Overflow);
        }
        Ok(MonotonicTime::from_nanos(self.0.now.load(Ordering::Acquire)))
    }
}
