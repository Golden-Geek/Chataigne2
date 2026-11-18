//! Clocked app-supplied PCM for the managed playback bus.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::{AudioError, ConfigGeneration, EngineLimits, FrameCount, PlanarBuffer, assert_not_realtime};

/// The next render block and its clock anchor. Frame addresses advance for the life of the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcmRenderCursor {
    pub generation: ConfigGeneration,
    pub next_frame: u64,
    /// Monotonic time at which Golden finished rendering the preceding block.
    pub observed_at: Instant,
    /// Rendered frames waiting in Golden's output bridge, excluding the device buffer.
    pub queued_output_frames: usize,
}

#[derive(Debug)]
struct Clock {
    origin: Instant,
    sequence: AtomicU64,
    frame: AtomicU64,
    generation: AtomicU64,
    nanos: AtomicU64,
    queued_output_frames: AtomicU64,
    epoch: AtomicU64,
}

impl Clock {
    fn publish(&self, generation: ConfigGeneration, frame: u64, queued_output_frames: usize, now: Instant) {
        self.sequence.fetch_add(1, Ordering::SeqCst);
        self.generation.store(generation.get(), Ordering::SeqCst);
        self.frame.store(frame, Ordering::SeqCst);
        self.queued_output_frames
            .store(queued_output_frames as u64, Ordering::SeqCst);
        self.nanos.store(
            u64::try_from(now.saturating_duration_since(self.origin).as_nanos()).unwrap_or(u64::MAX),
            Ordering::SeqCst,
        );
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }

    fn read(&self) -> PcmRenderCursor {
        loop {
            let sequence = self.sequence.load(Ordering::SeqCst);
            let generation = ConfigGeneration::new(self.generation.load(Ordering::SeqCst));
            let next_frame = self.frame.load(Ordering::SeqCst);
            let nanos = self.nanos.load(Ordering::SeqCst);
            let queued_output_frames = self.queued_output_frames.load(Ordering::SeqCst) as usize;
            if sequence.is_multiple_of(2) && sequence == self.sequence.load(Ordering::SeqCst) {
                return PcmRenderCursor {
                    generation,
                    next_frame,
                    observed_at: self.origin + Duration::from_nanos(nanos),
                    queued_output_frames,
                };
            }
            std::hint::spin_loop();
        }
    }
}

#[derive(Debug)]
struct Block {
    generation: ConfigGeneration,
    frame: u64,
    epoch: u64,
    planes: Vec<f32>,
}

/// One producer for a fixed-width playback bus. Supply decoded samples at the engine sample rate.
#[derive(Debug)]
pub struct PcmPlaybackWriter {
    pending: Producer<Block>,
    recycled: Consumer<Block>,
    clock: Arc<Clock>,
    frames: usize,
    samples: usize,
    last: Option<(ConfigGeneration, u64)>,
}

impl PcmPlaybackWriter {
    /// Read the render clock from a non-realtime producer.
    pub fn cursor(&self) -> PcmRenderCursor {
        self.clock.read()
    }

    /// Invalidate queued blocks after a transport discontinuity. The device may have prebuffered
    /// already-rendered samples. They remain bounded by the device bridge.
    pub fn invalidate(&mut self) {
        self.clock.epoch.fetch_add(1, Ordering::Release);
        self.last = None;
    }

    /// Queue one complete planar block addressed to an aligned render frame. Queue frames in
    /// ascending order per generation. `Ok(false)` means the bounded queue is full or disconnected.
    pub fn try_write(&mut self, generation: ConfigGeneration, frame: u64, samples: &[f32]) -> Result<bool, AudioError> {
        if !frame.is_multiple_of(self.frames as u64) || samples.len() != self.samples {
            return Err(AudioError::invalid_configuration(
                "PCM playback block shape or frame is invalid",
            ));
        }
        if self.last.is_some_and(|last| (generation, frame) <= last) {
            return Err(AudioError::invalid_configuration(
                "PCM playback blocks must be submitted in order",
            ));
        }
        let Ok(mut block) = self.recycled.pop() else {
            return Ok(false);
        };
        block.frame = frame;
        block.generation = generation;
        block.epoch = self.clock.epoch.load(Ordering::Acquire);
        for (slot, sample) in block.planes.iter_mut().zip(samples) {
            *slot = if sample.is_finite() { *sample } else { 0.0 };
        }
        // Owning one of the fixed pool's blocks guarantees a free pending slot.
        if self.pending.is_abandoned() {
            return Ok(false);
        }
        self.pending.push(block).expect("PCM block pool has a pending slot");
        self.last = Some((generation, frame));
        Ok(true)
    }
}

/// The render end of the bounded bus. Install once on a managed audio engine.
#[derive(Debug)]
pub struct PcmPlaybackReader {
    pending: Consumer<Block>,
    recycled: Producer<Block>,
    clock: Arc<Clock>,
    ahead: Option<Block>,
    channels: usize,
    frames: usize,
    capacity: usize,
}

impl PcmPlaybackReader {
    pub(crate) fn validate(&self, frames: FrameCount, limits: &EngineLimits) -> Result<(), AudioError> {
        if self.frames != frames.get() as usize || self.channels > usize::from(limits.max_virtual_outputs) {
            return Err(AudioError::invalid_configuration(
                "PCM playback bus does not fit the managed render engine",
            ));
        }
        Ok(())
    }

    pub(crate) fn mix(&mut self, generation: ConfigGeneration, frame: u64, destination: &mut PlanarBuffer) {
        let epoch = self.clock.epoch.load(Ordering::Acquire);
        for _ in 0..self.capacity {
            let Some(block) = self.ahead.take().or_else(|| self.pending.pop().ok()) else {
                break;
            };
            if block.epoch == epoch && block.generation >= generation && block.frame >= frame {
                if block.generation != generation || block.frame != frame {
                    self.ahead = Some(block);
                    break;
                }
                for (channel, source) in block.planes.chunks_exact(self.frames).enumerate() {
                    for (slot, sample) in destination.channel_mut(channel).iter_mut().zip(source) {
                        *slot += sample;
                    }
                }
                self.recycle(block);
                break;
            }
            self.recycle(block);
        }
    }

    fn recycle(&mut self, block: Block) {
        self.recycled.push(block).expect("PCM block pool has a recycle slot");
    }

    pub(crate) fn publish_cursor(
        &self,
        generation: ConfigGeneration,
        next_frame: u64,
        queued_output_frames: usize,
        now: Instant,
    ) {
        self.clock.publish(generation, next_frame, queued_output_frames, now);
    }
}

/// Allocate a fixed pool before rendering. The producer and renderer transfer ownership of each
/// block between two bounded SPSC queues. Rendering does not allocate, lock, or wait for the app.
pub fn pcm_playback_bus(
    channels: u16,
    frames: FrameCount,
    capacity: usize,
    limits: &EngineLimits,
) -> Result<(PcmPlaybackWriter, PcmPlaybackReader), AudioError> {
    assert_not_realtime("PCM playback bus allocation");
    let channels = usize::from(channels);
    let frames = frames.get() as usize;
    if channels == 0 || channels > usize::from(limits.max_virtual_outputs) || !(2..=32).contains(&capacity) {
        return Err(AudioError::invalid_configuration(
            "PCM playback requires a supported channel count and 2–32 blocks",
        ));
    }
    let frames_in_pool = frames.saturating_mul(capacity);
    if frames_in_pool > limits.stream_ring_frames as usize || frames_in_pool.saturating_mul(channels) > 4_194_304 {
        return Err(AudioError::capacity_exceeded(
            "PCM playback block storage exceeds the supported frame budget",
        ));
    }
    let samples = channels * frames;
    let (pending, consumer) = RingBuffer::new(capacity);
    let (mut recycled, free) = RingBuffer::new(capacity);
    for _ in 0..capacity {
        recycled
            .push(Block {
                generation: ConfigGeneration::INITIAL,
                frame: 0,
                epoch: 0,
                planes: vec![0.0; samples],
            })
            .expect("PCM block pool initialization has capacity");
    }
    let clock = Arc::new(Clock {
        origin: Instant::now(),
        sequence: AtomicU64::new(0),
        frame: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        nanos: AtomicU64::new(0),
        queued_output_frames: AtomicU64::new(0),
        epoch: AtomicU64::new(0),
    });
    Ok((
        PcmPlaybackWriter {
            pending,
            recycled: free,
            clock: clock.clone(),
            frames,
            samples,
            last: None,
        },
        PcmPlaybackReader {
            pending: consumer,
            recycled,
            clock,
            ahead: None,
            channels,
            frames,
            capacity,
        },
    ))
}

#[cfg(test)]
mod tests;
