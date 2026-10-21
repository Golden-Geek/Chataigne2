use super::*;
use golden_core::edit::{EditCompletion, EditCompletionStatus, EditQueue};
mod cc;
mod locks;
#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct StructureWork {
    cc: cc::CcWork,
    locks: Option<locks::LockWork>,
    queued: Option<EditCompletion>,
    generation: u64,
}
impl StructureWork {
    pub(super) fn busy(&self) -> bool {
        self.queued
            .as_ref()
            .is_some_and(|receipt| receipt.status() == EditCompletionStatus::Pending)
            || self.locks.is_some()
            || self.cc.busy()
    }
    pub(super) fn requires_snapshot(&self) -> bool {
        self.locks
            .as_ref()
            .map_or_else(|| self.cc.requires_snapshot(), locks::LockWork::requires_snapshot)
    }
    pub(super) fn cancel(&mut self) {
        self.cc = cc::CcWork::default();
        self.locks = None;
        self.queued = None;
        self.generation = self.generation.wrapping_add(1);
    }
}
impl MidiModule {
    pub(super) fn find_cc_folder(&self, snapshot: &ProcessTreeSnapshot, channel: u8) -> Option<NodeId> {
        let values_id = self.base.values_id()?;
        let channel_id = snapshot.find_child_by_decl_id(values_id, channel_decl_id(channel).as_str())?;
        snapshot.find_child_by_decl_id(channel_id, CONTROL_CHANGE_FOLDER_DECL_ID)
    }

    pub(super) fn cc_channel_is_14_bit(&self, _snapshot: &ProcessTreeSnapshot, channel: u8) -> bool {
        self.cc_channel_is_14_bit_handle(channel)
    }
    pub(super) fn cc_channel_is_14_bit_handle(&self, channel: u8) -> bool {
        match channel {
            1 => self.channel_1.get(),
            2 => self.channel_2.get(),
            3 => self.channel_3.get(),
            4 => self.channel_4.get(),
            5 => self.channel_5.get(),
            6 => self.channel_6.get(),
            7 => self.channel_7.get(),
            8 => self.channel_8.get(),
            9 => self.channel_9.get(),
            10 => self.channel_10.get(),
            11 => self.channel_11.get(),
            12 => self.channel_12.get(),
            13 => self.channel_13.get(),
            14 => self.channel_14.get(),
            15 => self.channel_15.get(),
            16 => self.channel_16.get(),
            _ => false,
        }
    }

    pub(super) fn cc_parameter_is_14_bit(&self, snapshot: &ProcessTreeSnapshot, channel: u8, controller: u8) -> bool {
        cc_supports_14_bit(controller) && self.cc_channel_is_14_bit(snapshot, channel)
    }

    pub(super) fn channel_14_bit_toggle_channel(&self, param: NodeId) -> Option<u8> {
        if self.channel_1.is_bound() && self.channel_1.id() == param {
            return Some(1);
        }
        if self.channel_2.is_bound() && self.channel_2.id() == param {
            return Some(2);
        }
        if self.channel_3.is_bound() && self.channel_3.id() == param {
            return Some(3);
        }
        if self.channel_4.is_bound() && self.channel_4.id() == param {
            return Some(4);
        }
        if self.channel_5.is_bound() && self.channel_5.id() == param {
            return Some(5);
        }
        if self.channel_6.is_bound() && self.channel_6.id() == param {
            return Some(6);
        }
        if self.channel_7.is_bound() && self.channel_7.id() == param {
            return Some(7);
        }
        if self.channel_8.is_bound() && self.channel_8.id() == param {
            return Some(8);
        }
        if self.channel_9.is_bound() && self.channel_9.id() == param {
            return Some(9);
        }
        if self.channel_10.is_bound() && self.channel_10.id() == param {
            return Some(10);
        }
        if self.channel_11.is_bound() && self.channel_11.id() == param {
            return Some(11);
        }
        if self.channel_12.is_bound() && self.channel_12.id() == param {
            return Some(12);
        }
        if self.channel_13.is_bound() && self.channel_13.id() == param {
            return Some(13);
        }
        if self.channel_14.is_bound() && self.channel_14.id() == param {
            return Some(14);
        }
        if self.channel_15.is_bound() && self.channel_15.id() == param {
            return Some(15);
        }
        if self.channel_16.is_bound() && self.channel_16.id() == param {
            return Some(16);
        }
        None
    }

    pub(super) fn queue_cc_conversion(&mut self, ctx: &mut ProcessCtx, channel: u8, previous: bool) {
        self.structure_work.cc.pending[usize::from(channel - 1)].get_or_insert(cc::Pending {
            previous,
            restore: true,
        });
        if self
            .structure_work
            .queued
            .as_ref()
            .is_some_and(|receipt| receipt.status() == EditCompletionStatus::Pending)
        {
            return;
        }
        let generation = self.structure_work.generation;
        let source = ctx.node_waker().ok();
        let mut edits = EditQueue::new();
        edits.push(Edit::CallNodeMutation {
            node: self.id(),
            needs_tree_snapshot: true,
            callback: Box::new(move |node, inner| {
                if source.as_ref().is_some_and(|source| source.is_closed()) {
                    return Ok(());
                }
                let module = node
                    .as_any_mut()
                    .downcast_mut::<MidiModule>()
                    .ok_or("expected MIDI structure owner")?;
                if module.structure_work.generation != generation {
                    return Ok(());
                }
                module.structure_work.queued = None;
                let snapshot = inner.tree_snapshot_arc();
                module.service_structure(inner, snapshot.as_deref());
                Ok(())
            }),
        });
        self.structure_work.queued = Some(edits.completion());
        ctx.edits.pending.extend(edits.drain());
        self.request_structure_wake(ctx);
    }
    pub(super) fn start_value_locks(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.structure_work.locks = self
            .base
            .values_id()
            .and_then(|root| locks::LockWork::new(snapshot, root, self.id()));
        for channel in 1..=16 {
            if self.find_cc_folder(snapshot, channel).is_some() {
                self.structure_work.cc.pending[usize::from(channel - 1)] = Some(cc::Pending {
                    previous: !self.cc_channel_is_14_bit_handle(channel),
                    restore: false,
                });
            }
        }
        self.service_structure(ctx, Some(snapshot));
    }
    pub(super) fn service_structure(&mut self, ctx: &mut ProcessCtx, snapshot: Option<&ProcessTreeSnapshot>) {
        if let Some(mut work) = self.structure_work.locks.take() {
            if !work.service(ctx, snapshot) {
                self.structure_work.locks = Some(work);
                self.request_structure_wake(ctx);
                return;
            }
        }
        self.service_cc_structure(ctx, snapshot);
        self.request_structure_wake(ctx);
    }
    pub(super) fn request_structure_wake(&self, ctx: &ProcessCtx) {
        if self.structure_work.busy() {
            if let Ok(waker) = ctx.node_waker() {
                let _ = waker.wake();
            }
        }
    }
    pub(super) fn channel_toggle_id(&self, channel: u8) -> Option<NodeId> {
        match channel {
            1 => self.channel_1.is_bound().then(|| self.channel_1.id()),
            2 => self.channel_2.is_bound().then(|| self.channel_2.id()),
            3 => self.channel_3.is_bound().then(|| self.channel_3.id()),
            4 => self.channel_4.is_bound().then(|| self.channel_4.id()),
            5 => self.channel_5.is_bound().then(|| self.channel_5.id()),
            6 => self.channel_6.is_bound().then(|| self.channel_6.id()),
            7 => self.channel_7.is_bound().then(|| self.channel_7.id()),
            8 => self.channel_8.is_bound().then(|| self.channel_8.id()),
            9 => self.channel_9.is_bound().then(|| self.channel_9.id()),
            10 => self.channel_10.is_bound().then(|| self.channel_10.id()),
            11 => self.channel_11.is_bound().then(|| self.channel_11.id()),
            12 => self.channel_12.is_bound().then(|| self.channel_12.id()),
            13 => self.channel_13.is_bound().then(|| self.channel_13.id()),
            14 => self.channel_14.is_bound().then(|| self.channel_14.id()),
            15 => self.channel_15.is_bound().then(|| self.channel_15.id()),
            16 => self.channel_16.is_bound().then(|| self.channel_16.id()),
            _ => None,
        }
    }
    pub(super) fn set_internal_param(&mut self, ctx: &mut ProcessCtx, param: NodeId, value: ParamValue) {
        ctx.set_param_from(self.id(), param, value, ParameterEventBehaviour::Append);
    }
}
