use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditQueue},
    events::CustomEvent,
    process_ctx::ProcessCtx,
};

pub(crate) struct CallbackDelivery {
    event: CustomEvent,
    completion: Option<EditCompletion>,
}

impl CallbackDelivery {
    pub(crate) fn new(event: CustomEvent) -> Self {
        Self {
            event,
            completion: None,
        }
    }

    /// Retain the shared payload until its own publication commits. Retry only an event
    /// that was not published; subscriber failures must never replay a delivered callback.
    pub(crate) fn service(&mut self, ctx: &mut ProcessCtx) -> bool {
        if let Some(completion) = self.completion.take() {
            match completion.status() {
                EditCompletionStatus::Applied => return true,
                EditCompletionStatus::Pending => {
                    self.completion = Some(completion);
                    return false;
                }
                EditCompletionStatus::Failed => {}
            }
        }
        // Observe only this event, independent of other edits already in the callback context.
        let mut delivery = EditQueue::new();
        delivery.push(Edit::EmitCustomEvent {
            event: self.event.clone(),
        });
        self.completion = Some(delivery.completion());
        ctx.edits.pending.extend(delivery.drain());
        false
    }
}
