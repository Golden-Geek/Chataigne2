use golden_io::WorkerCancellation;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone, Default)]
pub(crate) struct GenerationFence(Arc<SharedFence>);

#[derive(Default)]
struct SharedFence {
    lock: Mutex<()>,
    state: AtomicU8,
    changed: Condvar,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum FenceState {
    Pending = 0,
    Drained = 1,
    Aborted = 2,
}

pub(super) struct CompletionGuard {
    fence: GenerationFence,
    completed: bool,
}

impl GenerationFence {
    pub(super) fn is_drained(&self) -> bool {
        self.0.state.load(Ordering::Acquire) == FenceState::Drained as u8
    }

    pub(super) fn guard(&self) -> CompletionGuard {
        CompletionGuard {
            fence: self.clone(),
            completed: false,
        }
    }

    pub(super) fn wait(&self, cancellation: &WorkerCancellation) -> bool {
        let mut lock = self.0.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        while self.0.state.load(Ordering::Acquire) == FenceState::Pending as u8 && !cancellation.is_cancelled() {
            lock = self
                .0
                .changed
                .wait(lock)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        self.is_drained() && !cancellation.is_cancelled()
    }

    /// Pair cancellation with this lock so a waiter cannot miss a stop between its predicate
    /// check and condvar wait. No worker waits while holding the cancellation lock.
    pub(super) fn wake(&self) {
        let lock = self.0.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        drop(lock);
        self.0.changed.notify_all();
    }

    fn finish(&self, state: FenceState) {
        let lock = self.0.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        self.0.state.store(state as u8, Ordering::Release);
        drop(lock);
        self.0.changed.notify_all();
    }
}

impl CompletionGuard {
    /// Accepted commands and their results are resolved before native client cleanup begins.
    pub(super) fn drained(&mut self) {
        self.completed = true;
        self.fence.finish(FenceState::Drained);
    }
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.fence.finish(FenceState::Aborted);
        }
    }
}
