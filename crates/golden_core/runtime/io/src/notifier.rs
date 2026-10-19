use std::sync::Arc;

/// Payload-free readiness notification. Supply a short, nonblocking, infallible callback.
/// The owner still drains its bounded mailbox and handles cancellation/disconnection.
#[derive(Clone)]
pub struct WorkerNotifier(Arc<dyn Fn() + Send + Sync>);

impl WorkerNotifier {
    pub fn new(notify: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(notify))
    }

    pub fn notify(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for WorkerNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerNotifier").finish_non_exhaustive()
    }
}
