use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditQueue},
    process_ctx::ProcessCtx,
};

/// Tracks one repeatable edit, never a partially applied batch. Each caller owns
/// its publication state; failure of a later subscriber does not replay this edit.
#[derive(Default)]
pub(crate) struct EditReceipt(Option<EditCompletion>);

impl EditReceipt {
    pub(crate) fn service(&mut self, ctx: &mut ProcessCtx, edit: impl FnOnce() -> Edit) -> bool {
        match self.0.as_ref().map(EditCompletion::status) {
            Some(EditCompletionStatus::Applied) => return true,
            Some(EditCompletionStatus::Pending) => return false,
            None | Some(EditCompletionStatus::Failed) => {}
        }
        let mut delivery = EditQueue::new();
        delivery.push(edit());
        self.0 = Some(delivery.completion());
        ctx.edits.pending.extend(delivery.drain());
        false
    }
}
