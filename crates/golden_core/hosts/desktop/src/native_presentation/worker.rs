use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex, atomic::AtomicBool, atomic::Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

type Work = Box<dyn FnOnce(NativePresentationStop) -> io::Result<()> + Send + 'static>;

/// Work whose native rendering resources stay on its dedicated worker thread.
///
/// Capture configuration and renderer-safe handles only. Construct and retire GPU/SDK resources
/// inside the callback, including on errors and cancellation. Returning means every resource
/// using the host's window has retired, allowing the host to destroy that window.
pub struct NativePresentationTask {
    work: Work,
}

impl NativePresentationTask {
    /// Defers resource construction, rendering and retirement to the dedicated worker.
    pub fn new<F>(work: F) -> Self
    where
        F: FnOnce(NativePresentationStop) -> io::Result<()> + Send + 'static,
    {
        Self { work: Box::new(work) }
    }

    fn run(self, stop: NativePresentationStop) -> io::Result<()> {
        (self.work)(stop)
    }
}

#[derive(Default)]
struct StopState {
    requested: AtomicBool,
    wait_lock: Mutex<()>,
    changed: Condvar,
}

/// A bounded, idempotent cancellation signal shared with the presentation task.
///
/// This signal owns no native resources. A task must observe cancellation and retire its
/// resources before returning; the signal alone does not certify retirement.
#[derive(Clone)]
pub struct NativePresentationStop {
    state: Arc<StopState>,
}

impl NativePresentationStop {
    fn new() -> Self {
        Self {
            state: Arc::new(StopState::default()),
        }
    }

    /// Returns whether the host has requested cancellation; once true, it remains true.
    pub fn is_requested(&self) -> bool {
        self.state.requested.load(Ordering::Acquire)
    }

    /// Waits for cancellation or the supplied timeout, returning whether it was requested.
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        if self.is_requested() {
            return true;
        }
        let guard = self
            .state
            .wait_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (_guard, _) = self
            .state
            .changed
            .wait_timeout_while(guard, timeout, |_| !self.is_requested())
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.is_requested()
    }

    fn request(&self) {
        if !self.state.requested.swap(true, Ordering::AcqRel) {
            // Pair notification with the wait mutex so a waiter cannot miss cancellation
            // between its predicate check and the condvar releasing that mutex.
            let _guard = self
                .state
                .wait_lock
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            self.state.changed.notify_all();
        }
    }
}

pub(crate) struct PresentationWorker {
    stop: NativePresentationStop,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl PresentationWorker {
    pub(crate) fn spawn(
        task: NativePresentationTask,
        on_finished: impl FnOnce(bool) + Send + 'static,
    ) -> io::Result<Self> {
        let stop = NativePresentationStop::new();
        let task_stop = stop.clone();
        let thread = thread::Builder::new()
            .name("golden-native-presentation".into())
            .spawn(move || {
                let outcome = catch_unwind(AssertUnwindSafe(move || task.run(task_stop)))
                    .unwrap_or_else(|panic| Err(task_panic(panic)));
                // The task and its captures have been consumed, including unwinding drops.
                // Completion therefore follows retirement rather than merely requesting it.
                on_finished(outcome.is_ok());
                outcome
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }

    pub(crate) fn request_stop(&self) {
        self.stop.request();
    }

    /// Call only after the native event loop has returned; this can wait for retirement.
    pub(crate) fn join(mut self) -> io::Result<()> {
        self.request_stop();
        self.thread
            .take()
            .expect("presentation worker is retained until join")
            .join()
            .unwrap_or_else(|panic| Err(task_panic(panic)))
    }
}

impl Drop for PresentationWorker {
    fn drop(&mut self) {
        self.request_stop();
        // Dropping a JoinHandle detaches it. Never wait or retire GPU resources on the UI
        // thread, including during a host setup failure or unexpected event-loop teardown.
    }
}

fn task_panic(panic: Box<dyn std::any::Any + Send>) -> io::Error {
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload");
    io::Error::other(format!("native presentation worker panicked: {message}"))
}
