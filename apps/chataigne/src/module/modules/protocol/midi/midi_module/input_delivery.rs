use super::midi_input::PreparedInput;
use super::*;
use crate::app::module::script_api::CallbackDelivery;
use golden_core::edit::{EditCompletion, EditCompletionStatus, EditQueue};
use golden_io::RetainedPayload;

mod graph;
mod plan;
use graph::{FolderSpec, Target, Value, Write};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Mode {
    bits: u16,
    auto_add: bool,
    precision: i32,
}
pub(super) struct InputDelivery {
    pub input: RetainedPayload<PreparedInput>,
    writes: Vec<Write>,
    next: usize,
    receipt: Option<EditCompletion>,
    values_done: bool,
    callbacks: Vec<CallbackDelivery>,
    mode: Mode,
    module: NodeId,
}
impl InputDelivery {
    fn new(input: RetainedPayload<PreparedInput>, writes: Vec<Write>, module: NodeId, mode: Mode) -> Self {
        let callbacks = input
            .callbacks
            .iter()
            .map(|callback| CallbackDelivery::new(callback.event(module)))
            .collect();
        Self {
            input,
            writes,
            next: 0,
            receipt: None,
            values_done: false,
            callbacks,
            mode,
            module,
        }
    }
    pub(super) fn requires_snapshot(&self) -> bool {
        !self.values_done
            && self.next < self.writes.len()
            && self
                .receipt
                .as_ref()
                .is_none_or(|r| r.status() == EditCompletionStatus::Applied)
    }
    pub(super) fn graph_done(&self) -> bool {
        self.values_done && self.receipt.is_none()
    }
    pub(super) fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: Option<&ProcessTreeSnapshot>,
        root: Option<NodeId>,
        mode: Mode,
    ) -> bool {
        if let Some(receipt) = self.receipt.take() {
            match receipt.status() {
                EditCompletionStatus::Pending => {
                    self.receipt = Some(receipt);
                    return false;
                }
                EditCompletionStatus::Failed => {
                    logerror!("MIDI input graph edits failed; unapplied suffix abandoned without replaying the applied prefix");
                    self.values_done = true;
                }
                EditCompletionStatus::Applied => {}
            }
        }
        if !self.values_done && mode != self.mode {
            logerror!("MIDI input configuration changed; unapplied graph suffix cancelled");
            self.values_done = true;
        }
        self.values_done |= self.next == self.writes.len();
        if !self.values_done {
            let Some(snapshot) = snapshot else { return false };
            let parent = std::mem::replace(&mut ctx.edits, EditQueue::new());
            let mut units = 0;
            let mut bytes = 0;
            while let Some(write) = self.writes.get_mut(self.next) {
                let weight = write.value(&self.input).retained_bytes();
                if units >= 16 || units != 0 && bytes + weight > 64 * 1024 {
                    break;
                }
                match write.apply(ctx, snapshot, root, &self.input, self.mode.auto_add, self.module) {
                    Ok(graph::Progress::Done) => self.next += 1,
                    Ok(graph::Progress::Pending) => break,
                    Err(error) => {
                        logerror!(format!("MIDI input graph suffix cancelled: {error}"));
                        self.values_done = true;
                        break;
                    }
                }
                units += 1;
                bytes = bytes.saturating_add(weight);
            }
            let mut owned = std::mem::replace(&mut ctx.edits, parent);
            if !owned.pending.is_empty() {
                self.receipt = Some(owned.completion());
                ctx.edits.pending.extend(owned.drain());
                return false;
            }
            self.values_done |= self.next == self.writes.len();
            if !self.values_done {
                return false;
            }
        }
        let mut complete = true;
        for callback in &mut self.callbacks {
            complete &= callback.service(ctx);
        }
        complete
    }
}
impl MidiModule {
    pub(super) fn input_mode(&self) -> Mode {
        let mut bits = 0u16;
        for channel in 1..=16 {
            if self.cc_channel_is_14_bit_handle(channel) {
                bits |= 1 << (channel - 1);
            }
        }
        Mode {
            bits,
            auto_add: self.auto_add.get(),
            precision: self.clock_bpm_precision.get(),
        }
    }
    pub(super) fn prepare_delivery(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        input: RetainedPayload<PreparedInput>,
    ) -> InputDelivery {
        let mode = self.input_mode();
        let received_at = input.arrived.map_or(ctx.runtime_elapsed, |at| {
            ctx.runtime_elapsed.saturating_sub(at.elapsed())
        });
        let writes = self.input_writes(ctx, snapshot, &input, received_at);
        InputDelivery::new(input, writes, self.id(), mode)
    }
}
