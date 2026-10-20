use super::ModuleWake;
use golden_core::engine::NodeWaker;

impl ModuleWake {
    // Lifecycle fixtures must inspect the retired source rather than count
    // unrelated notifications from its asynchronously started replacement.
    pub(crate) fn source(&self) -> NodeWaker {
        self.waker.clone()
    }
}
