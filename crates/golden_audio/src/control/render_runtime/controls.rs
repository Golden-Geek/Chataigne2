use std::collections::HashMap;

use rtrb::{Consumer, Producer, RingBuffer};

use crate::{
    AudioChannelId, AudioError, CommandSequence, ConfigGeneration, GainDb, PlanSwapResult, RealtimePlanRetirement,
    RealtimePlanSlot, RenderPlanPublisher, acknowledged_plan_exchange, realtime::assert_not_realtime,
};

use super::RenderRuntimePlan;

#[derive(Clone, Copy, Debug)]
enum GainTarget {
    Master,
    Output(usize),
}

#[derive(Clone, Copy, Debug)]
enum ControlKind {
    Plan,
    Gain { target: GainTarget, linear: f32 },
}

#[derive(Clone, Copy, Debug)]
struct RenderControl {
    sequence: CommandSequence,
    generation: ConfigGeneration,
    kind: ControlKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ControlRejection {
    OutOfOrder,
    MissingPlan,
    GenerationMismatch,
    InvalidTarget,
}

#[derive(Debug)]
pub(super) struct RenderControlWriter {
    plans: RenderPlanPublisher<Option<RenderRuntimePlan>>,
    controls: Producer<RenderControl>,
    generation: Option<ConfigGeneration>,
    outputs: HashMap<AudioChannelId, usize>,
    last_sequence: u64,
}

#[derive(Debug)]
pub(super) struct RenderControlReader {
    plans: RealtimePlanSlot<Option<RenderRuntimePlan>>,
    controls: Consumer<RenderControl>,
    pending: Option<RenderControl>,
    last_sequence: u64,
}

pub(super) fn render_controls(capacity: usize) -> (RenderControlWriter, RenderControlReader) {
    let (publisher, plans) = acknowledged_plan_exchange(Box::new(None));
    let (controls, reader) = RingBuffer::new(capacity);
    (
        RenderControlWriter {
            plans: publisher,
            controls,
            generation: None,
            outputs: HashMap::new(),
            last_sequence: 0,
        },
        RenderControlReader {
            plans,
            controls: reader,
            pending: None,
            last_sequence: 0,
        },
    )
}

impl RenderControlWriter {
    pub(super) fn publish_plan(
        &mut self,
        sequence: CommandSequence,
        plan: RenderRuntimePlan,
    ) -> Result<(), AudioError> {
        self.preflight(sequence)?;
        if self.generation.is_some_and(|previous| plan.generation <= previous) {
            return Err(AudioError::invalid_configuration("render plan generation must advance"));
        }
        let generation = plan.generation;
        let outputs = plan
            .processor
            .plan()
            .virtual_outputs
            .iter()
            .copied()
            .enumerate()
            .map(|(index, channel)| (channel, index))
            .collect();
        self.plans
            .publish_without_reclaim(Box::new(Some(plan)))
            .map_err(|error| {
                drop(error.into_plan());
                AudioError::queue_full("managed_render_plan")
            })?;
        // The sole producer preflighted a slot. The consumer can only free more slots.
        self.commit(RenderControl {
            sequence,
            generation,
            kind: ControlKind::Plan,
        });
        self.generation = Some(generation);
        self.outputs = outputs;
        Ok(())
    }

    pub(super) fn set_master_gain(&mut self, sequence: CommandSequence, gain: GainDb) -> Result<(), AudioError> {
        self.set_gain(sequence, GainTarget::Master, gain)
    }

    pub(super) fn set_channel_gain(
        &mut self,
        sequence: CommandSequence,
        channel: AudioChannelId,
        gain: GainDb,
    ) -> Result<(), AudioError> {
        assert_not_realtime("render output target resolution");
        let index = self.outputs.get(&channel).copied().ok_or_else(|| {
            AudioError::invalid_configuration(format!(
                "accepted render plan does not contain output channel {channel}"
            ))
        })?;
        self.set_gain(sequence, GainTarget::Output(index), gain)
    }

    fn set_gain(&mut self, sequence: CommandSequence, target: GainTarget, gain: GainDb) -> Result<(), AudioError> {
        self.preflight(sequence)?;
        let generation = self
            .generation
            .ok_or_else(|| AudioError::invalid_configuration("audio runtime has no accepted render plan"))?;
        self.commit(RenderControl {
            sequence,
            generation,
            kind: ControlKind::Gain {
                target,
                linear: gain.to_linear(),
            },
        });
        Ok(())
    }

    fn preflight(&mut self, sequence: CommandSequence) -> Result<(), AudioError> {
        assert_not_realtime("render control admission");
        if sequence.get() <= self.last_sequence {
            return Err(AudioError::invalid_configuration(
                "render control sequence must advance",
            ));
        }
        if self.controls.is_abandoned() {
            return Err(AudioError::new(
                crate::AudioErrorCategory::ShuttingDown,
                "render control consumer exited",
            ));
        }
        if self.controls.slots() == 0 {
            return Err(AudioError::queue_full("managed_render_control"));
        }
        Ok(())
    }

    fn commit(&mut self, control: RenderControl) {
        self.controls.push(control).expect("preflighted render control slot");
        self.last_sequence = control.sequence.get();
    }

    pub(super) fn reclaim_acknowledged(&mut self) -> usize {
        self.plans.reclaim_acknowledged()
    }
}

impl RenderControlReader {
    /// A finite queue prefix is consumed in sequence order. Plan adoption occurs only at its marker.
    pub(super) fn begin_block(&mut self, mut reject: impl FnMut(ControlRejection)) {
        if self.plans.return_retained_plan().is_some() {
            return;
        }
        let budget = self.controls.slots() + usize::from(self.pending.is_some());
        for _ in 0..budget {
            let Some(control) = self.pending.take().or_else(|| self.controls.pop().ok()) else {
                break;
            };
            if control.sequence.get() <= self.last_sequence {
                reject(ControlRejection::OutOfOrder);
                continue;
            }
            if matches!(control.kind, ControlKind::Plan)
                && !matches!(
                    self.plans.begin_block(),
                    PlanSwapResult::Swapped | PlanSwapResult::SwappedAndRetainedOldPlan
                )
            {
                self.pending = Some(control);
                break;
            }
            self.last_sequence = control.sequence.get();
            let Some(plan) = self.plans.active_mut() else {
                reject(ControlRejection::MissingPlan);
                continue;
            };
            if plan.generation != control.generation {
                reject(ControlRejection::GenerationMismatch);
                continue;
            }
            match control.kind {
                ControlKind::Plan => {}
                ControlKind::Gain {
                    target: GainTarget::Master,
                    linear,
                } => plan.processor.set_master_gain_linear(linear),
                ControlKind::Gain {
                    target: GainTarget::Output(index),
                    linear,
                } => {
                    if !plan.processor.set_output_gain_at(index, linear) {
                        reject(ControlRejection::InvalidTarget);
                    }
                }
            }
        }
    }

    pub(super) fn active_mut(&mut self) -> &mut Option<RenderRuntimePlan> {
        self.plans.active_mut()
    }

    pub(super) fn retire(self) -> RealtimePlanRetirement<Option<RenderRuntimePlan>> {
        self.plans.retire()
    }
}

#[cfg(test)]
mod tests;
