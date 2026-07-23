use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering},
};

use rtrb::{Consumer, Producer, RingBuffer};
use rubato::{
    Adjustable, Async, FixedAsync, PolynomialDegree, Resampler, audioadapter_buffers::direct::InterleavedSlice,
};

use crate::{AudioError, FrameCount, PlanarBuffer, SampleRate, assert_not_realtime};

use super::DriftController;
use super::drift::DriftControllerConfig;

const MAX_RELATIVE_RESAMPLE_RATIO: f64 = 1.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockBridgeConfig {
    pub device_sample_rate: SampleRate,
    pub engine_sample_rate: SampleRate,
    pub channels: u16,
    pub engine_block_frames: FrameCount,
    pub ring_capacity_frames: usize,
    pub output_buffer_frames: u32,
    pub drift: DriftControllerConfig,
}

impl ClockBridgeConfig {
    pub fn validate(self) -> Result<(), AudioError> {
        if self.channels == 0 {
            return Err(AudioError::invalid_configuration(
                "input clock bridge channel count must be greater than zero",
            ));
        }
        if self.ring_capacity_frames < self.drift.target_fill_frames.saturating_mul(2) {
            return Err(AudioError::invalid_configuration(
                "input ring capacity must be at least twice the target fill",
            ));
        }
        if self.output_buffer_frames == 0 {
            return Err(AudioError::invalid_configuration(
                "output latency buffer must be greater than zero",
            ));
        }
        self.drift.validate()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClockBridgeObservation {
    pub fill_frames: usize,
    pub correction_ppm: f64,
    pub underflow_count: u64,
    pub overflow_count: u64,
    pub discontinuity_count: u64,
    pub timestamp_loss_count: u64,
    pub estimated_latency_ms: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputWriteError {
    InvalidShape,
    Overflow,
    ReaderDisconnected,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputWriteResult {
    pub written_frames: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputReadError {
    InvalidDestination,
    ResamplerFailure,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputReadResult {
    pub rendered_frames: usize,
    pub underflowed: bool,
}

#[derive(Debug)]
struct SharedObservation {
    fill_frames: AtomicUsize,
    correction_ppm_bits: AtomicU64,
    underflow_count: AtomicU64,
    overflow_count: AtomicU64,
    discontinuity_count: AtomicU64,
    timestamp_loss_count: AtomicU64,
    device_sample_rate: AtomicU32,
    resampler_delay_frames: AtomicUsize,
    output_buffer_frames: u32,
    engine_sample_rate: SampleRate,
}

impl SharedObservation {
    fn snapshot(&self) -> ClockBridgeObservation {
        let fill_frames = self.fill_frames.load(Ordering::Acquire);
        let device_sample_rate = self.device_sample_rate.load(Ordering::Acquire);
        let resampler_delay_frames = self.resampler_delay_frames.load(Ordering::Acquire);
        let input_latency_ms = fill_frames as f64 * 1_000.0 / f64::from(device_sample_rate.max(1));
        let engine_latency_ms = (resampler_delay_frames as f64 + f64::from(self.output_buffer_frames)) * 1_000.0
            / f64::from(self.engine_sample_rate.get());
        ClockBridgeObservation {
            fill_frames,
            correction_ppm: f64::from_bits(self.correction_ppm_bits.load(Ordering::Acquire)),
            underflow_count: self.underflow_count.load(Ordering::Relaxed),
            overflow_count: self.overflow_count.load(Ordering::Relaxed),
            discontinuity_count: self.discontinuity_count.load(Ordering::Relaxed),
            timestamp_loss_count: self.timestamp_loss_count.load(Ordering::Relaxed),
            estimated_latency_ms: input_latency_ms + engine_latency_ms,
        }
    }
}

#[derive(Debug)]
pub struct InputClockWriter {
    producer: Producer<f32>,
    shared: Arc<SharedObservation>,
    channels: usize,
    capacity_samples: usize,
    last_device_nanos: Option<u128>,
    last_frames: usize,
}

impl InputClockWriter {
    pub fn write_interleaved(
        &mut self,
        samples: &[f32],
        device_timestamp_nanos: Option<u128>,
    ) -> Result<InputWriteResult, InputWriteError> {
        if samples.is_empty() || !samples.len().is_multiple_of(self.channels) {
            return Err(InputWriteError::InvalidShape);
        }
        self.observe_timestamp(device_timestamp_nanos);
        if self.producer.is_abandoned() {
            return Err(InputWriteError::ReaderDisconnected);
        }
        if self.producer.slots() < samples.len() {
            self.shared.overflow_count.fetch_add(1, Ordering::Relaxed);
            return Err(InputWriteError::Overflow);
        }
        self.producer
            .push_entire_slice(samples)
            .map_err(|_| InputWriteError::Overflow)?;
        let fill_samples = self.capacity_samples.saturating_sub(self.producer.slots());
        self.shared
            .fill_frames
            .store(fill_samples / self.channels, Ordering::Release);
        let frames = samples.len() / self.channels;
        self.last_frames = frames;
        Ok(InputWriteResult { written_frames: frames })
    }

    #[must_use]
    pub fn observation(&self) -> ClockBridgeObservation {
        self.shared.snapshot()
    }

    fn observe_timestamp(&mut self, timestamp: Option<u128>) {
        let Some(timestamp) = timestamp else {
            self.shared.timestamp_loss_count.fetch_add(1, Ordering::Relaxed);
            self.last_device_nanos = None;
            return;
        };
        if let Some(last) = self.last_device_nanos {
            let rate = u128::from(self.shared.device_sample_rate.load(Ordering::Acquire).max(1));
            let expected = u128::try_from(self.last_frames).unwrap_or(u128::MAX) * 1_000_000_000 / rate;
            let actual = timestamp.saturating_sub(last);
            let tolerance = (2_000_000_000 / rate).max(1_000);
            if actual.abs_diff(expected) > tolerance {
                self.shared.discontinuity_count.fetch_add(1, Ordering::Relaxed);
            }
        }
        self.last_device_nanos = Some(timestamp);
    }
}

#[derive(Debug)]
pub struct InputClockReader {
    consumer: Consumer<f32>,
    shared: Arc<SharedObservation>,
    config: ClockBridgeConfig,
    resampler: Async<f32>,
    controller: DriftController,
    input_scratch: Vec<f32>,
    output_scratch: Vec<f32>,
}

impl InputClockReader {
    pub fn read_engine_block(&mut self, destination: &mut PlanarBuffer) -> Result<InputReadResult, InputReadError> {
        let channels = usize::from(self.config.channels);
        let output_frames = self.config.engine_block_frames.get() as usize;
        if destination.channels() < channels || destination.frame_capacity() < output_frames {
            return Err(InputReadError::InvalidDestination);
        }
        let fill_frames = self.consumer.slots() / channels;
        let correction_ppm = self.controller.observe_fill(fill_frames);
        self.shared
            .correction_ppm_bits
            .store(correction_ppm.to_bits(), Ordering::Release);
        let relative_ratio = 1.0 - correction_ppm / 1_000_000.0;
        self.resampler
            .set_resample_ratio_relative(relative_ratio, true)
            .map_err(|_| InputReadError::ResamplerFailure)?;
        let needed_input_frames = self.resampler.input_frames_next();
        let needed_samples = needed_input_frames.saturating_mul(channels);
        if self.consumer.slots() < needed_samples {
            destination
                .zero_range(0, output_frames)
                .map_err(|_| InputReadError::InvalidDestination)?;
            self.shared.underflow_count.fetch_add(1, Ordering::Relaxed);
            self.resampler.reset();
            self.update_fill();
            return Ok(InputReadResult {
                rendered_frames: output_frames,
                underflowed: true,
            });
        }
        let input = self
            .input_scratch
            .get_mut(..needed_samples)
            .ok_or(InputReadError::ResamplerFailure)?;
        self.consumer
            .pop_entire_slice(input)
            .map_err(|_| InputReadError::ResamplerFailure)?;
        let input_adapter = InterleavedSlice::new(input, channels, needed_input_frames)
            .map_err(|_| InputReadError::ResamplerFailure)?;
        let output_capacity = self.output_scratch.len() / channels;
        let mut output_adapter = InterleavedSlice::new_mut(&mut self.output_scratch, channels, output_capacity)
            .map_err(|_| InputReadError::ResamplerFailure)?;
        let (_, rendered_frames) = self
            .resampler
            .process_into_buffer(&input_adapter, &mut output_adapter, None)
            .map_err(|_| InputReadError::ResamplerFailure)?;
        let copied_frames = rendered_frames.min(output_frames);
        for channel in 0..channels {
            let destination_channel = destination.channel_mut(channel);
            for (frame, sample) in destination_channel.iter_mut().take(copied_frames).enumerate() {
                *sample = self.output_scratch[frame * channels + channel];
            }
            destination_channel[copied_frames..output_frames].fill(0.0);
        }
        self.update_fill();
        Ok(InputReadResult {
            rendered_frames: copied_frames,
            underflowed: false,
        })
    }

    pub fn reconfigure_device_rate(&mut self, device_sample_rate: SampleRate) -> Result<(), AudioError> {
        assert_not_realtime("input clock bridge device-rate reconfiguration");
        self.config.device_sample_rate = device_sample_rate;
        self.resampler = build_resampler(self.config)?;
        self.input_scratch.resize(
            self.resampler.input_frames_max() * usize::from(self.config.channels),
            0.0,
        );
        self.output_scratch.resize(
            self.resampler.output_frames_max() * usize::from(self.config.channels),
            0.0,
        );
        while self.consumer.pop().is_ok() {}
        self.controller.reset();
        self.shared
            .device_sample_rate
            .store(device_sample_rate.get(), Ordering::Release);
        self.shared
            .resampler_delay_frames
            .store(self.resampler.output_delay(), Ordering::Release);
        self.shared.discontinuity_count.fetch_add(1, Ordering::Relaxed);
        self.update_fill();
        Ok(())
    }

    #[must_use]
    pub fn observation(&self) -> ClockBridgeObservation {
        self.shared.snapshot()
    }

    #[must_use]
    pub const fn config(&self) -> ClockBridgeConfig {
        self.config
    }

    fn update_fill(&self) {
        let fill_frames = self.consumer.slots() / usize::from(self.config.channels);
        self.shared.fill_frames.store(fill_frames, Ordering::Release);
    }
}

pub fn input_clock_bridge(config: ClockBridgeConfig) -> Result<(InputClockWriter, InputClockReader), AudioError> {
    config.validate()?;
    let channels = usize::from(config.channels);
    let capacity_samples = config
        .ring_capacity_frames
        .checked_mul(channels)
        .ok_or_else(|| AudioError::capacity_exceeded("input ring sample capacity overflowed"))?;
    let (producer, consumer) = RingBuffer::new(capacity_samples);
    let resampler = build_resampler(config)?;
    let input_scratch = vec![0.0; resampler.input_frames_max() * channels];
    let output_scratch = vec![0.0; resampler.output_frames_max() * channels];
    let shared = Arc::new(SharedObservation {
        fill_frames: AtomicUsize::new(0),
        correction_ppm_bits: AtomicU64::new(0.0_f64.to_bits()),
        underflow_count: AtomicU64::new(0),
        overflow_count: AtomicU64::new(0),
        discontinuity_count: AtomicU64::new(0),
        timestamp_loss_count: AtomicU64::new(0),
        device_sample_rate: AtomicU32::new(config.device_sample_rate.get()),
        resampler_delay_frames: AtomicUsize::new(resampler.output_delay()),
        output_buffer_frames: config.output_buffer_frames,
        engine_sample_rate: config.engine_sample_rate,
    });
    Ok((
        InputClockWriter {
            producer,
            shared: Arc::clone(&shared),
            channels,
            capacity_samples,
            last_device_nanos: None,
            last_frames: 0,
        },
        InputClockReader {
            consumer,
            shared,
            config,
            resampler,
            controller: DriftController::new(config.drift)?,
            input_scratch,
            output_scratch,
        },
    ))
}

fn build_resampler(config: ClockBridgeConfig) -> Result<Async<f32>, AudioError> {
    let ratio = f64::from(config.engine_sample_rate.get()) / f64::from(config.device_sample_rate.get());
    Async::new_poly(
        ratio,
        MAX_RELATIVE_RESAMPLE_RATIO,
        PolynomialDegree::Septic,
        config.engine_block_frames.get() as usize,
        usize::from(config.channels),
        FixedAsync::Output,
    )
    .map_err(|error| AudioError::invalid_configuration(format!("failed to create input resampler: {error}")))
}
