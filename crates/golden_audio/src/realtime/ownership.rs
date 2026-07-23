use std::fmt;

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use crate::{AudioError, VoiceId};

use super::{QueuePressureCounters, assert_not_realtime};

#[derive(Debug)]
pub struct PreparedVoice<T> {
    pub id: VoiceId,
    pub payload: Box<T>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceRetirementReason {
    Completed,
    Requested,
    Replaced,
    Shutdown,
}

#[derive(Debug)]
pub struct RetiredVoice<T> {
    pub id: VoiceId,
    pub reason: VoiceRetirementReason,
    pub payload: Box<T>,
}

#[derive(Debug)]
enum VoiceSlotState<T> {
    Vacant,
    Active(PreparedVoice<T>),
    Retained(RetiredVoice<T>),
}

#[derive(Debug)]
struct VoiceSlot<T> {
    generation: u32,
    state: VoiceSlotState<T>,
}

#[derive(Debug)]
pub struct VoiceSlotController<T> {
    retired_consumer: Consumer<RetiredVoice<T>>,
    next_generations: Vec<u32>,
    pressure: QueuePressureCounters,
}

impl<T> VoiceSlotController<T> {
    pub fn next_id(&mut self, slot: u16) -> Result<VoiceId, AudioError> {
        assert_not_realtime("voice ID allocation");
        let Some(generation) = self.next_generations.get_mut(usize::from(slot)) else {
            return Err(AudioError::capacity_exceeded("voice slot is outside the fixed pool"));
        };
        *generation = generation.wrapping_add(1).max(1);
        Ok(VoiceId::new(slot, *generation))
    }

    pub fn reclaim(&mut self, mut consume: impl FnMut(RetiredVoice<T>)) -> usize {
        assert_not_realtime("voice asset reclamation");
        let mut reclaimed = 0;
        while let Ok(retired) = self.retired_consumer.pop() {
            consume(retired);
            reclaimed += 1;
        }
        reclaimed
    }

    #[must_use]
    pub fn pressure(&self) -> super::QueuePressureSnapshot {
        self.pressure.snapshot()
    }
}

#[derive(Debug)]
pub struct RealtimeVoiceSlots<T> {
    slots: Vec<VoiceSlot<T>>,
    retired_producer: Producer<RetiredVoice<T>>,
    pressure: QueuePressureCounters,
}

impl<T> RealtimeVoiceSlots<T> {
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn activate(&mut self, prepared: PreparedVoice<T>) -> Result<(), PreparedVoice<T>> {
        let slot_index = usize::from(prepared.id.slot());
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return Err(prepared);
        };
        if !matches!(slot.state, VoiceSlotState::Vacant) || prepared.id.generation() <= slot.generation {
            return Err(prepared);
        }
        slot.generation = prepared.id.generation();
        slot.state = VoiceSlotState::Active(prepared);
        Ok(())
    }

    #[must_use]
    pub fn active(&self, id: VoiceId) -> Option<&T> {
        let slot = self.slots.get(usize::from(id.slot()))?;
        match &slot.state {
            VoiceSlotState::Active(active) if active.id == id => Some(active.payload.as_ref()),
            VoiceSlotState::Vacant | VoiceSlotState::Active(_) | VoiceSlotState::Retained(_) => None,
        }
    }

    #[must_use]
    pub fn active_id_at(&self, slot_index: usize) -> Option<VoiceId> {
        let slot = self.slots.get(slot_index)?;
        match &slot.state {
            VoiceSlotState::Active(active) => Some(active.id),
            VoiceSlotState::Vacant | VoiceSlotState::Retained(_) => None,
        }
    }

    pub fn active_mut(&mut self, id: VoiceId) -> Option<&mut T> {
        let slot = self.slots.get_mut(usize::from(id.slot()))?;
        match &mut slot.state {
            VoiceSlotState::Active(active) if active.id == id => Some(active.payload.as_mut()),
            VoiceSlotState::Vacant | VoiceSlotState::Active(_) | VoiceSlotState::Retained(_) => None,
        }
    }

    pub fn retire(&mut self, id: VoiceId, reason: VoiceRetirementReason) -> bool {
        let Some(slot) = self.slots.get_mut(usize::from(id.slot())) else {
            return false;
        };
        let state = std::mem::replace(&mut slot.state, VoiceSlotState::Vacant);
        let VoiceSlotState::Active(active) = state else {
            slot.state = state;
            return false;
        };
        if active.id != id {
            slot.state = VoiceSlotState::Active(active);
            return false;
        }
        let retired = RetiredVoice {
            id,
            reason,
            payload: active.payload,
        };
        if let Err(PushError::Full(retired)) = self.retired_producer.push(retired) {
            slot.state = VoiceSlotState::Retained(retired);
            self.pressure.voice_return_full();
        }
        true
    }

    /// Retries fixed-slot retirements without dropping callback-owned payloads.
    pub fn flush_retired(&mut self) {
        for slot in &mut self.slots {
            let state = std::mem::replace(&mut slot.state, VoiceSlotState::Vacant);
            let VoiceSlotState::Retained(retired) = state else {
                slot.state = state;
                continue;
            };
            if let Err(PushError::Full(retired)) = self.retired_producer.push(retired) {
                slot.state = VoiceSlotState::Retained(retired);
                self.pressure.voice_return_full();
                break;
            }
        }
    }

    /// Transfers the complete fixed pool for destruction on the control thread.
    #[must_use]
    pub fn into_retirement(self) -> RealtimeVoiceRetirement<T> {
        RealtimeVoiceRetirement { slots: self.slots }
    }
}

pub struct RealtimeVoiceRetirement<T> {
    slots: Vec<VoiceSlot<T>>,
}

impl<T> fmt::Debug for RealtimeVoiceRetirement<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RealtimeVoiceRetirement")
            .field("slot_count", &self.slots.len())
            .finish()
    }
}

impl<T> RealtimeVoiceRetirement<T> {
    pub fn reclaim(self) {
        assert_not_realtime("final voice-slot reclamation");
        drop(self.slots);
    }
}

pub fn voice_slot_pool<T>(capacity: u16) -> Result<(VoiceSlotController<T>, RealtimeVoiceSlots<T>), AudioError> {
    if capacity == 0 {
        return Err(AudioError::invalid_configuration(
            "voice slot capacity must be greater than zero",
        ));
    }
    let pressure = QueuePressureCounters::default();
    let (retired_producer, retired_consumer) = RingBuffer::new(usize::from(capacity));
    let slots = (0..capacity)
        .map(|_| VoiceSlot {
            generation: 0,
            state: VoiceSlotState::Vacant,
        })
        .collect();
    Ok((
        VoiceSlotController {
            retired_consumer,
            next_generations: vec![0; usize::from(capacity)],
            pressure: pressure.clone(),
        },
        RealtimeVoiceSlots {
            slots,
            retired_producer,
            pressure,
        },
    ))
}

#[derive(Debug)]
pub struct AnalysisFrame {
    tag: AnalysisFrameTag,
    valid_samples: usize,
    samples: Box<[f32]>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnalysisFrameTag {
    pub topology_generation: u64,
    pub tap_index: u16,
    pub render_frame: u64,
}

impl AnalysisFrame {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.tag.render_frame
    }

    #[must_use]
    pub const fn tag(&self) -> AnalysisFrameTag {
        self.tag
    }

    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples[..self.valid_samples]
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.samples.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisCaptureError {
    FrameTooLarge,
    NoFreeFrame,
    ReadyQueueFull,
}

#[derive(Debug)]
pub struct AnalysisRecycleError(pub Box<AnalysisFrame>);

#[derive(Debug)]
pub struct AnalysisFrameWriter {
    free_consumer: Consumer<Box<AnalysisFrame>>,
    ready_producer: Producer<Box<AnalysisFrame>>,
    retained_free: Option<Box<AnalysisFrame>>,
    retained_ready: Option<Box<AnalysisFrame>>,
    pressure: QueuePressureCounters,
}

impl AnalysisFrameWriter {
    pub fn capture(&mut self, sequence: u64, samples: &[f32]) -> Result<(), AnalysisCaptureError> {
        self.capture_tagged(
            AnalysisFrameTag {
                render_frame: sequence,
                ..AnalysisFrameTag::default()
            },
            samples,
        )
    }

    pub fn capture_tagged(&mut self, tag: AnalysisFrameTag, samples: &[f32]) -> Result<(), AnalysisCaptureError> {
        self.capture_tagged_with(tag, samples.len(), |destination| destination.copy_from_slice(samples))
    }

    pub fn capture_tagged_with(
        &mut self,
        tag: AnalysisFrameTag,
        sample_count: usize,
        fill: impl FnOnce(&mut [f32]),
    ) -> Result<(), AnalysisCaptureError> {
        if let Some(mut frame) = self.retained_ready.take() {
            if sample_count > frame.samples.len() {
                self.retained_ready = Some(frame);
                return Err(AnalysisCaptureError::FrameTooLarge);
            }
            frame.tag = tag;
            frame.valid_samples = sample_count;
            fill(&mut frame.samples[..sample_count]);
            return match self.ready_producer.push(frame) {
                Ok(()) => Ok(()),
                Err(PushError::Full(frame)) => {
                    self.retained_ready = Some(frame);
                    self.pressure.analysis_ready_full();
                    Err(AnalysisCaptureError::ReadyQueueFull)
                }
            };
        }
        let mut frame = if let Some(frame) = self.retained_free.take() {
            frame
        } else {
            let Ok(frame) = self.free_consumer.pop() else {
                self.pressure.analysis_free_empty();
                return Err(AnalysisCaptureError::NoFreeFrame);
            };
            frame
        };
        if sample_count > frame.samples.len() {
            frame.valid_samples = 0;
            self.retained_free = Some(frame);
            return Err(AnalysisCaptureError::FrameTooLarge);
        }
        frame.tag = tag;
        frame.valid_samples = sample_count;
        fill(&mut frame.samples[..sample_count]);
        match self.ready_producer.push(frame) {
            Ok(()) => Ok(()),
            Err(PushError::Full(frame)) => {
                self.retained_ready = Some(frame);
                self.pressure.analysis_ready_full();
                Err(AnalysisCaptureError::ReadyQueueFull)
            }
        }
    }

    /// Publishes a newest retained frame after worker-side capacity becomes available.
    pub fn flush_retained(&mut self) -> bool {
        let Some(frame) = self.retained_ready.take() else {
            return true;
        };
        match self.ready_producer.push(frame) {
            Ok(()) => true,
            Err(PushError::Full(frame)) => {
                self.retained_ready = Some(frame);
                false
            }
        }
    }

    #[must_use]
    pub fn into_retirement(self) -> AnalysisWriterRetirement {
        AnalysisWriterRetirement {
            free_consumer: self.free_consumer,
            ready_producer: self.ready_producer,
            retained_free: self.retained_free,
            retained_ready: self.retained_ready,
            pressure: self.pressure,
        }
    }
}

#[derive(Debug)]
pub struct AnalysisWriterRetirement {
    free_consumer: Consumer<Box<AnalysisFrame>>,
    ready_producer: Producer<Box<AnalysisFrame>>,
    retained_free: Option<Box<AnalysisFrame>>,
    retained_ready: Option<Box<AnalysisFrame>>,
    pressure: QueuePressureCounters,
}

impl AnalysisWriterRetirement {
    pub fn reclaim(self) {
        assert_not_realtime("final analysis-frame reclamation");
        let Self {
            free_consumer,
            ready_producer,
            retained_free,
            retained_ready,
            pressure,
        } = self;
        drop((free_consumer, ready_producer, retained_free, retained_ready, pressure));
    }
}

#[derive(Debug)]
pub struct AnalysisFrameReader {
    ready_consumer: Consumer<Box<AnalysisFrame>>,
    free_producer: Producer<Box<AnalysisFrame>>,
    pressure: QueuePressureCounters,
}

impl AnalysisFrameReader {
    #[must_use]
    pub fn try_recv(&mut self) -> Option<Box<AnalysisFrame>> {
        assert_not_realtime("analysis-frame consumption");
        self.ready_consumer.pop().ok()
    }

    pub fn recycle(&mut self, mut frame: Box<AnalysisFrame>) -> Result<(), AnalysisRecycleError> {
        assert_not_realtime("analysis-frame recycling");
        frame.valid_samples = 0;
        self.free_producer
            .push(frame)
            .map_err(|PushError::Full(frame)| AnalysisRecycleError(frame))
    }

    #[must_use]
    pub fn pressure(&self) -> super::QueuePressureSnapshot {
        self.pressure.snapshot()
    }
}

pub fn analysis_frame_pool(
    slot_count: usize,
    frame_capacity: usize,
) -> Result<(AnalysisFrameReader, AnalysisFrameWriter), AudioError> {
    if slot_count == 0 || frame_capacity == 0 {
        return Err(AudioError::invalid_configuration(
            "analysis frame slot count and capacity must be greater than zero",
        ));
    }
    let pressure = QueuePressureCounters::default();
    let allocated_frames = slot_count
        .checked_add(1)
        .ok_or_else(|| AudioError::capacity_exceeded("analysis frame slot count overflowed"))?;
    let (mut free_producer, free_consumer) = RingBuffer::new(allocated_frames);
    let (ready_producer, ready_consumer) = RingBuffer::new(slot_count);
    for _ in 0..allocated_frames {
        free_producer
            .push(Box::new(AnalysisFrame {
                tag: AnalysisFrameTag::default(),
                valid_samples: 0,
                samples: vec![0.0; frame_capacity].into_boxed_slice(),
            }))
            .expect("analysis free queue was sized for all preallocated frames");
    }
    Ok((
        AnalysisFrameReader {
            ready_consumer,
            free_producer,
            pressure: pressure.clone(),
        },
        AnalysisFrameWriter {
            free_consumer,
            ready_producer,
            retained_free: None,
            retained_ready: None,
            pressure,
        },
    ))
}
