use std::collections::VecDeque;

use golden_audio::{AnalysisResult, AudioObservationSnapshot, ConfigGeneration};
use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus, EditQueue},
    node::NodeId,
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessCtx,
};
use golden_io::RetainedPayload;

use super::super::SoundCardValueBindings;
use super::capture::ObservationView;

mod cache;
pub(crate) use cache::ValueCache;

pub(super) const SERVICE_ITEMS: usize = 64;

#[derive(Default)]
pub(in super::super) struct ValueDelivery {
    pending: VecDeque<PendingValue>,
    retry: VecDeque<PendingValue>,
    frame: Option<Frame>,
    completed: Option<u64>,
}

struct PendingValue {
    node: NodeId,
    value: ParamValue,
    generation: ConfigGeneration,
    receipt: EditCompletion,
    isolated: bool,
}

struct Frame {
    snapshot: RetainedPayload<AudioObservationSnapshot>,
    binding_generation: ConfigGeneration,
    sequence: u64,
    position: usize,
}

impl Frame {
    fn next(&mut self, bindings: &SoundCardValueBindings) -> Option<(Option<NodeId>, ParamValue)> {
        let snapshot = &self.snapshot;
        let position = self.position;
        self.position += 1;
        if position < 4 {
            return Some(match position {
                0 => (
                    bindings.input_master_level,
                    ParamValue::Float(snapshot.input_global_max_rms.into()),
                ),
                1 => (
                    bindings.output_master_level,
                    ParamValue::Float(snapshot.output_global_max_rms.into()),
                ),
                2 => (
                    bindings.active_voices,
                    ParamValue::Int(snapshot.playback.active_voices.into()),
                ),
                _ => (
                    bindings.loading_voices,
                    ParamValue::Int(snapshot.playback.loading_voices.into()),
                ),
            });
        }
        let position = position - 4;
        if let Some(channel) = snapshot.inputs.get(position).or_else(|| {
            position
                .checked_sub(snapshot.inputs.len())
                .and_then(|index| snapshot.outputs.get(index))
        }) {
            return Some((
                bindings.channel_levels.get(&channel.channel).copied(),
                ParamValue::Float(channel.rms_linear.into()),
            ));
        }
        let position = position - snapshot.inputs.len() - snapshot.outputs.len();
        let tap = snapshot.analysis.taps.get(position / 6)?;
        let (Some(AnalysisResult::Pitch(pitch)), Some(binding)) = (&tap.result, bindings.pitch.get(&tap.tap)) else {
            return Some((None, ParamValue::Bool(false)));
        };
        Some(match position % 6 {
            0 => (Some(binding.valid), ParamValue::Bool(pitch.valid)),
            1 => (Some(binding.frequency_hz), ParamValue::Float(pitch.frequency_hz.into())),
            2 => (Some(binding.confidence), ParamValue::Float(pitch.confidence.into())),
            3 => (Some(binding.midi_note), ParamValue::Float(pitch.midi_note.into())),
            4 if pitch.note_name.len() <= cache::TEXT_BYTES => {
                (Some(binding.note_name), ParamValue::Str(pitch.note_name.clone()))
            }
            4 => (None, ParamValue::Bool(false)),
            _ => (Some(binding.cents), ParamValue::Float(pitch.cents.into())),
        })
    }
}

impl ValueDelivery {
    pub(in super::super) fn reset(&mut self) {
        self.frame = None;
        self.completed = None;
        self.retry.clear();
        // Existing receipts settle without populating a new generation's cache.
        // Their bounded payloads remain alive until service sees their disposition.
    }

    pub(in super::super) fn has_pending(&self) -> bool {
        self.frame.is_some() || !self.pending.is_empty() || !self.retry.is_empty()
    }

    pub(in super::super) fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        bindings: &SoundCardValueBindings,
        generation: ConfigGeneration,
        view: &ObservationView,
        cache: &mut ValueCache,
    ) {
        let mut budget = SERVICE_ITEMS;
        for _ in 0..self.pending.len().min(budget) {
            budget -= 1;
            let mut pending = self.pending.pop_front().expect("bounded pending prefix");
            match pending.receipt.status() {
                EditCompletionStatus::Pending => self.pending.push_back(pending),
                EditCompletionStatus::Applied => {
                    if pending.generation == generation && bindings.is_runtime_value(pending.node) {
                        cache.store(pending.node, generation, &pending.value);
                    }
                }
                EditCompletionStatus::Failed => {
                    if pending.generation == generation && !pending.isolated {
                        pending.isolated = true;
                        self.retry.push_back(pending);
                    }
                }
            }
        }
        if !self.pending.is_empty() {
            return;
        }
        // A failing cell can discard its queued suffix. Retry failed cells one
        // at a time, once per frame, so one missing target cannot poison that
        // suffix again or create an endless actor continuation.
        while budget > 0 {
            let Some(retry) = self.retry.pop_front() else {
                break;
            };
            budget -= 1;
            if retry.generation != generation || !bindings.is_runtime_value(retry.node) {
                continue;
            }
            self.stage(ctx, retry.node, retry.value, generation, true);
            return;
        }
        if !self.retry.is_empty() {
            return;
        }
        if self
            .frame
            .as_ref()
            .is_some_and(|frame| frame.binding_generation != generation)
        {
            self.frame = None;
        }
        if self.frame.is_none() {
            if !view.values_available() {
                return;
            }
            if self.completed == Some(view.sequence) {
                return;
            }
            self.frame = Some(Frame {
                snapshot: view.snapshot(),
                binding_generation: generation,
                sequence: view.sequence,
                position: 0,
            });
        }
        while budget > 0 {
            budget -= 1;
            let frame = self.frame.as_mut().expect("active frame");
            let Some((node, value)) = frame.next(bindings) else {
                if self.pending.is_empty() {
                    let frame = self.frame.take().expect("finished frame");
                    self.completed = Some(frame.sequence);
                } else {
                    // Preserve the end cursor until the submitted prefix settles.
                    frame.position -= 1;
                }
                break;
            };
            let Some(node) = node else {
                continue;
            };
            if cache.matches(node, generation, &value) {
                continue;
            }
            self.stage(ctx, node, value, generation, false);
        }
    }

    fn stage(
        &mut self,
        ctx: &mut ProcessCtx,
        node: NodeId,
        value: ParamValue,
        generation: ConfigGeneration,
        isolated: bool,
    ) {
        let mut edit = EditQueue::new();
        edit.push(Edit::SetParam {
            node,
            value: value.clone(),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        let receipt = edit.completion();
        ctx.edits.pending.extend(edit.drain());
        self.pending.push_back(PendingValue {
            node,
            value,
            generation,
            receipt,
            isolated,
        });
    }
}

#[cfg(test)]
mod tests;
