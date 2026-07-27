use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread,
    time::Duration,
};

use golden_core::{edit::Edit, events::CustomEvent, node::NodeId};

pub(crate) const SOUND_CARD_RUNTIME_WAKE_TOPIC: &str =
    "chataigne.sound_card.runtime_wake.v1";

/// Coalesces worker notifications into transient engine events.
///
/// The host/runtime boundary owns wake timing. The Sound Card node itself remains
/// passive and only runs when an authored graph event or this wake event reaches
/// its inbox.
#[derive(Clone)]
pub(crate) struct RuntimeWakeSender {
    edits: Sender<Edit>,
    module: NodeId,
    pending: Arc<AtomicBool>,
}

impl RuntimeWakeSender {
    pub(crate) fn new(edits: Sender<Edit>, module: NodeId) -> Self {
        Self {
            edits,
            module,
            pending: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn wake(&self) {
        if self.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let event = CustomEvent::transient(
            SOUND_CARD_RUNTIME_WAKE_TOPIC,
            Some(self.module),
            serde_json::Value::Null,
        );
        if self.edits.send(Edit::EmitCustomEvent { event }).is_err() {
            self.pending.store(false, Ordering::Release);
        }
    }

    pub(crate) fn wake_after(&self, delay: Duration) {
        let wake = self.clone();
        let _ = thread::Builder::new()
            .name("chataigne-sound-card-retry-timer".to_owned())
            .spawn(move || {
                thread::sleep(delay);
                wake.wake();
            });
    }

    pub(crate) fn acknowledge(&self) {
        self.pending.store(false, Ordering::Release);
    }
}

impl std::fmt::Debug for RuntimeWakeSender {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeWakeSender")
            .field("module", &self.module)
            .field("pending", &self.pending.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}
