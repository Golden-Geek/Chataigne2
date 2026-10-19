use std::time::Duration;

use golden_core::engine::NodeWaker;
use golden_io::{DelayedNotifier, WorkerNotifier};

#[derive(Clone, Debug)]
pub(crate) struct RuntimeWakeSender {
    waker: NodeWaker,
    delayed: DelayedNotifier,
}

impl RuntimeWakeSender {
    pub(crate) fn new(waker: NodeWaker) -> Result<Self, String> {
        let target = waker.clone();
        let delayed = DelayedNotifier::new(WorkerNotifier::new(move || {
            let _ = target.wake();
        }))?;
        Ok(Self { waker, delayed })
    }

    pub(crate) fn matches(&self, waker: &NodeWaker) -> bool {
        self.waker == *waker
    }

    pub(crate) fn wake(&self) {
        self.delayed.wake();
    }

    pub(crate) fn wake_after(&self, delay: Duration) {
        self.delayed.wake_after(delay);
    }
}

#[cfg(test)]
mod tests;
