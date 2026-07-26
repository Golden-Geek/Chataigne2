use std::{
    collections::HashMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    AnalysisFrameReader, AnalysisFrameTag, AnalysisFrameWriter, AnalysisProcessorConfiguration, AnalysisTapId,
    AudioChannelId, AudioError, AudioErrorCategory, ConfigGeneration, EngineLimits, PlanarBuffer, RenderPlan,
    analysis_frame_pool, assert_not_realtime,
};

use super::{
    AnalysisDiagnosticsObservation, AnalysisObservationSnapshot, AnalysisResult, AnalysisTapObservation,
    ChannelObservation, MeterAccumulator, PitchAnalyzer, SpectrumAnalyzer,
};

// Covers short host-scheduler stalls while keeping overload memory strictly bounded.
const BUFFERED_FRAMES_PER_ANALYSIS_TAP: usize = 4;

#[derive(Debug)]
struct AtomicChannelObservation {
    rms_linear: AtomicU32,
    rms_dbfs: AtomicU32,
    peak_dbfs: AtomicU32,
    clipped: AtomicBool,
}

impl AtomicChannelObservation {
    fn new() -> Self {
        Self {
            rms_linear: AtomicU32::new(0.0_f32.to_bits()),
            rms_dbfs: AtomicU32::new(super::rms::DBFS_FLOOR.to_bits()),
            peak_dbfs: AtomicU32::new(super::rms::DBFS_FLOOR.to_bits()),
            clipped: AtomicBool::new(false),
        }
    }
}

#[derive(Debug)]
struct SharedMeterBank {
    channels: Vec<AudioChannelId>,
    values: Vec<AtomicChannelObservation>,
    epoch: AtomicU64,
    render_frame: AtomicU64,
}

impl SharedMeterBank {
    fn new(channels: Vec<AudioChannelId>) -> Self {
        Self {
            values: channels.iter().map(|_| AtomicChannelObservation::new()).collect(),
            channels,
            epoch: AtomicU64::new(0),
            render_frame: AtomicU64::new(0),
        }
    }

    fn publish(&self, observations: &[ChannelObservation], render_frame: u64) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        for (destination, source) in self.values.iter().zip(observations) {
            destination
                .rms_linear
                .store(source.rms_linear.to_bits(), Ordering::Relaxed);
            destination.rms_dbfs.store(source.rms_dbfs.to_bits(), Ordering::Relaxed);
            destination
                .peak_dbfs
                .store(source.peak_dbfs.to_bits(), Ordering::Relaxed);
            destination.clipped.store(source.clipped, Ordering::Relaxed);
        }
        self.render_frame.store(render_frame, Ordering::Relaxed);
        self.epoch.fetch_add(1, Ordering::Release);
    }

    fn snapshot(&self) -> (u64, Vec<ChannelObservation>) {
        loop {
            let before = self.epoch.load(Ordering::Acquire);
            if !before.is_multiple_of(2) {
                std::hint::spin_loop();
                continue;
            }
            let render_frame = self.render_frame.load(Ordering::Relaxed);
            let observations = self
                .channels
                .iter()
                .zip(&self.values)
                .map(|(channel, value)| ChannelObservation {
                    channel: *channel,
                    rms_linear: f32::from_bits(value.rms_linear.load(Ordering::Relaxed)),
                    rms_dbfs: f32::from_bits(value.rms_dbfs.load(Ordering::Relaxed)),
                    peak_dbfs: f32::from_bits(value.peak_dbfs.load(Ordering::Relaxed)),
                    clipped: value.clipped.load(Ordering::Relaxed),
                })
                .collect();
            let after = self.epoch.load(Ordering::Acquire);
            if before == after {
                return (render_frame, observations);
            }
        }
    }
}

#[derive(Debug, Default)]
struct SharedDiagnostics {
    captured_frames: AtomicU64,
    processed_frames: AtomicU64,
    dropped_frames: AtomicU64,
    stale_frames: AtomicU64,
    worker_time_micros: AtomicU64,
    maximum_worker_time_micros: AtomicU64,
}

impl SharedDiagnostics {
    fn snapshot(&self) -> AnalysisDiagnosticsObservation {
        AnalysisDiagnosticsObservation {
            captured_frames: self.captured_frames.load(Ordering::Relaxed),
            processed_frames: self.processed_frames.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            stale_frames: self.stale_frames.load(Ordering::Relaxed),
            worker_time_micros: self.worker_time_micros.load(Ordering::Relaxed),
            maximum_worker_time_micros: self.maximum_worker_time_micros.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
struct SharedAnalysis {
    generation: ConfigGeneration,
    input_meters: SharedMeterBank,
    output_meters: SharedMeterBank,
    taps: RwLock<Vec<AnalysisTapObservation>>,
    diagnostics: SharedDiagnostics,
    shutdown: AtomicBool,
}

#[derive(Clone, Debug)]
pub struct AnalysisObservationReader {
    shared: Arc<SharedAnalysis>,
}

impl AnalysisObservationReader {
    #[must_use]
    pub fn latest(&self) -> AnalysisObservationSnapshot {
        let (input_frame, inputs) = self.shared.input_meters.snapshot();
        let (output_frame, outputs) = self.shared.output_meters.snapshot();
        let input_global_max_rms = maximum_rms(&inputs);
        let output_global_max_rms = maximum_rms(&outputs);
        AnalysisObservationSnapshot {
            generation: self.shared.generation,
            render_frame: input_frame.max(output_frame),
            inputs,
            outputs,
            input_global_max_rms,
            output_global_max_rms,
            global_max_rms: input_global_max_rms.max(output_global_max_rms),
            taps: self.shared.taps.read().map_or_else(|_| Vec::new(), |taps| taps.clone()),
            diagnostics: self.shared.diagnostics.snapshot(),
        }
    }
}

fn maximum_rms(observations: &[ChannelObservation]) -> f32 {
    observations
        .iter()
        .map(|observation| observation.rms_linear)
        .fold(0.0, f32::max)
}

#[derive(Debug)]
pub struct AnalysisController {
    shared: Arc<SharedAnalysis>,
    enabled: HashMap<AnalysisTapId, Arc<AtomicBool>>,
    worker: Option<JoinHandle<()>>,
}

impl AnalysisController {
    #[must_use]
    pub fn observations(&self) -> AnalysisObservationReader {
        AnalysisObservationReader {
            shared: Arc::clone(&self.shared),
        }
    }

    pub fn set_tap_enabled(&self, tap: AnalysisTapId, enabled: bool) -> Result<(), AudioError> {
        assert_not_realtime("analysis tap enablement");
        let Some(state) = self.enabled.get(&tap) else {
            return Err(AudioError::new(
                AudioErrorCategory::InvalidConfiguration,
                format!("analysis topology does not contain tap {tap}"),
            ));
        };
        state.store(enabled, Ordering::Release);
        if let Ok(mut observations) = self.shared.taps.write()
            && let Some(observation) = observations.iter_mut().find(|observation| observation.tap == tap)
        {
            observation.enabled = enabled;
            if !enabled {
                observation.result = None;
            }
        }
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), AudioError> {
        if self.worker.is_none() {
            return Ok(());
        }
        self.shared.shutdown.store(true, Ordering::Release);
        self.worker.take().unwrap().join().map_err(|_| {
            AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "analysis worker panicked during shutdown",
            )
        })
    }
}

impl Drop for AnalysisController {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[derive(Debug)]
struct TapCapture {
    tap_index: u16,
    source_index: usize,
    enabled: Arc<AtomicBool>,
    hop_frames: usize,
    samples: Vec<f32>,
    write_position: usize,
    filled_samples: usize,
    samples_since_emit: usize,
    emitted_once: bool,
}

impl TapCapture {
    fn capture(
        &mut self,
        source: &[f32],
        topology_generation: u64,
        render_frame: u64,
        writer: &mut AnalysisFrameWriter,
        diagnostics: &SharedDiagnostics,
    ) {
        if !self.enabled.load(Ordering::Acquire) {
            self.write_position = 0;
            self.filled_samples = 0;
            self.samples_since_emit = 0;
            self.emitted_once = false;
            return;
        }
        for (offset, sample) in source.iter().enumerate() {
            self.samples[self.write_position] = if sample.is_finite() { *sample } else { 0.0 };
            self.write_position = (self.write_position + 1) % self.samples.len();
            self.filled_samples = (self.filled_samples + 1).min(self.samples.len());
            self.samples_since_emit += 1;
            let ready = self.filled_samples == self.samples.len()
                && (!self.emitted_once || self.samples_since_emit >= self.hop_frames);
            if !ready {
                continue;
            }
            let split = self.write_position;
            let tail = self.samples.len() - split;
            let result = writer.capture_tagged_with(
                AnalysisFrameTag {
                    topology_generation,
                    tap_index: self.tap_index,
                    render_frame: render_frame.saturating_add(offset as u64 + 1),
                },
                self.samples.len(),
                |destination| {
                    destination[..tail].copy_from_slice(&self.samples[split..]);
                    destination[tail..].copy_from_slice(&self.samples[..split]);
                },
            );
            if result.is_ok() {
                diagnostics.captured_frames.fetch_add(1, Ordering::Relaxed);
            } else {
                diagnostics.dropped_frames.fetch_add(1, Ordering::Relaxed);
            }
            self.emitted_once = true;
            self.samples_since_emit = 0;
        }
    }
}

#[derive(Debug)]
pub struct AnalysisRenderer {
    generation: ConfigGeneration,
    input_channels: Vec<AudioChannelId>,
    output_channels: Vec<AudioChannelId>,
    input_meters: MeterAccumulator,
    output_meters: MeterAccumulator,
    observation_interval_frames: usize,
    input_frames_since_publish: usize,
    output_frames_since_publish: usize,
    taps: Vec<TapCapture>,
    writer: AnalysisFrameWriter,
    shared: Arc<SharedAnalysis>,
}

impl AnalysisRenderer {
    pub fn capture_inputs(
        &mut self,
        inputs: &PlanarBuffer,
        frames: usize,
        render_frame: u64,
    ) -> Result<(), AudioError> {
        self.writer.flush_retained();
        self.input_meters.accumulate(inputs, frames, |_| {})?;
        self.input_frames_since_publish = self.input_frames_since_publish.saturating_add(frames);
        if self.input_frames_since_publish >= self.observation_interval_frames {
            self.input_frames_since_publish %= self.observation_interval_frames;
            self.shared
                .input_meters
                .publish(self.input_meters.latest(), render_frame.saturating_add(frames as u64));
        }
        for tap in &mut self.taps {
            tap.capture(
                &inputs.channel(tap.source_index)[..frames],
                self.generation.get(),
                render_frame,
                &mut self.writer,
                &self.shared.diagnostics,
            );
        }
        Ok(())
    }

    pub fn capture_outputs(
        &mut self,
        outputs: &PlanarBuffer,
        frames: usize,
        render_frame: u64,
    ) -> Result<(), AudioError> {
        self.output_meters.accumulate(outputs, frames, |_| {})?;
        self.output_frames_since_publish = self.output_frames_since_publish.saturating_add(frames);
        if self.output_frames_since_publish >= self.observation_interval_frames {
            self.output_frames_since_publish %= self.observation_interval_frames;
            self.shared
                .output_meters
                .publish(self.output_meters.latest(), render_frame.saturating_add(frames as u64));
        }
        Ok(())
    }

    #[must_use]
    pub fn matches_plan(&self, plan: &RenderPlan) -> bool {
        self.input_channels == plan.virtual_inputs && self.output_channels == plan.virtual_outputs
    }

    #[must_use]
    pub fn into_retirement(self) -> AnalysisRendererRetirement {
        AnalysisRendererRetirement {
            input_channels: self.input_channels,
            output_channels: self.output_channels,
            input_meters: self.input_meters,
            output_meters: self.output_meters,
            taps: self.taps,
            writer: self.writer,
            shared: self.shared,
        }
    }
}

#[derive(Debug)]
pub struct AnalysisRendererRetirement {
    input_channels: Vec<AudioChannelId>,
    output_channels: Vec<AudioChannelId>,
    input_meters: MeterAccumulator,
    output_meters: MeterAccumulator,
    taps: Vec<TapCapture>,
    writer: AnalysisFrameWriter,
    shared: Arc<SharedAnalysis>,
}

impl AnalysisRendererRetirement {
    pub fn reclaim(self) {
        assert_not_realtime("final analysis renderer reclamation");
        self.writer.into_retirement().reclaim();
        drop(self.input_channels);
        drop(self.output_channels);
        drop(self.input_meters);
        drop(self.output_meters);
        drop(self.taps);
        drop(self.shared);
    }
}

enum WorkerAnalyzer {
    Pitch(PitchAnalyzer),
    Spectrum(SpectrumAnalyzer),
}

impl WorkerAnalyzer {
    fn analyze(&mut self, samples: &[f32]) -> Result<AnalysisResult, AudioError> {
        match self {
            Self::Pitch(analyzer) => Ok(AnalysisResult::Pitch(analyzer.analyze(samples))),
            Self::Spectrum(analyzer) => analyzer.analyze(samples).map(AnalysisResult::Spectrum),
        }
    }
}

struct WorkerTap {
    enabled: Arc<AtomicBool>,
    analyzer: WorkerAnalyzer,
}

pub fn analysis_pipeline(
    generation: ConfigGeneration,
    plan: &RenderPlan,
    limits: &EngineLimits,
) -> Result<(AnalysisController, AnalysisRenderer), AudioError> {
    assert_not_realtime("analysis pipeline creation");
    limits.validate()?;
    if plan.analysis_taps.len() > usize::from(limits.max_analysis_taps) {
        return Err(AudioError::capacity_exceeded(
            "compiled analysis tap count exceeds the configured engine limit",
        ));
    }
    let mut enabled = HashMap::with_capacity(plan.analysis_taps.len());
    let mut captures = Vec::with_capacity(plan.analysis_taps.len());
    let mut worker_taps = Vec::with_capacity(plan.analysis_taps.len());
    let mut tap_observations = Vec::with_capacity(plan.analysis_taps.len());
    for (index, tap) in plan.analysis_taps.iter().enumerate() {
        let state = Arc::new(AtomicBool::new(tap.enabled));
        enabled.insert(tap.id, Arc::clone(&state));
        captures.push(TapCapture {
            tap_index: index as u16,
            source_index: tap.source_index,
            enabled: Arc::clone(&state),
            hop_frames: tap.processor.hop_frames().max(1),
            samples: vec![0.0; tap.processor.frame_size() as usize],
            write_position: 0,
            filled_samples: 0,
            samples_since_emit: 0,
            emitted_once: false,
        });
        let analyzer = match tap.processor {
            AnalysisProcessorConfiguration::Pitch(configuration) => {
                WorkerAnalyzer::Pitch(PitchAnalyzer::new(configuration, plan.sample_rate)?)
            }
            AnalysisProcessorConfiguration::Spectrum(configuration) => {
                WorkerAnalyzer::Spectrum(SpectrumAnalyzer::new(configuration, plan.sample_rate)?)
            }
        };
        worker_taps.push(WorkerTap {
            enabled: Arc::clone(&state),
            analyzer,
        });
        tap_observations.push(AnalysisTapObservation {
            tap: tap.id,
            source: tap.source,
            enabled: tap.enabled,
            result: None,
        });
    }

    let shared = Arc::new(SharedAnalysis {
        generation,
        input_meters: SharedMeterBank::new(plan.virtual_inputs.clone()),
        output_meters: SharedMeterBank::new(plan.virtual_outputs.clone()),
        taps: RwLock::new(tap_observations),
        diagnostics: SharedDiagnostics::default(),
        shutdown: AtomicBool::new(false),
    });
    let slot_count = plan
        .analysis_taps
        .len()
        .saturating_mul(BUFFERED_FRAMES_PER_ANALYSIS_TAP)
        .max(1);
    let (reader, writer) = analysis_frame_pool(slot_count, limits.max_fft_frames as usize)?;
    let input_meters = MeterAccumulator::new(plan.virtual_inputs.clone(), plan.rms_window_frames as usize)?;
    let output_meters = MeterAccumulator::new(plan.virtual_outputs.clone(), plan.rms_window_frames as usize)?;
    let worker_shared = Arc::clone(&shared);
    let worker = thread::Builder::new()
        .name("golden-audio-analysis".to_owned())
        .spawn(move || run_worker(reader, worker_taps, worker_shared))
        .map_err(|error| {
            AudioError::new(
                AudioErrorCategory::InternalInvariant,
                format!("failed to start analysis worker: {error}"),
            )
        })?;
    Ok((
        AnalysisController {
            shared: Arc::clone(&shared),
            enabled,
            worker: Some(worker),
        },
        AnalysisRenderer {
            generation,
            input_channels: plan.virtual_inputs.clone(),
            output_channels: plan.virtual_outputs.clone(),
            input_meters,
            output_meters,
            observation_interval_frames: plan.observation_interval_frames as usize,
            input_frames_since_publish: 0,
            output_frames_since_publish: 0,
            taps: captures,
            writer,
            shared,
        },
    ))
}

fn run_worker(mut reader: AnalysisFrameReader, mut taps: Vec<WorkerTap>, shared: Arc<SharedAnalysis>) {
    while !shared.shutdown.load(Ordering::Acquire) {
        let Some(frame) = reader.try_recv() else {
            thread::park_timeout(Duration::from_millis(5));
            continue;
        };
        if shared.shutdown.load(Ordering::Acquire) {
            let _ = reader.recycle(frame);
            break;
        }
        let tag = frame.tag();
        let tap_index = usize::from(tag.tap_index);
        let Some(tap) = taps.get_mut(tap_index) else {
            shared.diagnostics.stale_frames.fetch_add(1, Ordering::Relaxed);
            let _ = reader.recycle(frame);
            continue;
        };
        if tag.topology_generation != shared.generation.get() || !tap.enabled.load(Ordering::Acquire) {
            shared.diagnostics.stale_frames.fetch_add(1, Ordering::Relaxed);
            let _ = reader.recycle(frame);
            continue;
        }
        let started_at = Instant::now();
        let result = tap.analyzer.analyze(frame.samples());
        let elapsed = u64::try_from(started_at.elapsed().as_micros()).unwrap_or(u64::MAX);
        shared
            .diagnostics
            .worker_time_micros
            .fetch_add(elapsed, Ordering::Relaxed);
        shared
            .diagnostics
            .maximum_worker_time_micros
            .fetch_max(elapsed, Ordering::Relaxed);
        shared.diagnostics.processed_frames.fetch_add(1, Ordering::Relaxed);
        if tap.enabled.load(Ordering::Acquire)
            && let Ok(result) = result
            && let Ok(mut observations) = shared.taps.write()
            && let Some(observation) = observations.get_mut(tap_index)
        {
            observation.result = Some(result);
        }
        let _ = reader.recycle(frame);
    }
}
