use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::native::AdapterId;

/// Qualification-only selection before the real native factory call. This is scoped to one
/// owned control, never process-global, and is absent from production builds.
#[derive(Clone)]
pub(crate) struct InitializationGate {
    unavailable: AdapterId,
    released: Arc<AtomicBool>,
}

impl InitializationGate {
    pub(crate) fn new(unavailable: AdapterId) -> Self {
        Self {
            unavailable,
            released: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn release(&self) {
        self.released.store(true, Ordering::Release);
    }

    pub(crate) fn adapter(&self, selected: AdapterId) -> AdapterId {
        if self.released.load(Ordering::Acquire) {
            selected
        } else {
            self.unavailable
        }
    }
}
