use super::*;
use crate::app::module::{
    common::received_values::service::{ReceivedValueCursor, ReceivedValueProgress},
    script_api::CallbackDelivery,
};
use golden_core::edit::{Edit, EditQueue};

pub(super) struct Delivery {
    input: RetainedPayload<Prepared>,
    values: ReceivedValueCursor,
    values_done: bool,
    callback: CallbackDelivery,
    error: Option<String>,
}
impl Delivery {
    pub(super) fn new(input: RetainedPayload<Prepared>) -> Self {
        Self {
            callback: CallbackDelivery::new(input.callback.event(input.source.module)),
            error: input.error.clone(),
            input,
            values: ReceivedValueCursor::default(),
            values_done: false,
        }
    }
    pub(super) fn requires_snapshot(&self) -> bool {
        self.values_done || self.values.requires_snapshot(&self.input.values)
    }
    pub(super) fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }
    pub(super) fn service(&mut self, ctx: &mut ProcessCtx, snapshot: Option<&ProcessTreeSnapshot>) -> bool {
        if self.requires_snapshot() && snapshot.is_none() {
            return false;
        }
        if !self.input.source.lifetime.is_current() {
            self.error = Some("MQTT input source retired; unapplied suffix cancelled without replay".into());
            return true;
        }
        if snapshot.is_some_and(|snapshot| !self.input.source.current(snapshot)) {
            self.error = Some(
                "MQTT input processing or Values source changed; unapplied suffix cancelled without replay".into(),
            );
            return true;
        }
        let parent = std::mem::replace(&mut ctx.edits, EditQueue::new());
        let done = if !self.values_done {
            match self.values.service(
                &self.input.values,
                ctx,
                snapshot,
                Some(self.input.source.root),
                self.input.source.mode.auto_add,
                true,
            ) {
                ReceivedValueProgress::Pending => false,
                ReceivedValueProgress::Failed => {
                    self.error = Some("MQTT received-value edits failed; applied prefix is not replayed".into());
                    self.values_done = true;
                    false
                }
                ReceivedValueProgress::Complete => {
                    self.values_done = true;
                    false
                }
            }
        } else {
            self.callback.service(ctx)
        };
        let mut owned = std::mem::replace(&mut ctx.edits, parent);
        if !owned.pending.is_empty() {
            let snapshot = snapshot.unwrap();
            let targets = match targets::capture(snapshot, self.input.source.root, &owned.pending) {
                Ok(targets) => targets,
                Err(error) => {
                    self.error = Some(error);
                    self.values_done = true;
                    return false;
                }
            };
            let mut fenced = EditQueue::new();
            fenced.push(Edit::CheckPreconditions {
                preconditions: self.input.source.preconditions(snapshot),
            });
            // Every read precedes every mutation. A structural prefix can then
            // change its own parents without making a later guard falsely stale.
            for preconditions in targets {
                fenced.push(Edit::CheckPreconditions { preconditions });
            }
            fenced.pending.extend(owned.drain());
            fenced.retain_payload(self.input.clone());
            ctx.edits.pending.extend(fenced.drain());
        }
        done
    }
}
