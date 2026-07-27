use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use super::{ReferenceWorkload, ReferenceWorkloadSpec, configuration};
use crate::{
    AudioBackend, AudioBackendState, AudioCommand, AudioDeviceReadiness, AudioDeviceSelection, AudioDirection,
    AudioEngineBuilder, AudioError, AudioErrorCategory, AudioEvent, AudioEventReceiver, AudioRecoveryPolicy,
    AudioSampleFormat, BackendPolicy, ConfigGeneration, DirectionConfiguration, GainDb, PlayFileRequest, PlaybackId,
    RenderRuntimeObservation, SampleRate,
};

const CONTRACT: &str = "golden-audio-managed-device-soak.v1";
const DEFAULT_DURATION: Duration = Duration::from_secs(60 * 60);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(250);
const DEFAULT_READINESS_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_STALL_TOLERANCE: Duration = Duration::from_secs(2);
const DEFAULT_RECOVERY_INTERVAL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadlineMissAcceptance {
    RequireZero,
    RecordOnly,
}

#[derive(Clone, Debug)]
pub struct ManagedDeviceSoakOptions {
    pub duration: Duration,
    pub poll_interval: Duration,
    pub readiness_timeout: Duration,
    pub stall_tolerance: Duration,
    pub recovery_interval: Option<Duration>,
    pub deadline_miss_acceptance: DeadlineMissAcceptance,
    pub workload: ReferenceWorkload,
    pub playback_gain: GainDb,
    pub revision: String,
}

impl Default for ManagedDeviceSoakOptions {
    fn default() -> Self {
        Self {
            duration: DEFAULT_DURATION,
            poll_interval: DEFAULT_POLL_INTERVAL,
            readiness_timeout: DEFAULT_READINESS_TIMEOUT,
            stall_tolerance: DEFAULT_STALL_TOLERANCE,
            recovery_interval: Some(DEFAULT_RECOVERY_INTERVAL),
            deadline_miss_acceptance: DeadlineMissAcceptance::RequireZero,
            workload: ReferenceWorkload::Medium,
            playback_gain: GainDb::new(-96.0).expect("the default qualification gain is valid"),
            revision: "unlabeled".to_owned(),
        }
    }
}

impl ManagedDeviceSoakOptions {
    pub fn validate(&self) -> Result<(), AudioError> {
        if self.duration.is_zero() {
            return Err(AudioError::invalid_configuration(
                "managed device soak duration must be greater than zero",
            ));
        }
        if self.poll_interval < Duration::from_millis(10) || self.poll_interval > self.duration {
            return Err(AudioError::invalid_configuration(
                "managed device soak poll interval must be at least 10 ms and no longer than the run",
            ));
        }
        if self.readiness_timeout.is_zero() || self.stall_tolerance < self.poll_interval {
            return Err(AudioError::invalid_configuration(
                "managed device soak timeouts must be non-zero and stall tolerance must cover one poll",
            ));
        }
        if self.recovery_interval.is_some_and(|interval| interval.is_zero()) {
            return Err(AudioError::invalid_configuration(
                "managed device soak recovery interval must be greater than zero",
            ));
        }
        if self.workload == ReferenceWorkload::ExtremeOffline {
            return Err(AudioError::invalid_configuration(
                "the extreme-offline workload is not a supported real-device deadline workload",
            ));
        }
        if self.revision.trim().is_empty() {
            return Err(AudioError::invalid_configuration(
                "managed device soak revision label must not be empty",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ManagedDeviceSoakProgress {
    pub elapsed_ms: u64,
    pub readiness: AudioDeviceReadiness,
    pub rendered_frames: u64,
    pub active_voices: u16,
    pub xrun_count: u64,
    pub completed_recovery_cycles: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManagedDeviceReadinessTransition {
    pub elapsed_ms: u64,
    pub readiness: AudioDeviceReadiness,
    pub retry_attempt: u32,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ManagedDeviceSoakReport {
    pub contract: String,
    pub revision: String,
    pub platform: String,
    pub started_unix_ms: u64,
    pub backend_id: String,
    pub backend_label: String,
    pub device_label: String,
    pub workload: ReferenceWorkload,
    pub specification: ReferenceWorkloadSpec,
    pub requested_duration_ms: u64,
    pub observed_duration_ms: u64,
    pub sample_rate: u32,
    pub output_channels: u16,
    pub output_buffer_frames: u32,
    pub output_sample_format: AudioSampleFormat,
    pub playback_gain_db: f32,
    pub playback_starts: u64,
    pub playback_finishes: u64,
    pub playback_failures: u64,
    pub runtime_warnings: u64,
    pub runtime_warning_messages: Vec<String>,
    pub attempted_recovery_cycles: u32,
    pub completed_recovery_cycles: u32,
    pub maximum_render_stall_ms: u64,
    pub output_global_max_rms: f32,
    pub dropped_analysis_frames: u64,
    pub stale_analysis_frames: u64,
    pub deadline_miss_acceptance: DeadlineMissAcceptance,
    pub runtime: RenderRuntimeObservation,
    pub readiness_transitions: Vec<ManagedDeviceReadinessTransition>,
    pub passed: bool,
    pub failures: Vec<String>,
}

#[derive(Debug, Default)]
struct EventCounters {
    playback_starts: u64,
    playback_finishes: u64,
    playback_failures: u64,
    runtime_warnings: u64,
    runtime_warning_messages: Vec<String>,
    failures: Vec<String>,
}

pub fn run_managed_device_soak(
    backend: Box<dyn AudioBackend>,
    options: ManagedDeviceSoakOptions,
    playback_path: &Path,
) -> Result<ManagedDeviceSoakReport, AudioError> {
    run_managed_device_soak_with_progress(backend, options, playback_path, |_| {})
}

pub fn run_managed_device_soak_with_progress(
    backend: Box<dyn AudioBackend>,
    options: ManagedDeviceSoakOptions,
    playback_path: &Path,
    mut progress: impl FnMut(&ManagedDeviceSoakProgress),
) -> Result<ManagedDeviceSoakReport, AudioError> {
    options.validate()?;
    if !playback_path.is_file() {
        return Err(AudioError::invalid_configuration(format!(
            "managed device soak playback fixture does not exist: {}",
            playback_path.display()
        )));
    }

    let backend_descriptor = backend.descriptor();
    if backend_descriptor.state != AudioBackendState::Available {
        return Err(AudioError::new(
            AudioErrorCategory::BackendUnavailable,
            format!("backend {} is not available", backend_descriptor.id),
        ));
    }
    let device = backend
        .discover()?
        .into_iter()
        .find(|device| device.is_system_default_output && !device.output_channels.is_empty())
        .ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::DeviceMissing,
                format!("backend {} has no system default output device", backend_descriptor.id),
            )
        })?;
    let supported = device
        .supported_configurations
        .iter()
        .filter(|configuration| configuration.direction == AudioDirection::Output)
        .min_by_key(|configuration| {
            (
                configuration.sample_format != AudioSampleFormat::F32,
                configuration.channels.abs_diff(2),
                configuration.min_sample_rate.abs_diff(48_000),
            )
        })
        .ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::StreamNegotiationFailed,
                "the system default output has no supported stream configuration",
            )
        })?;
    let specification = options.workload.specification();
    if supported.channels > specification.channels {
        return Err(AudioError::invalid_configuration(format!(
            "{} output channels exceed the {:?} workload's {} virtual channels",
            supported.channels, options.workload, specification.channels
        )));
    }
    let sample_rate = SampleRate::new(48_000_u32.clamp(supported.min_sample_rate, supported.max_sample_rate))?;
    let backend_id = backend_descriptor.id.clone();
    let mut builder = AudioEngineBuilder::default()
        .without_backends()
        .with_boxed_backend(backend)
        .with_managed_render_runtime();
    builder.config.sample_rate = sample_rate;
    builder.backend_policy = BackendPolicy {
        preferred: vec![backend_id.clone()],
        allow_null_fallback: false,
    };
    let mut engine = builder.build()?;
    let control = engine.control();
    let observations = engine.observations();
    let mut events = engine
        .take_event_receiver()
        .expect("a newly built audio engine owns its event receiver");
    let generation = ConfigGeneration::new(1);
    let (mut configuration, _) = configuration(specification, usize::from(supported.channels))?;
    configuration.input = DirectionConfiguration::disabled();
    configuration.output = DirectionConfiguration {
        enabled: true,
        device: Some(AudioDeviceSelection::follow_system_default(
            backend_id.clone(),
            AudioDirection::Output,
        )),
        recovery_policy: AudioRecoveryPolicy::FollowSystemDefault,
        buffer_policy: crate::AudioBufferPolicy::Fixed(supported.buffer_frames.preferred),
    };
    control.submit(AudioCommand::ApplyConfiguration {
        generation,
        config: Box::new(configuration),
    })?;

    let wall_start = Instant::now();
    let started_unix_ms = unix_millis();
    let mut counters = EventCounters::default();
    let mut transitions = Vec::new();
    let ready = wait_for_readiness(
        &observations,
        &mut events,
        &mut counters,
        &mut transitions,
        wall_start,
        generation,
        AudioDeviceReadiness::Ready,
        options.readiness_timeout,
    );
    let workload_ready = if ready {
        schedule_voices(&control, playback_path, specification.voices, options.playback_gain)?;
        wait_for_workload(
            &control,
            &observations,
            &mut events,
            &mut counters,
            &mut transitions,
            wall_start,
            playback_path,
            options.playback_gain,
            specification.voices,
            options.readiness_timeout,
        )?
    } else {
        counters
            .failures
            .push("system default output did not become ready before the readiness timeout".to_owned());
        false
    };
    if ready && !workload_ready {
        counters
            .failures
            .push("reference playback workload did not become active before the readiness timeout".to_owned());
    }
    let baseline = observations.latest();
    let negotiated_output = baseline.device.output.format.ok_or_else(|| {
        AudioError::new(
            AudioErrorCategory::StreamNegotiationFailed,
            "ready output did not publish its negotiated stream format",
        )
    })?;
    let soak_start = Instant::now();
    let mut attempted_recovery_cycles = 0_u32;
    let mut completed_recovery_cycles = 0_u32;
    let mut maximum_render_stall = Duration::ZERO;
    let mut stalled_since: Option<Instant> = None;
    let mut last_rendered_frames = baseline.runtime.rendered_frames;
    let mut output_global_max_rms = baseline.output_global_max_rms;
    let mut next_recovery = options.recovery_interval.map(|interval| soak_start + interval);

    while ready && workload_ready && soak_start.elapsed() < options.duration {
        thread::park_timeout(
            options
                .poll_interval
                .min(options.duration.saturating_sub(soak_start.elapsed())),
        );
        replay_finished(
            &control,
            playback_path,
            options.playback_gain,
            drain_events(&mut events, &mut counters),
        )?;
        let snapshot = observations.latest();
        record_readiness_transition(&mut transitions, wall_start, &snapshot);
        output_global_max_rms = output_global_max_rms.max(snapshot.output_global_max_rms);
        if snapshot.runtime.rendered_frames > last_rendered_frames {
            if let Some(started) = stalled_since.take() {
                maximum_render_stall = maximum_render_stall.max(started.elapsed());
            }
            last_rendered_frames = snapshot.runtime.rendered_frames;
        } else if stalled_since.is_none() && snapshot.device.output.readiness == AudioDeviceReadiness::Ready {
            stalled_since = Some(Instant::now());
        }

        if next_recovery.is_some_and(|deadline| Instant::now() >= deadline) {
            attempted_recovery_cycles = attempted_recovery_cycles.saturating_add(1);
            let recovered = cycle_output(
                &control,
                &observations,
                &mut events,
                &mut counters,
                &mut transitions,
                wall_start,
                generation,
                options.readiness_timeout,
            )?;
            if recovered {
                completed_recovery_cycles = completed_recovery_cycles.saturating_add(1);
                schedule_voices(&control, playback_path, specification.voices, options.playback_gain)?;
                stalled_since = None;
                last_rendered_frames = observations.latest().runtime.rendered_frames;
            }
            if let Some(interval) = options.recovery_interval {
                next_recovery = Some(Instant::now() + interval);
            }
        }
        progress(&ManagedDeviceSoakProgress {
            elapsed_ms: duration_millis(soak_start.elapsed()),
            readiness: snapshot.device.output.readiness,
            rendered_frames: snapshot.runtime.rendered_frames,
            active_voices: snapshot.playback.active_voices,
            xrun_count: snapshot.runtime.xrun_count,
            completed_recovery_cycles,
        });
    }
    if let Some(started) = stalled_since {
        maximum_render_stall = maximum_render_stall.max(started.elapsed());
    }

    let before_finalize = observations.latest().runtime.rendered_frames;
    let _ = control.submit(AudioCommand::StopAllFiles);
    let final_snapshot = wait_for_observation_advance(
        &observations,
        &mut events,
        &mut counters,
        before_finalize,
        options.stall_tolerance,
    );
    let runtime = runtime_delta(baseline.runtime, final_snapshot.runtime);
    let dropped_analysis_frames = final_snapshot
        .analysis
        .diagnostics
        .dropped_frames
        .saturating_sub(baseline.analysis.diagnostics.dropped_frames);
    let stale_analysis_frames = final_snapshot
        .analysis
        .diagnostics
        .stale_frames
        .saturating_sub(baseline.analysis.diagnostics.stale_frames);
    let mut failures = counters.failures;
    qualify(
        &options,
        &final_snapshot,
        runtime,
        attempted_recovery_cycles,
        completed_recovery_cycles,
        maximum_render_stall,
        output_global_max_rms,
        dropped_analysis_frames,
        stale_analysis_frames,
        counters.playback_starts,
        counters.playback_failures,
        counters.runtime_warnings,
        &mut failures,
    );
    engine.shutdown()?;

    Ok(ManagedDeviceSoakReport {
        contract: CONTRACT.to_owned(),
        revision: options.revision,
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        started_unix_ms,
        backend_id: backend_id.into_inner(),
        backend_label: backend_descriptor.label,
        device_label: device.label,
        workload: options.workload,
        specification,
        requested_duration_ms: duration_millis(options.duration),
        observed_duration_ms: duration_millis(soak_start.elapsed()),
        sample_rate: sample_rate.get(),
        output_channels: supported.channels,
        output_buffer_frames: negotiated_output.buffer_frames,
        output_sample_format: negotiated_output.sample_format,
        playback_gain_db: options.playback_gain.get(),
        playback_starts: counters.playback_starts,
        playback_finishes: counters.playback_finishes,
        playback_failures: counters.playback_failures,
        runtime_warnings: counters.runtime_warnings,
        runtime_warning_messages: counters.runtime_warning_messages,
        attempted_recovery_cycles,
        completed_recovery_cycles,
        maximum_render_stall_ms: duration_millis(maximum_render_stall),
        output_global_max_rms,
        dropped_analysis_frames,
        stale_analysis_frames,
        deadline_miss_acceptance: options.deadline_miss_acceptance,
        runtime,
        readiness_transitions: transitions,
        passed: failures.is_empty(),
        failures,
    })
}

pub fn write_reference_wave(path: &Path, duration: Duration, sample_rate: SampleRate) -> Result<(), AudioError> {
    if duration.is_zero() {
        return Err(AudioError::invalid_configuration(
            "reference wave duration must be greater than zero",
        ));
    }
    let frames = u64::from(sample_rate.get())
        .checked_mul(duration.as_secs())
        .and_then(|whole| {
            u64::from(sample_rate.get())
                .checked_mul(u64::from(duration.subsec_nanos()))
                .map(|partial| whole.saturating_add(partial / 1_000_000_000))
        })
        .ok_or_else(|| AudioError::capacity_exceeded("reference wave frame count overflowed"))?;
    let channels = 2_u16;
    let bytes_per_sample = 2_u16;
    let data_bytes = frames
        .checked_mul(u64::from(channels))
        .and_then(|samples| samples.checked_mul(u64::from(bytes_per_sample)))
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or_else(|| AudioError::capacity_exceeded("reference wave exceeds the WAV 32-bit data limit"))?;
    let file = File::create(path).map_err(|error| qualification_io("create reference wave", path, error))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(b"RIFF")
        .and_then(|()| writer.write_all(&(36_u32.saturating_add(data_bytes)).to_le_bytes()))
        .and_then(|()| writer.write_all(b"WAVEfmt "))
        .and_then(|()| writer.write_all(&16_u32.to_le_bytes()))
        .and_then(|()| writer.write_all(&1_u16.to_le_bytes()))
        .and_then(|()| writer.write_all(&channels.to_le_bytes()))
        .and_then(|()| writer.write_all(&sample_rate.get().to_le_bytes()))
        .and_then(|()| {
            writer.write_all(
                &(sample_rate
                    .get()
                    .saturating_mul(u32::from(channels))
                    .saturating_mul(u32::from(bytes_per_sample)))
                .to_le_bytes(),
            )
        })
        .and_then(|()| writer.write_all(&(channels.saturating_mul(bytes_per_sample)).to_le_bytes()))
        .and_then(|()| writer.write_all(&(bytes_per_sample.saturating_mul(8)).to_le_bytes()))
        .and_then(|()| writer.write_all(b"data"))
        .and_then(|()| writer.write_all(&data_bytes.to_le_bytes()))
        .map_err(|error| qualification_io("write reference wave header", path, error))?;
    for frame in 0..frames {
        for frequency in [220.0_f32, 440.0_f32] {
            let phase = frame as f32 * frequency * std::f32::consts::TAU / sample_rate.get() as f32;
            let sample = (phase.sin() * 0.25 * f32::from(i16::MAX)).round() as i16;
            writer
                .write_all(&sample.to_le_bytes())
                .map_err(|error| qualification_io("write reference wave samples", path, error))?;
        }
    }
    writer
        .flush()
        .map_err(|error| qualification_io("flush reference wave", path, error))
}

fn schedule_voices(control: &crate::AudioControl, path: &Path, voices: u16, gain: GainDb) -> Result<(), AudioError> {
    for index in 0..voices {
        control.submit(AudioCommand::PlayFile(PlayFileRequest {
            path: path.to_owned(),
            playback_id: playback_id(index),
            gain,
        }))?;
    }
    Ok(())
}

fn replay_finished(
    control: &crate::AudioControl,
    path: &Path,
    gain: GainDb,
    finished: Vec<PlaybackId>,
) -> Result<(), AudioError> {
    for playback_id in finished {
        control.submit(AudioCommand::PlayFile(PlayFileRequest {
            path: path.to_owned(),
            playback_id,
            gain,
        }))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn wait_for_workload(
    control: &crate::AudioControl,
    observations: &crate::AudioObservationReader,
    events: &mut AudioEventReceiver,
    counters: &mut EventCounters,
    transitions: &mut Vec<ManagedDeviceReadinessTransition>,
    wall_start: Instant,
    playback_path: &Path,
    playback_gain: GainDb,
    expected_voices: u16,
    timeout: Duration,
) -> Result<bool, AudioError> {
    let deadline = Instant::now() + timeout;
    loop {
        replay_finished(control, playback_path, playback_gain, drain_events(events, counters))?;
        let snapshot = observations.latest();
        record_readiness_transition(transitions, wall_start, &snapshot);
        if snapshot.playback.active_voices >= expected_voices && snapshot.output_global_max_rms > 0.0 {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::park_timeout(Duration::from_millis(10));
    }
}

fn wait_for_observation_advance(
    observations: &crate::AudioObservationReader,
    events: &mut AudioEventReceiver,
    counters: &mut EventCounters,
    previous_rendered_frames: u64,
    timeout: Duration,
) -> crate::AudioObservationSnapshot {
    let deadline = Instant::now() + timeout;
    loop {
        drain_events(events, counters);
        let snapshot = observations.latest();
        if snapshot.runtime.rendered_frames > previous_rendered_frames || Instant::now() >= deadline {
            return snapshot;
        }
        thread::park_timeout(Duration::from_millis(10));
    }
}

#[allow(clippy::too_many_arguments)]
fn cycle_output(
    control: &crate::AudioControl,
    observations: &crate::AudioObservationReader,
    events: &mut AudioEventReceiver,
    counters: &mut EventCounters,
    transitions: &mut Vec<ManagedDeviceReadinessTransition>,
    wall_start: Instant,
    generation: ConfigGeneration,
    timeout: Duration,
) -> Result<bool, AudioError> {
    control.submit(AudioCommand::SetEnabled(false))?;
    let disabled = wait_for_readiness(
        observations,
        events,
        counters,
        transitions,
        wall_start,
        generation,
        AudioDeviceReadiness::Disabled,
        timeout,
    );
    control.submit(AudioCommand::SetEnabled(true))?;
    let ready = wait_for_readiness(
        observations,
        events,
        counters,
        transitions,
        wall_start,
        generation,
        AudioDeviceReadiness::Ready,
        timeout,
    );
    if !disabled {
        counters
            .failures
            .push("planned recovery cycle did not reach the disabled state".to_owned());
    }
    if !ready {
        counters
            .failures
            .push("planned recovery cycle did not return the output to ready".to_owned());
    }
    Ok(disabled && ready)
}

#[allow(clippy::too_many_arguments)]
fn wait_for_readiness(
    observations: &crate::AudioObservationReader,
    events: &mut AudioEventReceiver,
    counters: &mut EventCounters,
    transitions: &mut Vec<ManagedDeviceReadinessTransition>,
    wall_start: Instant,
    generation: ConfigGeneration,
    expected: AudioDeviceReadiness,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        drain_events(events, counters);
        let snapshot = observations.latest();
        record_readiness_transition(transitions, wall_start, &snapshot);
        if snapshot.generation == generation && snapshot.device.output.readiness == expected {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::park_timeout(Duration::from_millis(10));
    }
}

fn drain_events(events: &mut AudioEventReceiver, counters: &mut EventCounters) -> Vec<PlaybackId> {
    let mut finished = Vec::new();
    for event in events.drain() {
        match event {
            AudioEvent::PlaybackStarted(_) => {
                counters.playback_starts = counters.playback_starts.saturating_add(1);
            }
            AudioEvent::PlaybackFinished(info) => {
                counters.playback_finishes = counters.playback_finishes.saturating_add(1);
                finished.push(info.playback_id);
            }
            AudioEvent::PlaybackFailed(failure) => {
                counters.playback_failures = counters.playback_failures.saturating_add(1);
                counters
                    .failures
                    .push(format!("playback {} failed: {}", failure.playback_id, failure.error));
            }
            AudioEvent::ConfigurationRejected { error, .. } => {
                counters.failures.push(format!("configuration rejected: {error}"));
            }
            AudioEvent::Diagnostic(diagnostic) if diagnostic.severity == crate::DiagnosticSeverity::Error => {
                counters
                    .failures
                    .push(format!("audio diagnostic: {}", diagnostic.message));
            }
            AudioEvent::Diagnostic(diagnostic) if diagnostic.severity == crate::DiagnosticSeverity::Warning => {
                counters.runtime_warnings = counters.runtime_warnings.saturating_add(1);
                counters.runtime_warning_messages.push(diagnostic.message);
            }
            _ => {}
        }
    }
    finished
}

fn record_readiness_transition(
    transitions: &mut Vec<ManagedDeviceReadinessTransition>,
    wall_start: Instant,
    snapshot: &crate::AudioObservationSnapshot,
) {
    let status = &snapshot.device.output;
    if transitions
        .last()
        .is_some_and(|transition| transition.readiness == status.readiness && transition.error == status_error(status))
    {
        return;
    }
    transitions.push(ManagedDeviceReadinessTransition {
        elapsed_ms: duration_millis(wall_start.elapsed()),
        readiness: status.readiness,
        retry_attempt: status.retry_attempt,
        error: status_error(status),
    });
}

fn status_error(status: &crate::AudioStreamStatus) -> Option<String> {
    status.error.as_ref().map(|error| error.message.clone())
}

#[allow(clippy::too_many_arguments)]
fn qualify(
    options: &ManagedDeviceSoakOptions,
    final_snapshot: &crate::AudioObservationSnapshot,
    runtime: RenderRuntimeObservation,
    attempted_recovery_cycles: u32,
    completed_recovery_cycles: u32,
    maximum_render_stall: Duration,
    output_global_max_rms: f32,
    dropped_analysis_frames: u64,
    stale_analysis_frames: u64,
    playback_starts: u64,
    playback_failures: u64,
    runtime_warnings: u64,
    failures: &mut Vec<String>,
) {
    if final_snapshot.device.output.readiness != AudioDeviceReadiness::Ready {
        failures.push(format!(
            "output ended in {:?} instead of ready",
            final_snapshot.device.output.readiness
        ));
    }
    if runtime.rendered_frames == 0 {
        failures.push("managed renderer did not advance".to_owned());
    }
    if playback_starts < u64::from(options.workload.specification().voices) {
        failures.push("not every reference playback voice started".to_owned());
    }
    if playback_failures > 0 {
        failures.push(format!("{playback_failures} playback failures were observed"));
    }
    if runtime_warnings > 0 {
        failures.push(format!("{runtime_warnings} backend runtime warnings were observed"));
    }
    if completed_recovery_cycles != attempted_recovery_cycles {
        failures.push(format!(
            "{completed_recovery_cycles} of {attempted_recovery_cycles} planned recovery cycles completed"
        ));
    }
    if maximum_render_stall > options.stall_tolerance {
        failures.push(format!(
            "maximum render stall of {} ms exceeded the {} ms tolerance",
            duration_millis(maximum_render_stall),
            duration_millis(options.stall_tolerance)
        ));
    }
    if !output_global_max_rms.is_finite() || output_global_max_rms <= 0.0 {
        failures.push("the managed workload produced no finite non-silent output observation".to_owned());
    }
    if options.deadline_miss_acceptance == DeadlineMissAcceptance::RequireZero && runtime.deadline_miss_count > 0 {
        failures.push(format!(
            "{} managed render deadline misses were observed",
            runtime.deadline_miss_count
        ));
    }
    if runtime.control_queue_pressure_count > 0 {
        failures.push(format!(
            "{} managed render control queue pressure events were observed",
            runtime.control_queue_pressure_count
        ));
    }
    if runtime.input_overflow_count > 0 || runtime.output_overflow_count > 0 {
        failures.push("managed callback bridge overflow was observed".to_owned());
    }
    if runtime.input_underflow_count > 0 || runtime.output_underflow_count > 0 {
        failures.push("managed callback bridge underflow was observed".to_owned());
    }
    if dropped_analysis_frames > 0 || stale_analysis_frames > 0 {
        failures.push(format!(
            "analysis worker dropped {dropped_analysis_frames} and rejected {stale_analysis_frames} stale frames"
        ));
    }
}

fn runtime_delta(
    baseline: RenderRuntimeObservation,
    final_observation: RenderRuntimeObservation,
) -> RenderRuntimeObservation {
    RenderRuntimeObservation {
        rendered_blocks: final_observation
            .rendered_blocks
            .saturating_sub(baseline.rendered_blocks),
        rendered_frames: final_observation
            .rendered_frames
            .saturating_sub(baseline.rendered_frames),
        render_time_micros: final_observation
            .render_time_micros
            .saturating_sub(baseline.render_time_micros),
        maximum_render_time_micros: final_observation.maximum_render_time_micros,
        deadline_miss_count: final_observation
            .deadline_miss_count
            .saturating_sub(baseline.deadline_miss_count),
        xrun_count: final_observation.xrun_count.saturating_sub(baseline.xrun_count),
        control_queue_pressure_count: final_observation
            .control_queue_pressure_count
            .saturating_sub(baseline.control_queue_pressure_count),
        input_underflow_count: final_observation
            .input_underflow_count
            .saturating_sub(baseline.input_underflow_count),
        input_overflow_count: final_observation
            .input_overflow_count
            .saturating_sub(baseline.input_overflow_count),
        output_underflow_count: final_observation
            .output_underflow_count
            .saturating_sub(baseline.output_underflow_count),
        output_overflow_count: final_observation
            .output_overflow_count
            .saturating_sub(baseline.output_overflow_count),
    }
}

fn playback_id(index: u16) -> PlaybackId {
    PlaybackId::new(format!("managed-soak-voice-{index}")).expect("generated soak playback ID is valid")
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(duration_millis)
        .unwrap_or_default()
}

fn qualification_io(action: &'static str, path: &Path, error: std::io::Error) -> AudioError {
    AudioError::new(
        AudioErrorCategory::InternalInvariant,
        format!("failed to {action}: {error}"),
    )
    .with_context("path", path.display().to_string())
}
