use golden_core::engine::NodeWaker;
use golden_io::{DelayedNotifier, WorkerNotifier};
use std::time::Duration;

pub(super) struct UdpWake {
    pub notifier: WorkerNotifier,
    delayed: DelayedNotifier,
    waker: NodeWaker,
}

impl UdpWake {
    pub(super) fn new(waker: NodeWaker) -> Result<Self, String> {
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

    pub(super) fn matches(&self, waker: &NodeWaker) -> bool {
        self.waker == *waker
    }

    pub(super) fn later(&self, delay: Duration) {
        self.delayed.wake_after(delay);
    }
    pub(super) fn cancel_deadline(&self) {
        self.delayed.cancel_deadline();
    }
}
