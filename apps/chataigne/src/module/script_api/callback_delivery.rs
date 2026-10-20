use crate::app::module::common::edit_receipt::EditReceipt;
use golden_core::{edit::Edit, events::CustomEvent, process_ctx::ProcessCtx};

pub(crate) struct CallbackDelivery {
    event: CustomEvent,
    completion: EditReceipt,
}

impl CallbackDelivery {
    pub(crate) fn new(event: CustomEvent) -> Self {
        Self {
            event,
            completion: EditReceipt::default(),
        }
    }

    /// Retain the shared payload until its own publication commits. Retry only an event
    /// that was not published; subscriber failures must never replay a delivered callback.
    pub(crate) fn service(&mut self, ctx: &mut ProcessCtx) -> bool {
        // Observe only this event, independent of other edits already in the callback context.
        self.completion.service(ctx, || Edit::EmitCustomEvent {
            event: self.event.clone(),
        })
    }
}
