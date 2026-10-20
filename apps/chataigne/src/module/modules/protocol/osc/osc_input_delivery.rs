use super::osc_input::PreparedInput;
use crate::app::module::{
    common::received_values::service::{ReceivedValueCursor, ReceivedValueProgress},
    script_api::CallbackDelivery,
};
use golden_core::{
    edit::Edit,
    node::NodeId,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::RetainedPayload;
use std::collections::HashMap;

pub(super) struct InputDelivery {
    input: RetainedPayload<PreparedInput>,
    values: ReceivedValueCursor,
    values_done: bool,
    callbacks: Vec<CallbackDelivery>,
}
impl InputDelivery {
    pub(super) fn new(input: RetainedPayload<PreparedInput>) -> Self {
        Self {
            callbacks: input
                .callbacks
                .iter()
                .map(|callback| CallbackDelivery::new(callback.event(input.module)))
                .collect(),
            input,
            values: ReceivedValueCursor::default(),
            values_done: false,
        }
    }
    pub(super) fn requires_snapshot(&self) -> bool {
        !self.values_done && self.values.requires_snapshot(&self.input.values)
    }
    pub(super) fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: Option<&ProcessTreeSnapshot>,
        root: Option<NodeId>,
        auto_add: bool,
        ignored: &mut HashMap<NodeId, usize>,
    ) -> bool {
        if !self.values_done {
            let start = ctx.edits.pending.len();
            let progress = self
                .values
                .service(&self.input.values, ctx, snapshot, root, auto_add, true);
            for edit in &ctx.edits.pending[start..] {
                if let Edit::SetParam { node, .. } = &edit.edit {
                    *ignored.entry(*node).or_default() += 1;
                }
            }
            if progress == ReceivedValueProgress::Pending {
                return false;
            }
            if progress == ReceivedValueProgress::Failed {
                golden_core::logerror!("OSC received-value edits failed; applied prefix is not replayed");
            }
            self.values_done = true;
        }
        // Submit the bounded prefix together; each receipt retries only its own
        // unpublished event, and a later subscriber failure cannot replay it.
        let mut complete = true;
        for callback in &mut self.callbacks {
            complete &= callback.service(ctx);
        }
        complete
    }
}
