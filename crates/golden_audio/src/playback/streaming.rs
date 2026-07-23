use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::AudioError;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StreamPlaybackState {
    pub fill_frames: usize,
    pub written_frames: u64,
    pub read_frames: u64,
    pub starvation_count: u64,
    pub end_of_file: bool,
    pub cancelled: bool,
    pub failed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamWriteError {
    InvalidShape,
    Full,
    ReaderDisconnected,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StreamWriteResult {
    pub written_frames: usize,
}

#[derive(Debug)]
struct SharedStreamState {
    channels: usize,
    capacity_samples: usize,
    fill_frames: AtomicUsize,
    written_frames: AtomicU64,
    read_frames: AtomicU64,
    starvation_count: AtomicU64,
    end_of_file: AtomicBool,
    cancelled: AtomicBool,
    failed: AtomicBool,
}

impl SharedStreamState {
    fn snapshot(&self) -> StreamPlaybackState {
        StreamPlaybackState {
            fill_frames: self.fill_frames.load(Ordering::Acquire),
            written_frames: self.written_frames.load(Ordering::Relaxed),
            read_frames: self.read_frames.load(Ordering::Relaxed),
            starvation_count: self.starvation_count.load(Ordering::Relaxed),
            end_of_file: self.end_of_file.load(Ordering::Acquire),
            cancelled: self.cancelled.load(Ordering::Acquire),
            failed: self.failed.load(Ordering::Acquire),
        }
    }
}

#[derive(Debug)]
pub struct StreamPlaybackWriter {
    producer: Producer<f32>,
    shared: Arc<SharedStreamState>,
}

impl StreamPlaybackWriter {
    pub fn write_interleaved(&mut self, samples: &[f32]) -> Result<StreamWriteResult, StreamWriteError> {
        if samples.is_empty() || !samples.len().is_multiple_of(self.shared.channels) {
            return Err(StreamWriteError::InvalidShape);
        }
        if self.shared.cancelled.load(Ordering::Acquire) {
            return Err(StreamWriteError::Cancelled);
        }
        if self.producer.is_abandoned() {
            return Err(StreamWriteError::ReaderDisconnected);
        }
        if self.producer.slots() < samples.len() {
            return Err(StreamWriteError::Full);
        }
        self.producer
            .push_entire_slice(samples)
            .map_err(|_| StreamWriteError::Full)?;
        let frames = samples.len() / self.shared.channels;
        self.shared
            .written_frames
            .fetch_add(u64::try_from(frames).unwrap_or(u64::MAX), Ordering::Relaxed);
        self.update_fill();
        Ok(StreamWriteResult { written_frames: frames })
    }

    pub fn finish(&self) {
        self.shared.end_of_file.store(true, Ordering::Release);
    }

    pub fn fail(&self) {
        self.shared.failed.store(true, Ordering::Release);
        self.shared.end_of_file.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.shared.cancelled.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn is_reader_connected(&self) -> bool {
        !self.producer.is_abandoned()
    }

    #[must_use]
    pub fn writable_frames(&self) -> usize {
        self.producer.slots() / self.shared.channels
    }

    #[must_use]
    pub fn state(&self) -> StreamPlaybackState {
        self.shared.snapshot()
    }

    fn update_fill(&self) {
        let fill_samples = self.shared.capacity_samples.saturating_sub(self.producer.slots());
        self.shared
            .fill_frames
            .store(fill_samples / self.shared.channels, Ordering::Release);
    }
}

#[derive(Debug)]
pub struct StreamPlaybackReader {
    consumer: Consumer<f32>,
    shared: Arc<SharedStreamState>,
}

impl StreamPlaybackReader {
    /// Copies complete interleaved frames and writes silence for any unavailable tail.
    pub fn read_interleaved(&mut self, destination: &mut [f32]) -> usize {
        if destination.is_empty() || !destination.len().is_multiple_of(self.shared.channels) {
            return 0;
        }
        let requested_frames = destination.len() / self.shared.channels;
        let available_frames = self.consumer.slots() / self.shared.channels;
        let copied_frames = requested_frames.min(available_frames);
        let copied_samples = copied_frames * self.shared.channels;
        if copied_samples > 0 {
            let _ = self.consumer.pop_entire_slice(&mut destination[..copied_samples]);
        }
        destination[copied_samples..].fill(0.0);
        if copied_frames < requested_frames && !self.shared.end_of_file.load(Ordering::Acquire) {
            self.shared.starvation_count.fetch_add(1, Ordering::Relaxed);
        }
        self.shared
            .read_frames
            .fetch_add(u64::try_from(copied_frames).unwrap_or(u64::MAX), Ordering::Relaxed);
        self.update_fill();
        copied_frames
    }

    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.shared.end_of_file.load(Ordering::Acquire) && self.consumer.slots() == 0
    }

    #[must_use]
    pub fn state(&self) -> StreamPlaybackState {
        self.shared.snapshot()
    }

    fn update_fill(&self) {
        self.shared
            .fill_frames
            .store(self.consumer.slots() / self.shared.channels, Ordering::Release);
    }
}

pub fn streaming_playback_ring(
    channels: u16,
    capacity_frames: usize,
) -> Result<(StreamPlaybackWriter, StreamPlaybackReader), AudioError> {
    if channels == 0 || capacity_frames == 0 {
        return Err(AudioError::invalid_configuration(
            "stream playback ring requires positive channels and frame capacity",
        ));
    }
    let channels = usize::from(channels);
    let capacity_samples = channels
        .checked_mul(capacity_frames)
        .ok_or_else(|| AudioError::capacity_exceeded("stream playback sample capacity overflowed"))?;
    let (producer, consumer) = RingBuffer::new(capacity_samples);
    let shared = Arc::new(SharedStreamState {
        channels,
        capacity_samples,
        fill_frames: AtomicUsize::new(0),
        written_frames: AtomicU64::new(0),
        read_frames: AtomicU64::new(0),
        starvation_count: AtomicU64::new(0),
        end_of_file: AtomicBool::new(false),
        cancelled: AtomicBool::new(false),
        failed: AtomicBool::new(false),
    });
    Ok((
        StreamPlaybackWriter {
            producer,
            shared: Arc::clone(&shared),
        },
        StreamPlaybackReader { consumer, shared },
    ))
}
