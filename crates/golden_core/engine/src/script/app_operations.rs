//! Deferred application work admitted by a node callback, with its originating edit receipt.

use crate::{
    edit::{Edit, EditCompletion},
    node::NodeId,
    process_ctx::ProcessCtx,
    script::ScriptAppOperationRequest,
};
use std::sync::{Arc, OnceLock, Weak};

/// Topic delivered directly to an app request's source node after atomic application.
/// The shared payload uses the standard `onAppOperationResult` callback envelope.
pub const APP_OPERATION_RESULT_TOPIC: &str = "golden.script.appOperation";

/// Internal edit payload for a callback-owned app request.
pub struct PendingScriptAppOperation {
    pub(crate) source: NodeId,
    pub(crate) owner: Weak<()>,
    pub(crate) request: ScriptAppOperationRequest,
    pub(crate) fence: Arc<OnceLock<EditCompletion>>,
}

impl ProcessCtx {
    /// Queues an already-admitted app request from a node or script callback.
    ///
    /// `owner` must belong to the live source instance and expire on destruction/reload.
    /// Requests run only after the originating callback edits commit. The bounded application
    /// service returns the committed result or rejection directly to `source` through
    /// [`APP_OPERATION_RESULT_TOPIC`]; dropping the owner cancels abandoned requests.
    pub fn enqueue_app_operation(&mut self, source: NodeId, owner: Weak<()>, request: ScriptAppOperationRequest) {
        let pending = PendingScriptAppOperation {
            source,
            owner,
            request,
            fence: Arc::new(OnceLock::new()),
        };
        // Register requests before a later failing edit can discard the queue suffix.
        // Their receipt still covers the entire originating callback batch.
        let index = self
            .edits
            .pending
            .iter()
            .take_while(|edit| matches!(edit.edit, Edit::RequestAppOperation { .. }))
            .count();
        self.edits.pending.insert(
            index,
            crate::edit::EditRequest::new(Edit::RequestAppOperation { pending }),
        );
    }
}

impl<T: crate::node::Node> crate::engine::Engine<T> {
    pub(crate) fn prepare_script_app_operation_fences(&mut self, ctx: &mut ProcessCtx) {
        let fences = ctx
            .edits
            .pending
            .iter()
            .filter_map(|edit| match &edit.edit {
                Edit::RequestAppOperation { pending } if pending.fence.get().is_none() => Some(pending.fence.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !fences.is_empty() {
            let completion = ctx.edits.completion();
            for fence in fences {
                let _ = fence.set(completion.clone());
            }
        }
    }
}
