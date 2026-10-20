use std::collections::VecDeque;

use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditQueue},
    node::NodeWarning,
    process_ctx::ProcessCtx,
};
use golden_io::RetainedPayload;

use super::{WarningPatch, WarningPlan};

pub(crate) const SERVICE_ITEMS: usize = 16;

#[derive(Default)]
pub(crate) struct WarningDelivery {
    frame: Option<(RetainedPayload<WarningPlan>, usize)>,
    pending: VecDeque<Pending>,
    retry: VecDeque<Pending>,
    failed: bool,
}

struct Pending {
    plan: RetainedPayload<WarningPlan>,
    index: usize,
    receipt: EditCompletion,
    isolated: bool,
}

impl WarningDelivery {
    pub(crate) fn start(&mut self, plan: RetainedPayload<WarningPlan>) {
        self.frame = Some((plan, 0));
    }

    /// Settle admitted edits before capturing a replacement source, so a late
    /// old edit cannot invalidate the replacement diff.
    pub(crate) fn quiesce(&mut self) -> bool {
        self.frame = None;
        self.retry.clear();
        self.pending.is_empty()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.frame.is_some() || !self.pending.is_empty() || !self.retry.is_empty()
    }

    pub(crate) fn take_failure(&mut self) -> bool {
        std::mem::take(&mut self.failed)
    }

    pub(crate) fn service(&mut self, ctx: &mut ProcessCtx, paused: bool) {
        let mut budget = SERVICE_ITEMS;
        for _ in 0..self.pending.len().min(budget) {
            budget -= 1;
            let pending = self.pending.pop_front().expect("bounded pending prefix");
            match pending.receipt.status() {
                EditCompletionStatus::Pending => self.pending.push_back(pending),
                EditCompletionStatus::Applied => {}
                EditCompletionStatus::Failed if paused => {}
                EditCompletionStatus::Failed if !pending.isolated => self.retry.push_back(pending),
                EditCompletionStatus::Failed => self.failed = true,
            }
        }
        if paused || !self.pending.is_empty() {
            return;
        }
        if budget > 0 {
            if let Some(retry) = self.retry.pop_front() {
                self.stage(ctx, retry.plan, retry.index, true);
                return;
            }
        }
        if !self.retry.is_empty() {
            return;
        }
        while budget > 0 {
            budget -= 1;
            let Some((plan, index)) = self.frame.as_mut() else {
                break;
            };
            if *index == plan.patches.len() {
                self.frame = None;
                break;
            }
            let plan = plan.clone();
            let position = *index;
            *index += 1;
            self.stage(ctx, plan, position, false);
        }
    }

    fn stage(&mut self, ctx: &mut ProcessCtx, plan: RetainedPayload<WarningPlan>, index: usize, isolated: bool) {
        let edit = match &plan.patches[index] {
            WarningPatch::Set(warning) => Edit::SetNodeWarning {
                node: warning.node,
                warning: NodeWarning {
                    id: warning.id.clone(),
                    message: warning.message.clone(),
                    detail: warning.detail.clone(),
                },
            },
            WarningPatch::Clear { node, id } => Edit::ClearNodeWarning {
                node: *node,
                warning_id: Some((*id).into()),
            },
        };
        let mut queue = EditQueue::new();
        queue.push(edit);
        let receipt = queue.completion();
        ctx.edits.pending.extend(queue.drain());
        self.pending.push_back(Pending {
            plan,
            index,
            receipt,
            isolated,
        });
    }
}
