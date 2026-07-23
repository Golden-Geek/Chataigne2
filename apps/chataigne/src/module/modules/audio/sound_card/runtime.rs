use std::collections::{HashMap, HashSet};

use chataigne_sound_card_protocol::SoundCardUiTelemetryDto;
use golden_audio::{
    AnalysisProcessorConfiguration, AnalysisTapConfiguration, AnalysisTapId, AnalysisTapObservation,
    AnalysisResult, AudioBufferPolicy, AudioChannelId, AudioCommand, AudioConfiguration,
    AudioDeviceDescriptor, AudioDeviceInspectorState, AudioDeviceMatch, AudioDeviceReadiness,
    AudioDeviceSelection, AudioDeviceTargetId, AudioDirection, AudioEngine, AudioEngineBuilder,
    AudioEvent, AudioEventReceiver, AudioObservationReader, AudioObservationSnapshot,
    AudioRecoveryPolicy, AudioRouteId, ConfigGeneration, DirectionConfiguration, GainDb,
    InputPatchRoute, MonitorRoute, NullBackend, OutputPatchRoute, PhysicalChannelKey,
    PitchAnalysisConfiguration, PlaybackRoute, SampleRate, SpectrumAnalysisConfiguration,
    SpectrumBandSpacing, SpectrumOverlap, SpectrumWindow, VirtualInputChannel, VirtualOutputChannel,
    match_device_selection, profile_key_for,
};
use golden_core::{
    node::{NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use super::{
    ANALYSIS_PATH, INPUT_LEVELS_PATH, INPUT_PROFILES_PATH, OUTPUT_LEVELS_PATH,
    OUTPUT_PROFILES_PATH, PITCH_RESULTS_PATH, SPECTRUM_RESULTS_PATH, SYSTEM_DEFAULT_INPUT,
    SYSTEM_DEFAULT_OUTPUT, SoundCardChannelMeter, SoundCardInputPatchRoute, SoundCardInputProfile,
    SoundCardMonitorRoute, SoundCardOutputPatchRoute, SoundCardOutputProfile,
    SoundCardPitchAnalyzer, SoundCardPlaybackRoute, SoundCardSpectrumAnalyzer,
    SoundCardSpectrumBand, SoundCardVirtualInput, SoundCardVirtualOutput, VIRTUAL_INPUTS_PATH,
    VIRTUAL_OUTPUTS_PATH, find_child_by_key, find_path, pitch_result_uuid, spectrum_result_uuid,
};

mod observation;
mod snapshot;

use observation::{
    collect_bindings, format_description, readiness_name, saturating_i32,
    telemetry_from_observation, values_nearly_equal,
};
use snapshot::*;

pub(super) const SOUND_CARD_UPDATE_RATE_HZ: u32 = 30;
pub(super) const SOUND_CARD_COMPILED_KERNEL: &str = "chataigne.runtime.sound_card";
pub(super) const OBSERVATION_EPSILON: f64 = 0.000_01;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RuntimeWarning {
    pub node: NodeId,
    pub id: String,
    pub message: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct BuiltSoundCardConfiguration {
    pub configuration: AudioConfiguration,
    pub sample_rate: SampleRate,
    pub bindings: SoundCardValueBindings,
    pub warnings: Vec<RuntimeWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConfigurationError {
    pub node: NodeId,
    pub message: String,
}

#[derive(Clone, Copy, Debug)]
struct MeterBinding {
    linear_rms: NodeId,
    rms_dbfs: NodeId,
    peak_dbfs: NodeId,
    clipped: NodeId,
}

#[derive(Clone, Copy, Debug)]
struct PitchBinding {
    valid: NodeId,
    frequency_hz: NodeId,
    confidence: NodeId,
    midi_note: NodeId,
    note_name: NodeId,
    cents: NodeId,
}

#[derive(Clone, Copy, Debug)]
struct SpectrumBandBinding {
    low_hz: NodeId,
    center_hz: NodeId,
    high_hz: NodeId,
    linear_amplitude: NodeId,
    dbfs: NodeId,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SoundCardValueBindings {
    input_device: Option<NodeId>,
    output_device: Option<NodeId>,
    input_readiness: Option<NodeId>,
    output_readiness: Option<NodeId>,
    negotiated_input_format: Option<NodeId>,
    negotiated_output_format: Option<NodeId>,
    input_global_max_rms: Option<NodeId>,
    output_global_max_rms: Option<NodeId>,
    global_max_rms: Option<NodeId>,
    active_voices: Option<NodeId>,
    loading_voices: Option<NodeId>,
    xruns: Option<NodeId>,
    dropped_analysis_frames: Option<NodeId>,
    last_error: Option<NodeId>,
    meters: HashMap<AudioChannelId, MeterBinding>,
    pitch: HashMap<AnalysisTapId, PitchBinding>,
    spectrum: HashMap<AnalysisTapId, Vec<SpectrumBandBinding>>,
    runtime_values: HashSet<NodeId>,
}

impl SoundCardValueBindings {
    pub(super) fn is_runtime_value(&self, node: NodeId) -> bool {
        self.runtime_values.contains(&node)
    }

    pub(super) fn input_device(&self) -> Option<NodeId> {
        self.input_device
    }

    pub(super) fn output_device(&self) -> Option<NodeId> {
        self.output_device
    }
}

#[derive(Debug)]
pub(super) struct SoundCardRuntime {
    engine: AudioEngine,
    events: AudioEventReceiver,
    observations: AudioObservationReader,
    generation: ConfigGeneration,
    sample_rate: SampleRate,
    bindings: SoundCardValueBindings,
    last_values: HashMap<NodeId, ParamValue>,
    last_telemetry: Option<SoundCardUiTelemetryDto>,
    last_error: Option<String>,
}

impl SoundCardRuntime {
    pub(super) fn start(sample_rate: SampleRate) -> Result<Self, String> {
        let mut builder = AudioEngineBuilder::default().without_backends();
        builder.config.sample_rate = sample_rate;
        #[cfg(not(test))]
        {
            for backend in golden_audio::compiled_cpal_backends() {
                builder = builder.with_boxed_backend(backend);
            }
        }
        builder = builder.with_backend(NullBackend);
        let mut engine = builder.build().map_err(|error| error.to_string())?;
        let events = engine
            .take_event_receiver()
            .ok_or_else(|| "golden_audio event receiver was already taken".to_owned())?;
        Ok(Self {
            observations: engine.observations(),
            engine,
            events,
            generation: ConfigGeneration::INITIAL,
            sample_rate,
            bindings: SoundCardValueBindings::default(),
            last_values: HashMap::new(),
            last_telemetry: None,
            last_error: None,
        })
    }

    pub(super) fn inspector_state(&self) -> AudioDeviceInspectorState {
        self.observations.latest().device
    }

    pub(super) const fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    #[cfg(test)]
    pub(super) const fn generation(&self) -> ConfigGeneration {
        self.generation
    }

    #[cfg(test)]
    pub(super) fn control(&self) -> golden_audio::AudioControl {
        self.engine.control()
    }

    pub(super) fn bindings(&self) -> &SoundCardValueBindings {
        &self.bindings
    }

    pub(super) fn submit(
        &mut self,
        built: BuiltSoundCardConfiguration,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<ConfigGeneration, String> {
        let generation = self.generation.next();
        self.engine
            .control()
            .submit(AudioCommand::ApplyConfiguration {
                generation,
                config: Box::new(built.configuration),
            })
            .map_err(|error| error.to_string())?;
        self.generation = generation;
        self.bindings = built.bindings;
        self.last_values.clear();
        for node in &self.bindings.runtime_values {
            if let Some(value) = snapshot.node(*node).and_then(|node| node.param_value.clone()) {
                self.last_values.insert(*node, value);
            }
        }
        Ok(generation)
    }

    pub(super) fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.engine
            .control()
            .submit(AudioCommand::SetEnabled(enabled))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub(super) fn poll(
        &mut self,
        ctx: &mut ProcessCtx,
    ) -> (AudioObservationSnapshot, Option<SoundCardUiTelemetryDto>) {
        for event in self.events.drain() {
            match event {
                AudioEvent::ConfigurationRejected { error, .. } => {
                    self.last_error = Some(error.to_string());
                }
                AudioEvent::ConfigurationApplied { .. } => {
                    self.last_error = None;
                }
                AudioEvent::PlaybackFailed(failure) => {
                    self.last_error = Some(failure.error.to_string());
                }
                AudioEvent::Diagnostic(diagnostic) => {
                    self.last_error = Some(diagnostic.message);
                }
                _ => {}
            }
        }

        let observation = self.observations.latest();
        self.apply_observation_values(ctx, &observation);
        let telemetry = telemetry_from_observation(&observation);
        let changed = self.last_telemetry.as_ref() != Some(&telemetry);
        if changed {
            self.last_telemetry = Some(telemetry.clone());
        }
        (observation, changed.then_some(telemetry))
    }

    pub(super) fn stop(&mut self) {
        let _ = self.engine.shutdown();
    }

    fn apply_observation_values(
        &mut self,
        ctx: &mut ProcessCtx,
        observation: &AudioObservationSnapshot,
    ) {
        self.set_value(
            ctx,
            self.bindings.input_readiness,
            ParamValue::Str(readiness_name(observation.device.input.readiness)),
        );
        self.set_value(
            ctx,
            self.bindings.output_readiness,
            ParamValue::Str(readiness_name(observation.device.output.readiness)),
        );
        self.set_value(
            ctx,
            self.bindings.negotiated_input_format,
            ParamValue::Str(format_description(observation.device.input.format.as_ref())),
        );
        self.set_value(
            ctx,
            self.bindings.negotiated_output_format,
            ParamValue::Str(format_description(observation.device.output.format.as_ref())),
        );
        self.set_float(ctx, self.bindings.input_global_max_rms, observation.input_global_max_rms);
        self.set_float(ctx, self.bindings.output_global_max_rms, observation.output_global_max_rms);
        self.set_float(ctx, self.bindings.global_max_rms, observation.global_max_rms);
        self.set_value(
            ctx,
            self.bindings.active_voices,
            ParamValue::Int(i32::from(observation.active_voice_count)),
        );
        self.set_value(ctx, self.bindings.loading_voices, ParamValue::Int(0));
        self.set_value(ctx, self.bindings.xruns, ParamValue::Int(0));
        self.set_value(
            ctx,
            self.bindings.dropped_analysis_frames,
            ParamValue::Int(saturating_i32(
                observation.analysis.diagnostics.dropped_frames,
            )),
        );
        self.set_value(
            ctx,
            self.bindings.last_error,
            ParamValue::Str(self.last_error.clone().unwrap_or_default()),
        );

        for channel in observation.inputs.iter().chain(&observation.outputs) {
            let Some(binding) = self.bindings.meters.get(&channel.channel).copied() else {
                continue;
            };
            self.set_float(ctx, Some(binding.linear_rms), channel.rms_linear);
            self.set_float(ctx, Some(binding.rms_dbfs), channel.rms_dbfs);
            self.set_float(ctx, Some(binding.peak_dbfs), channel.peak_dbfs);
            self.set_value(ctx, Some(binding.clipped), ParamValue::Bool(channel.clipped));
        }
        for tap in &observation.analysis.taps {
            self.apply_analysis_tap(ctx, tap);
        }
    }

    fn apply_analysis_tap(&mut self, ctx: &mut ProcessCtx, tap: &AnalysisTapObservation) {
        match tap.result.as_ref() {
            Some(AnalysisResult::Pitch(value)) => {
                let Some(binding) = self.bindings.pitch.get(&tap.tap).copied() else {
                    return;
                };
                self.set_value(ctx, Some(binding.valid), ParamValue::Bool(value.valid));
                self.set_float(ctx, Some(binding.frequency_hz), value.frequency_hz);
                self.set_float(ctx, Some(binding.confidence), value.confidence);
                self.set_float(ctx, Some(binding.midi_note), value.midi_note);
                self.set_value(
                    ctx,
                    Some(binding.note_name),
                    ParamValue::Str(value.note_name.clone()),
                );
                self.set_float(ctx, Some(binding.cents), value.cents);
            }
            Some(AnalysisResult::Spectrum(value)) => {
                let Some(bindings) = self.bindings.spectrum.get(&tap.tap).cloned() else {
                    return;
                };
                for (binding, band) in bindings.into_iter().zip(&value.bands) {
                    self.set_float(ctx, Some(binding.low_hz), band.low_hz);
                    self.set_float(ctx, Some(binding.center_hz), band.center_hz);
                    self.set_float(ctx, Some(binding.high_hz), band.high_hz);
                    self.set_float(ctx, Some(binding.linear_amplitude), band.amplitude_linear);
                    self.set_float(ctx, Some(binding.dbfs), band.amplitude_dbfs);
                }
            }
            None => {}
        }
    }

    fn set_float(&mut self, ctx: &mut ProcessCtx, node: Option<NodeId>, value: impl Into<f64>) {
        self.set_value(ctx, node, ParamValue::Float(value.into()));
    }

    fn set_value(&mut self, ctx: &mut ProcessCtx, node: Option<NodeId>, value: ParamValue) {
        let Some(node) = node else {
            return;
        };
        if self
            .last_values
            .get(&node)
            .is_some_and(|current| values_nearly_equal(current, &value))
        {
            return;
        }
        self.last_values.insert(node, value.clone());
        ctx.set_param_with_behaviour(node, value, ParameterEventBehaviour::Coalesce);
    }
}

impl Drop for SoundCardRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(super) fn build_configuration(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    inspector: &AudioDeviceInspectorState,
) -> Result<BuiltSoundCardConfiguration, ConfigurationError> {
    let connection = required_path(snapshot, module, "connection")?;
    let input_enabled = child_bool(snapshot, connection, "input_enabled", false);
    let output_enabled = child_bool(snapshot, connection, "output_enabled", true);
    let sample_rate = SampleRate::new(
        child_int(snapshot, connection, "engine_sample_rate", 48_000).max(1) as u32,
    )
    .map_err(|error| configuration_error(connection, error.to_string()))?;
    let buffer_policy = match child_enum(snapshot, connection, "buffer_policy", "automatic").as_str() {
        "fixed" => AudioBufferPolicy::Fixed(
            child_int(snapshot, connection, "fixed_buffer_frames", 128).max(1) as u32,
        ),
        _ => AudioBufferPolicy::Automatic,
    };
    let recovery_policy =
        match child_enum(snapshot, connection, "recovery_policy", "wait_for_selected").as_str() {
            "follow_system_default" => AudioRecoveryPolicy::FollowSystemDefault,
            _ => AudioRecoveryPolicy::WaitForSelected,
        };
    let input_value = child_enum(snapshot, connection, "input_device", SYSTEM_DEFAULT_INPUT);
    let output_value = child_enum(snapshot, connection, "output_device", SYSTEM_DEFAULT_OUTPUT);
    let input_selection = resolve_selection(inspector, &input_value, AudioDirection::Input);
    if input_enabled && input_selection.is_none() {
        return Err(configuration_error(
            connection,
            "no input audio backend is currently available",
        ));
    }
    let output_selection = resolve_selection(inspector, &output_value, AudioDirection::Output);
    if output_enabled && output_selection.is_none() {
        return Err(configuration_error(
            connection,
            "no output audio backend is currently available",
        ));
    }

    let input_device = input_selection
        .as_ref()
        .and_then(|selection| matched_device(inspector, selection, AudioDirection::Input));
    let output_device = output_selection
        .as_ref()
        .and_then(|selection| matched_device(inspector, selection, AudioDirection::Output));
    let mut warnings = Vec::new();
    let virtual_inputs = collect_virtual_inputs(snapshot, module);
    let virtual_outputs = collect_virtual_outputs(snapshot, module)?;
    let input_ids = virtual_inputs.iter().map(|(_, id, _)| (*id, ())).collect::<HashMap<_, _>>();
    let output_ids = virtual_outputs
        .iter()
        .map(|(_, id, _, _)| (*id, ()))
        .collect::<HashMap<_, _>>();

    let input_patch = collect_input_patch(
        snapshot,
        module,
        input_selection
            .as_ref()
            .map(|selection| selection.profile_key.as_str()),
        input_device,
        &input_ids,
        &mut warnings,
    )?;
    let output_patch = collect_output_patch(
        snapshot,
        module,
        output_selection
            .as_ref()
            .map(|selection| selection.profile_key.as_str()),
        output_device,
        &output_ids,
        &mut warnings,
    )?;
    let monitoring = collect_monitoring(snapshot, module, &input_ids, &output_ids, &mut warnings)?;
    let playback_patch = collect_playback(snapshot, module, &output_ids, &mut warnings)?;
    let analysis_taps = collect_analysis(snapshot, module, &input_ids, &mut warnings)?;

    let configuration = AudioConfiguration {
        enabled: snapshot.node(module).is_some_and(|node| node.enabled),
        input: DirectionConfiguration {
            enabled: input_enabled,
            device: if input_enabled { input_selection } else { None },
            recovery_policy,
            buffer_policy,
        },
        output: DirectionConfiguration {
            enabled: output_enabled,
            device: if output_enabled {
                output_selection
            } else {
                None
            },
            recovery_policy,
            buffer_policy,
        },
        virtual_inputs: virtual_inputs
            .iter()
            .map(|(_, id, label)| VirtualInputChannel {
                id: *id,
                label: label.clone(),
            })
            .collect(),
        virtual_outputs: virtual_outputs
            .iter()
            .map(|(_, id, label, gain)| VirtualOutputChannel {
                id: *id,
                label: label.clone(),
                gain: *gain,
            })
            .collect(),
        input_patch,
        monitoring,
        playback_patch,
        output_patch,
        analysis_taps,
        master_gain: gain(snapshot, module, "parameters/master_volume_db")?,
    };
    configuration
        .validate(&golden_audio::EngineLimits::default())
        .map_err(|error| configuration_error(module, error.to_string()))?;

    Ok(BuiltSoundCardConfiguration {
        configuration,
        sample_rate,
        bindings: collect_bindings(snapshot, module),
        warnings,
    })
}

pub(super) fn device_target_value(target: &AudioDeviceTargetId) -> String {
    serde_json::to_string(target).expect("audio device targets serialize")
}

fn resolve_selection(
    inspector: &AudioDeviceInspectorState,
    value: &str,
    direction: AudioDirection,
) -> Option<AudioDeviceSelection> {
    let target = if value == SYSTEM_DEFAULT_INPUT || value == SYSTEM_DEFAULT_OUTPUT {
        preferred_default_target(inspector, direction)?
    } else {
        serde_json::from_str::<AudioDeviceTargetId>(value).ok()?
    };
    match &target {
        AudioDeviceTargetId::SystemDefault { backend } => Some(
            AudioDeviceSelection::follow_system_default(backend.clone(), direction),
        ),
        AudioDeviceTargetId::Device { .. } => inspector
            .devices
            .iter()
            .find(|device| device.target == target)
            .map(AudioDeviceSelection::from_descriptor)
            .or_else(|| {
                Some(AudioDeviceSelection {
                    profile_key: profile_key_for(&target, None, Some(direction)),
                    last_known_label: format!("{target:?}"),
                    fallback_fingerprint: None,
                    target,
                })
            }),
    }
}

fn preferred_default_target(
    inspector: &AudioDeviceInspectorState,
    direction: AudioDirection,
) -> Option<AudioDeviceTargetId> {
    inspector
        .backends
        .iter()
        .filter(|backend| backend.state == golden_audio::AudioBackendState::Available)
        .find_map(|backend| {
            inspector
                .devices
                .iter()
                .any(|device| {
                    device.target.backend() == &backend.backend
                        && device.supports(direction)
                        && match direction {
                            AudioDirection::Input => device.is_system_default_input,
                            AudioDirection::Output => device.is_system_default_output,
                        }
                })
                .then(|| AudioDeviceTargetId::SystemDefault {
                    backend: backend.backend.clone(),
                })
        })
}

fn matched_device<'a>(
    inspector: &'a AudioDeviceInspectorState,
    selection: &AudioDeviceSelection,
    direction: AudioDirection,
) -> Option<&'a AudioDeviceDescriptor> {
    match match_device_selection(selection, direction, &inspector.devices) {
        AudioDeviceMatch::Matched(device) => inspector
            .devices
            .iter()
            .find(|candidate| candidate.target == device.target),
        AudioDeviceMatch::Missing | AudioDeviceMatch::Ambiguous(_) => None,
    }
}

fn collect_virtual_inputs(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
) -> Vec<(NodeId, AudioChannelId, String)> {
    typed_children(snapshot, find_path(snapshot, module, VIRTUAL_INPUTS_PATH), SoundCardVirtualInput::NODE_TYPE)
        .into_iter()
        .filter_map(|node| {
            let state = snapshot.node(node)?;
            Some((node, AudioChannelId::from_uuid(state.uuid.0), state.label.clone()))
        })
        .collect()
}

fn collect_virtual_outputs(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
) -> Result<Vec<(NodeId, AudioChannelId, String, GainDb)>, ConfigurationError> {
    typed_children(
        snapshot,
        find_path(snapshot, module, VIRTUAL_OUTPUTS_PATH),
        SoundCardVirtualOutput::NODE_TYPE,
    )
    .into_iter()
    .map(|node| {
        let state = snapshot
            .node(node)
            .ok_or_else(|| configuration_error(node, "virtual output disappeared"))?;
        Ok((
            node,
            AudioChannelId::from_uuid(state.uuid.0),
            state.label.clone(),
            child_gain(snapshot, node, "volume_db")?,
        ))
    })
    .collect()
}

fn collect_input_patch(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    profile_key: Option<&str>,
    device: Option<&AudioDeviceDescriptor>,
    inputs: &HashMap<AudioChannelId, ()>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Result<Vec<InputPatchRoute>, ConfigurationError> {
    let Some(profile) = matching_profile(
        snapshot,
        find_path(snapshot, module, INPUT_PROFILES_PATH),
        SoundCardInputProfile::NODE_TYPE,
        profile_key,
    ) else {
        return Ok(Vec::new());
    };
    typed_children(snapshot, Some(profile), SoundCardInputPatchRoute::NODE_TYPE)
        .into_iter()
        .filter_map(|route| {
            let destination = match resolve_channel_reference(
                snapshot,
                route,
                "virtual_input",
                SoundCardVirtualInput::NODE_TYPE,
                inputs,
                warnings,
            ) {
                Ok(Some(destination)) => destination,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            let authored = child_string(snapshot, route, "physical_channel", "channel_1");
            let source = resolve_physical_channel(
                route,
                authored.as_str(),
                device.map(|device| device.input_channels.as_slice()),
                warnings,
            )?;
            let gain = match child_gain(snapshot, route, "gain_db") {
                Ok(gain) => gain,
                Err(error) => return Some(Err(error)),
            };
            Some(Ok(InputPatchRoute {
                id: route_id(snapshot, route),
                source,
                destination,
                gain,
            }))
        })
        .collect()
}

fn collect_output_patch(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    profile_key: Option<&str>,
    device: Option<&AudioDeviceDescriptor>,
    outputs: &HashMap<AudioChannelId, ()>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Result<Vec<OutputPatchRoute>, ConfigurationError> {
    let Some(profile) = matching_profile(
        snapshot,
        find_path(snapshot, module, OUTPUT_PROFILES_PATH),
        SoundCardOutputProfile::NODE_TYPE,
        profile_key,
    ) else {
        return Ok(Vec::new());
    };
    typed_children(snapshot, Some(profile), SoundCardOutputPatchRoute::NODE_TYPE)
        .into_iter()
        .filter_map(|route| {
            let source = match resolve_channel_reference(
                snapshot,
                route,
                "virtual_output",
                SoundCardVirtualOutput::NODE_TYPE,
                outputs,
                warnings,
            ) {
                Ok(Some(source)) => source,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            let authored = child_string(snapshot, route, "physical_channel", "channel_1");
            let destination = resolve_physical_channel(
                route,
                authored.as_str(),
                device.map(|device| device.output_channels.as_slice()),
                warnings,
            )?;
            let gain = match child_gain(snapshot, route, "gain_db") {
                Ok(gain) => gain,
                Err(error) => return Some(Err(error)),
            };
            Some(Ok(OutputPatchRoute {
                id: route_id(snapshot, route),
                source,
                destination,
                gain,
            }))
        })
        .collect()
}

fn collect_monitoring(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    inputs: &HashMap<AudioChannelId, ()>,
    outputs: &HashMap<AudioChannelId, ()>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Result<Vec<MonitorRoute>, ConfigurationError> {
    let parent = find_path(snapshot, module, "parameters/monitoring_routes");
    typed_children(snapshot, parent, SoundCardMonitorRoute::NODE_TYPE)
        .into_iter()
        .filter_map(|route| {
            let source = match resolve_channel_reference(
                snapshot,
                route,
                "virtual_input",
                SoundCardVirtualInput::NODE_TYPE,
                inputs,
                warnings,
            ) {
                Ok(Some(value)) => value,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            let destination = match resolve_channel_reference(
                snapshot,
                route,
                "virtual_output",
                SoundCardVirtualOutput::NODE_TYPE,
                outputs,
                warnings,
            ) {
                Ok(Some(value)) => value,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            let gain = match child_gain(snapshot, route, "gain_db") {
                Ok(gain) => gain,
                Err(error) => return Some(Err(error)),
            };
            Some(Ok(MonitorRoute {
                id: route_id(snapshot, route),
                source,
                destination,
                gain,
            }))
        })
        .collect()
}

fn collect_playback(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    outputs: &HashMap<AudioChannelId, ()>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Result<Vec<PlaybackRoute>, ConfigurationError> {
    let parent = find_path(snapshot, module, "parameters/playback_routes");
    typed_children(snapshot, parent, SoundCardPlaybackRoute::NODE_TYPE)
        .into_iter()
        .filter_map(|route| {
            let destination = match resolve_channel_reference(
                snapshot,
                route,
                "virtual_output",
                SoundCardVirtualOutput::NODE_TYPE,
                outputs,
                warnings,
            ) {
                Ok(Some(value)) => value,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            let gain = match child_gain(snapshot, route, "gain_db") {
                Ok(gain) => gain,
                Err(error) => return Some(Err(error)),
            };
            Some(Ok(PlaybackRoute {
                id: route_id(snapshot, route),
                source_channel: child_int(snapshot, route, "source_channel", 1)
                    .saturating_sub(1)
                    .max(0) as u16,
                destination,
                gain,
            }))
        })
        .collect()
}

fn collect_analysis(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    inputs: &HashMap<AudioChannelId, ()>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Result<Vec<AnalysisTapConfiguration>, ConfigurationError> {
    let Some(parent) = find_path(snapshot, module, ANALYSIS_PATH) else {
        return Ok(Vec::new());
    };
    snapshot
        .child_ids_slice(parent)
        .iter()
        .copied()
        .filter_map(|analyzer| {
            let state = snapshot.node(analyzer)?;
            let source = match resolve_channel_reference(
                snapshot,
                analyzer,
                "source",
                SoundCardVirtualInput::NODE_TYPE,
                inputs,
                warnings,
            ) {
                Ok(Some(value)) => value,
                Ok(None) => return None,
                Err(error) => return Some(Err(error)),
            };
            let processor = if state.node_type == SoundCardPitchAnalyzer::NODE_TYPE {
                AnalysisProcessorConfiguration::Pitch(PitchAnalysisConfiguration {
                    frame_size: child_int(snapshot, analyzer, "frame_size", 2_048).max(1) as u32,
                    minimum_frequency_hz: child_float(
                        snapshot,
                        analyzer,
                        "minimum_frequency_hz",
                        50.0,
                    ) as f32,
                    maximum_frequency_hz: child_float(
                        snapshot,
                        analyzer,
                        "maximum_frequency_hz",
                        2_000.0,
                    ) as f32,
                    power_threshold: child_float(snapshot, analyzer, "power_threshold", 0.000_1)
                        as f32,
                    yin_threshold: child_float(snapshot, analyzer, "yin_threshold", 0.15) as f32,
                    confidence_threshold: child_float(
                        snapshot,
                        analyzer,
                        "confidence_threshold",
                        0.75,
                    ) as f32,
                })
            } else if state.node_type == SoundCardSpectrumAnalyzer::NODE_TYPE {
                AnalysisProcessorConfiguration::Spectrum(SpectrumAnalysisConfiguration {
                    fft_size: child_int(snapshot, analyzer, "fft_size", 2_048).max(1) as u32,
                    window: match child_enum(snapshot, analyzer, "window", "hann").as_str() {
                        "blackman_harris" => SpectrumWindow::BlackmanHarris,
                        _ => SpectrumWindow::Hann,
                    },
                    overlap: match child_enum(snapshot, analyzer, "overlap", "half").as_str() {
                        "none" => SpectrumOverlap::None,
                        "three_quarters" => SpectrumOverlap::ThreeQuarters,
                        _ => SpectrumOverlap::Half,
                    },
                    spacing: match child_enum(snapshot, analyzer, "spacing", "logarithmic").as_str() {
                        "linear" => SpectrumBandSpacing::Linear,
                        _ => SpectrumBandSpacing::Logarithmic,
                    },
                    minimum_frequency_hz: child_float(
                        snapshot,
                        analyzer,
                        "minimum_frequency_hz",
                        20.0,
                    ) as f32,
                    maximum_frequency_hz: child_float(
                        snapshot,
                        analyzer,
                        "maximum_frequency_hz",
                        20_000.0,
                    ) as f32,
                    band_count: child_int(snapshot, analyzer, "band_count", 64).clamp(1, 256)
                        as u16,
                    attack_ms: (child_float(snapshot, analyzer, "attack", 0.35) * 1_000.0)
                        as f32,
                    release_ms: (child_float(snapshot, analyzer, "release", 0.12) * 1_000.0)
                        as f32,
                })
            } else {
                return None;
            };
            Some(Ok(AnalysisTapConfiguration {
                id: AnalysisTapId::from_uuid(state.uuid.0),
                source,
                enabled: state.enabled,
                processor,
            }))
        })
        .collect()
}

fn resolve_channel_reference(
    snapshot: &ProcessTreeSnapshot,
    owner: NodeId,
    field: &str,
    expected_type: &str,
    known: &HashMap<AudioChannelId, ()>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Result<Option<AudioChannelId>, ConfigurationError> {
    let reference = child_reference(snapshot, owner, field);
    let Some(reference) = reference.filter(|reference| !reference.is_empty()) else {
        warnings.push(unresolved_warning(owner, field, "route target is not selected"));
        return Ok(None);
    };
    let Some(target) = snapshot.node_id_by_uuid(reference.uuid()) else {
        warnings.push(unresolved_warning(
            owner,
            field,
            "route target is missing; the route remains authored and will recover after undo or restore",
        ));
        return Ok(None);
    };
    let target_state = snapshot
        .node(target)
        .ok_or_else(|| configuration_error(owner, "route target disappeared during conversion"))?;
    let channel = AudioChannelId::from_uuid(target_state.uuid.0);
    if target_state.node_type != expected_type || !known.contains_key(&channel) {
        return Err(configuration_error(
            owner,
            format!("route target '{}' does not belong to this Sound Card", target_state.label),
        ));
    }
    Ok(Some(channel))
}

fn resolve_physical_channel(
    owner: NodeId,
    authored: &str,
    available: Option<&[golden_audio::PhysicalChannelDescriptor]>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Option<PhysicalChannelKey> {
    let Some(available) = available else {
        warnings.push(unresolved_warning(
            owner,
            "physical_channel",
            "selected device is missing; physical route is retained but inactive",
        ));
        return None;
    };
    if let Some(channel) = available.iter().find(|channel| channel.key.as_str() == authored) {
        return Some(channel.key.clone());
    }
    let alias_index = authored
        .strip_prefix("channel_")
        .and_then(|index| index.parse::<usize>().ok())
        .and_then(|index| index.checked_sub(1));
    if let Some(channel) = alias_index.and_then(|index| available.get(index)) {
        return Some(channel.key.clone());
    }
    warnings.push(unresolved_warning(
        owner,
        "physical_channel",
        format!("physical channel '{authored}' is unavailable"),
    ));
    None
}
