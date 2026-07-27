//! Deterministic whole-pipeline workloads used by benchmarks and product evidence.
//!
//! The harness deliberately uses only public Golden Audio contracts. It keeps setup,
//! allocation, worker synchronization, and teardown outside measured render calls.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AnalysisController, AnalysisObservationReader, AnalysisProcessorConfiguration, AnalysisTapConfiguration,
    AnalysisTapId, AudioChannelId, AudioConfiguration, AudioEngineConfig, AudioError, AudioRouteId, ConfigGeneration,
    DefaultPlaybackRoute, EngineLimits, GainDb, InputPatchRoute, MonitorRoute, OutputPatchRoute, PhysicalChannelKey,
    PitchAnalysisConfiguration, PlanarBuffer, PlaybackId, PlaybackRenderMetrics, PlaybackRoute, PlaybackVoice,
    PlaybackVoiceController, PlaybackVoiceRenderer, PlaybackVoiceSource, RenderCompileContext, RenderPlanCompiler,
    RenderProcessor, ResidentAssetKey, ResidentAudioAsset, SampleRate, SpectrumAnalysisConfiguration,
    VirtualInputChannel, VirtualOutputChannel, analysis_pipeline, default_playback_routes, playback_voice_pool,
};

mod device_soak;
pub use device_soak::{
    DeadlineMissAcceptance, ManagedDeviceReadinessTransition, ManagedDeviceSoakOptions, ManagedDeviceSoakProgress,
    ManagedDeviceSoakReport, run_managed_device_soak, run_managed_device_soak_with_progress, write_reference_wave,
};

#[cfg(test)]
mod tests;

const BLOCK_FRAMES: usize = 128;
const ASSET_FRAMES: usize = 480_000;
const PHYSICAL_CHANNELS: usize = 2;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceWorkload {
    Small,
    Medium,
    Large,
    ExtremeOffline,
}

impl ReferenceWorkload {
    pub const ALL: [Self; 4] = [Self::Small, Self::Medium, Self::Large, Self::ExtremeOffline];

    #[must_use]
    pub const fn specification(self) -> ReferenceWorkloadSpec {
        match self {
            Self::Small => ReferenceWorkloadSpec {
                channels: 8,
                routes: 16,
                voices: 4,
                pitch_taps: 0,
                spectrum_taps: 0,
                spectrum_bands: 0,
                analysis_frame_size: 0,
            },
            Self::Medium => ReferenceWorkloadSpec {
                channels: 32,
                routes: 128,
                voices: 32,
                pitch_taps: 1,
                spectrum_taps: 1,
                spectrum_bands: 64,
                analysis_frame_size: 2_048,
            },
            Self::Large => ReferenceWorkloadSpec {
                channels: 128,
                routes: 1_024,
                voices: 128,
                pitch_taps: 4,
                spectrum_taps: 1,
                spectrum_bands: 256,
                analysis_frame_size: 2_048,
            },
            Self::ExtremeOffline => ReferenceWorkloadSpec {
                channels: 256,
                routes: 16_384,
                voices: 256,
                pitch_taps: 32,
                spectrum_taps: 32,
                spectrum_bands: 256,
                analysis_frame_size: 16_384,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReferenceWorkloadSpec {
    pub channels: u16,
    pub routes: u32,
    pub voices: u16,
    pub pitch_taps: u16,
    pub spectrum_taps: u16,
    pub spectrum_bands: u16,
    pub analysis_frame_size: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReferenceWorkloadObservation {
    pub workload: ReferenceWorkload,
    pub specification: ReferenceWorkloadSpec,
    pub rendered_blocks: u64,
    pub rendered_frames: u64,
    pub active_voices: u16,
    pub peak_output: f32,
    pub finite_output: bool,
    pub input_global_max_rms: f32,
    pub output_global_max_rms: f32,
    pub observed_pitch_taps: u16,
    pub observed_spectrum_taps: u16,
    pub dropped_analysis_frames: u64,
    pub stale_analysis_frames: u64,
    pub estimated_resident_bytes: u64,
}

#[derive(Debug)]
pub struct ReferenceWorkloadHarness {
    workload: ReferenceWorkload,
    specification: ReferenceWorkloadSpec,
    processor: RenderProcessor,
    playback_controller: PlaybackVoiceController,
    playback_renderer: Option<PlaybackVoiceRenderer>,
    physical_inputs: PlanarBuffer,
    playback_inputs: PlanarBuffer,
    physical_outputs: PlanarBuffer,
    analysis_controller: Option<AnalysisController>,
    analysis_observations: AnalysisObservationReader,
    estimated_resident_bytes: u64,
}

impl ReferenceWorkloadHarness {
    pub fn new(workload: ReferenceWorkload) -> Result<Self, AudioError> {
        let specification = workload.specification();
        let limits = EngineLimits::default();
        let (configuration, context) = configuration(specification, PHYSICAL_CHANNELS)?;
        let plan = RenderPlanCompiler::new(AudioEngineConfig::default(), limits.clone())
            .compile(&configuration, &context)?
            .plan;
        let (analysis_controller, analysis_renderer) = analysis_pipeline(ConfigGeneration::new(1), &plan, &limits)?;
        let analysis_observations = analysis_controller.observations();
        let mut processor = RenderProcessor::new(plan)?;
        processor.attach_analysis(analysis_renderer)?;

        let channels = usize::from(specification.channels);
        let mut physical_inputs = PlanarBuffer::new(PHYSICAL_CHANNELS, BLOCK_FRAMES)?;
        seed_input(&mut physical_inputs);
        let physical_outputs = PlanarBuffer::new(PHYSICAL_CHANNELS, BLOCK_FRAMES)?;
        let playback_inputs = PlanarBuffer::new(channels, BLOCK_FRAMES)?;
        let (mut playback_controller, playback_renderer) =
            playback_voice_pool(specification.voices, specification.channels, BLOCK_FRAMES)?;
        let asset = Arc::new(synthetic_asset()?);
        for index in 0..specification.voices {
            playback_controller.try_activate(synthetic_voice(index, Arc::clone(&asset), specification.channels)?)?;
        }

        let estimated_resident_bytes = estimate_resident_bytes(specification, asset.memory_bytes(), processor.plan());
        Ok(Self {
            workload,
            specification,
            processor,
            playback_controller,
            playback_renderer: Some(playback_renderer),
            physical_inputs,
            playback_inputs,
            physical_outputs,
            analysis_controller: Some(analysis_controller),
            analysis_observations,
            estimated_resident_bytes,
        })
    }

    pub fn render_block(&mut self) -> Result<(), AudioError> {
        let renderer = self
            .playback_renderer
            .as_mut()
            .expect("reference renderer remains present until teardown");
        renderer.render(&mut self.playback_inputs, BLOCK_FRAMES)?;
        self.processor.render(
            &self.physical_inputs,
            &self.playback_inputs,
            &mut self.physical_outputs,
            BLOCK_FRAMES,
        )?;
        Ok(())
    }

    pub fn render_blocks(&mut self, blocks: usize) -> Result<(), AudioError> {
        for _ in 0..blocks {
            self.render_block()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn playback_metrics(&self) -> PlaybackRenderMetrics {
        self.playback_renderer
            .as_ref()
            .expect("reference renderer remains present until teardown")
            .metrics()
    }

    pub fn wait_for_analysis(&self, timeout: Duration) -> bool {
        let expected = usize::from(
            self.specification
                .pitch_taps
                .saturating_add(self.specification.spectrum_taps),
        );
        if expected == 0 {
            return true;
        }
        let deadline = Instant::now() + timeout;
        loop {
            let ready = self
                .analysis_observations
                .latest()
                .taps
                .iter()
                .filter(|tap| tap.result.is_some())
                .count();
            if ready >= expected {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::park_timeout(Duration::from_millis(1));
        }
    }

    #[must_use]
    pub fn observation(&self) -> ReferenceWorkloadObservation {
        let render_metrics = self.processor.metrics();
        let playback_metrics = self.playback_metrics();
        let analysis = self.analysis_observations.latest();
        let mut peak_output = 0.0_f32;
        let mut finite_output = true;
        for channel in 0..self.physical_outputs.channels() {
            for sample in self.physical_outputs.channel(channel).iter().take(BLOCK_FRAMES) {
                finite_output &= sample.is_finite();
                peak_output = peak_output.max(sample.abs());
            }
        }
        let mut observed_pitch_taps = 0_u16;
        let mut observed_spectrum_taps = 0_u16;
        for tap in &analysis.taps {
            match tap.result {
                Some(crate::AnalysisResult::Pitch(_)) => {
                    observed_pitch_taps = observed_pitch_taps.saturating_add(1);
                }
                Some(crate::AnalysisResult::Spectrum(_)) => {
                    observed_spectrum_taps = observed_spectrum_taps.saturating_add(1);
                }
                None => {}
            }
        }
        ReferenceWorkloadObservation {
            workload: self.workload,
            specification: self.specification,
            rendered_blocks: render_metrics.rendered_blocks,
            rendered_frames: render_metrics.rendered_frames,
            active_voices: u16::try_from(playback_metrics.active_voices).unwrap_or(u16::MAX),
            peak_output,
            finite_output,
            input_global_max_rms: analysis.input_global_max_rms,
            output_global_max_rms: analysis.output_global_max_rms,
            observed_pitch_taps,
            observed_spectrum_taps,
            dropped_analysis_frames: analysis.diagnostics.dropped_frames,
            stale_analysis_frames: analysis.diagnostics.stale_frames,
            estimated_resident_bytes: self.estimated_resident_bytes,
        }
    }
}

impl Drop for ReferenceWorkloadHarness {
    fn drop(&mut self) {
        if let Some(renderer) = self.processor.take_analysis() {
            renderer.into_retirement().reclaim();
        }
        if let Some(mut controller) = self.analysis_controller.take() {
            let _ = controller.shutdown();
        }
        if let Some(renderer) = self.playback_renderer.take() {
            renderer.into_retirement().reclaim();
        }
        self.playback_controller.reclaim(|_| {});
    }
}

fn configuration(
    specification: ReferenceWorkloadSpec,
    physical_channel_count: usize,
) -> Result<(AudioConfiguration, RenderCompileContext), AudioError> {
    let channels = usize::from(specification.channels);
    let inputs = (0..channels).map(|index| channel_id(1, index)).collect::<Vec<_>>();
    let outputs = (0..channels).map(|index| channel_id(2, index)).collect::<Vec<_>>();
    let physical_inputs = (0..physical_channel_count)
        .map(|index| physical("input", index))
        .collect::<Vec<_>>();
    let physical_outputs = (0..physical_channel_count)
        .map(|index| physical("output", index))
        .collect::<Vec<_>>();
    let mut configuration = AudioConfiguration::empty();
    configuration.physical_inputs = physical_inputs;
    configuration.physical_outputs = physical_outputs;
    configuration.virtual_inputs = inputs
        .iter()
        .enumerate()
        .map(|(index, id)| VirtualInputChannel {
            id: *id,
            label: format!("Input {}", index + 1),
        })
        .collect();
    configuration.virtual_outputs = outputs
        .iter()
        .enumerate()
        .map(|(index, id)| VirtualOutputChannel {
            id: *id,
            label: format!("Output {}", index + 1),
            gain: GainDb::UNITY,
        })
        .collect();
    configuration.input_patch = (0..physical_channel_count.min(channels))
        .map(|index| InputPatchRoute {
            id: route_id(1, index),
            source: configuration.physical_inputs[index].clone(),
            destination: inputs[index],
            gain: GainDb::UNITY,
        })
        .collect();
    configuration.output_patch = (0..physical_channel_count.min(channels))
        .map(|index| OutputPatchRoute {
            id: route_id(2, index),
            source: outputs[index],
            destination: configuration.physical_outputs[index].clone(),
            gain: GainDb::UNITY,
        })
        .collect();
    configuration.playback_patch = (0..physical_channel_count.min(channels))
        .map(|index| PlaybackRoute {
            id: route_id(3, index),
            source_channel: index as u16,
            destination: outputs[index],
            gain: GainDb::UNITY,
        })
        .collect();
    let fixed_routes =
        configuration.input_patch.len() + configuration.output_patch.len() + configuration.playback_patch.len();
    let monitoring_routes = usize::try_from(specification.routes)
        .unwrap_or(usize::MAX)
        .saturating_sub(fixed_routes);
    let monitoring_gain = GainDb::new(-18.0)?;
    configuration.monitoring = (0..monitoring_routes)
        .map(|index| MonitorRoute {
            id: route_id(4, index),
            source: inputs[index % channels],
            destination: outputs[(index / channels) % channels],
            gain: monitoring_gain,
        })
        .collect();
    configuration.analysis_taps = analysis_taps(specification, &inputs);
    configuration.validate(&EngineLimits::default())?;
    Ok((
        configuration,
        RenderCompileContext {
            playback_source_channels: channels,
        },
    ))
}

fn analysis_taps(specification: ReferenceWorkloadSpec, inputs: &[AudioChannelId]) -> Vec<AnalysisTapConfiguration> {
    let mut taps = Vec::with_capacity(usize::from(
        specification.pitch_taps.saturating_add(specification.spectrum_taps),
    ));
    for index in 0..specification.pitch_taps {
        let configuration = PitchAnalysisConfiguration {
            frame_size: specification.analysis_frame_size,
            ..PitchAnalysisConfiguration::default()
        };
        taps.push(AnalysisTapConfiguration {
            id: analysis_id(1, usize::from(index)),
            source: inputs[usize::from(index) % inputs.len()],
            enabled: true,
            processor: AnalysisProcessorConfiguration::Pitch(configuration),
        });
    }
    for index in 0..specification.spectrum_taps {
        let configuration = SpectrumAnalysisConfiguration {
            fft_size: specification.analysis_frame_size,
            band_count: specification.spectrum_bands,
            ..SpectrumAnalysisConfiguration::default()
        };
        taps.push(AnalysisTapConfiguration {
            id: analysis_id(2, usize::from(index)),
            source: inputs[usize::from(index) % inputs.len()],
            enabled: true,
            processor: AnalysisProcessorConfiguration::Spectrum(configuration),
        });
    }
    taps
}

fn synthetic_asset() -> Result<ResidentAudioAsset, AudioError> {
    let sample_rate = SampleRate::default();
    let samples = (0..2)
        .flat_map(|channel| {
            (0..ASSET_FRAMES).map(move |frame| {
                let frequency = if channel == 0 { 220.0 } else { 440.0 };
                ((frame as f32 * frequency * std::f32::consts::TAU) / sample_rate.get() as f32).sin() * 0.02
            })
        })
        .collect::<Vec<_>>();
    ResidentAudioAsset::new(
        ResidentAssetKey {
            source: crate::AudioSourceFingerprint {
                canonical_path: PathBuf::from("golden-audio-reference.wav"),
                length_bytes: u64::try_from(samples.len()).unwrap_or(u64::MAX).saturating_mul(4),
                modified_nanos: 0,
            },
            track: 0,
            engine_sample_rate: sample_rate,
        },
        2,
        ASSET_FRAMES,
        samples,
    )
}

fn synthetic_voice(index: u16, asset: Arc<ResidentAudioAsset>, channels: u16) -> Result<PlaybackVoice, AudioError> {
    PlaybackVoice::new(
        PlaybackId::new(format!("reference-voice-{index}")).expect("generated reference playback ID is valid"),
        PathBuf::from("golden-audio-reference.wav"),
        PlaybackVoiceSource::Resident(asset),
        GainDb::new(-36.0)?,
        playback_routes(channels),
        480,
        BLOCK_FRAMES,
    )
}

fn playback_routes(channels: u16) -> Vec<DefaultPlaybackRoute> {
    default_playback_routes(2, channels)
}

fn seed_input(input: &mut PlanarBuffer) {
    for channel in 0..input.channels() {
        let frequency = if channel == 0 { 440.0 } else { 880.0 };
        for frame in 0..BLOCK_FRAMES {
            let phase = frame as f32 * frequency * std::f32::consts::TAU / SampleRate::default().get() as f32;
            input.set_sample(channel, frame, phase.sin() * 0.1);
        }
    }
}

fn estimate_resident_bytes(specification: ReferenceWorkloadSpec, asset_bytes: u64, plan: &crate::RenderPlan) -> u64 {
    let sample_bytes = std::mem::size_of::<f32>() as u64;
    let render_samples = u64::from(specification.channels)
        .saturating_mul(BLOCK_FRAMES as u64)
        .saturating_mul(4)
        .saturating_add((PHYSICAL_CHANNELS * BLOCK_FRAMES * 2) as u64);
    let analysis_samples = u64::from(specification.pitch_taps.saturating_add(specification.spectrum_taps))
        .saturating_mul(u64::from(specification.analysis_frame_size))
        .saturating_mul(3);
    let route_bytes = u64::try_from(
        plan.input_patch.routes.len()
            + plan.monitoring.routes.len()
            + plan.playback_patch.routes.len()
            + plan.output_patch.routes.len(),
    )
    .unwrap_or(u64::MAX)
    .saturating_mul(std::mem::size_of::<crate::CompiledRoute>() as u64);
    asset_bytes
        .saturating_add(render_samples.saturating_mul(sample_bytes))
        .saturating_add(analysis_samples.saturating_mul(sample_bytes))
        .saturating_add(route_bytes)
}

fn channel_id(group: u8, index: usize) -> AudioChannelId {
    AudioChannelId::from_uuid(Uuid::from_u128((u128::from(group) << 120) | (index as u128 + 1)))
}

fn route_id(group: u8, index: usize) -> AudioRouteId {
    AudioRouteId::from_uuid(Uuid::from_u128((u128::from(group) << 120) | (index as u128 + 1)))
}

fn analysis_id(group: u8, index: usize) -> AnalysisTapId {
    AnalysisTapId::from_uuid(Uuid::from_u128((u128::from(group) << 112) | (index as u128 + 1)))
}

fn physical(prefix: &str, index: usize) -> PhysicalChannelKey {
    PhysicalChannelKey::new(format!("{prefix}:{index}")).expect("generated reference physical channel key is valid")
}
