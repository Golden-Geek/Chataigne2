use golden_core::engine::NodeWaker;
use golden_io::{DelayedNotifier, WorkerNotifier};
use std::time::Duration;

pub(crate) struct StreamingWake {
    pub notifier: WorkerNotifier,
    delayed: DelayedNotifier,
    waker: NodeWaker,
}

impl StreamingWake {
    pub(crate) fn new(waker: NodeWaker) -> Result<Self, String> {
        let target = waker.clone();
        let notifier = WorkerNotifier::new(move || {
            let _ = target.wake();
        });
        let delayed = DelayedNotifier::new(notifier.clone())?;
        Ok(Self {
            notifier,
            delayed,
            waker,
        })
    }

    pub(crate) fn matches(&self, waker: &NodeWaker) -> bool {
        self.waker == *waker
    }

    pub(crate) fn later(&self, delay: Duration) {
        self.delayed.wake_after(delay);
    }
    pub(crate) fn cancel_deadline(&self) {
        self.delayed.cancel_deadline();
    }
}
