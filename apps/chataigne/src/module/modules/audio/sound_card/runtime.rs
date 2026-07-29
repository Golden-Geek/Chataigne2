use std::{
    collections::{BTreeSet, HashMap, HashSet},
    time::Duration,
};

use chataigne_sound_card_protocol::{SoundCardPlaybackVoiceDto, SoundCardUiTelemetryDto};
use golden_audio::{
    AnalysisProcessorConfiguration, AnalysisResult, AnalysisTapConfiguration, AnalysisTapId, AnalysisTapObservation,
    AudioBufferPolicy, AudioChannelId, AudioCommand, AudioConfiguration, AudioDeviceDescriptor,
    AudioDeviceInspectorState, AudioDeviceMatch, AudioDeviceSelection, AudioDeviceTargetId, AudioDirection,
    AudioEngine, AudioEngineBuilder, AudioEvent, AudioEventReceiver, AudioObservationReader, AudioObservationSnapshot,
    AudioRecoveryPolicy, AudioRouteId, BackendId, BackendPolicy, ConfigGeneration, DirectionConfiguration, GainDb,
    InputPatchRoute, NullBackend, OutputPatchRoute, PhysicalChannelKey, PitchAnalysisConfiguration, PlaybackId,
    PlaybackRoute, SampleRate, SpectrumAnalysisConfiguration, SpectrumBandSpacing, SpectrumOverlap, SpectrumWindow,
    VirtualInputChannel, VirtualOutputChannel, match_device_selection,
};
use golden_core::{
    node::{NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use uuid::Uuid;

use super::{
    ASIO_AUDIO_DRIVER, AUTOMATIC_CONFIGURATION, INPUT_CHANNELS_PATH, INPUT_ROUTES_PATH, MAX_PLAYBACK_SOURCE_CHANNELS,
    NO_AUDIO_DEVICE, NO_AUDIO_DRIVER, OUTPUT_CHANNELS_PATH, OUTPUT_ROUTES_PATH, PITCH_VALUES_PATH,
    SYSTEM_DEFAULT_DEVICE, LEVELS_UPDATE_RATE_DEFAULT_HZ, LEVELS_UPDATE_RATE_MAX_HZ,
    LEVELS_UPDATE_RATE_MIN_HZ, find_path,
};
use crate::app::module_modules_audio_sound_card_schema::{SoundCardInputRoute, SoundCardOutputRoute};

mod capabilities;
mod events;
mod observation;
mod playback;
mod snapshot;
mod wake;
mod worker;

pub(super) use capabilities::{
    configuration_capabilities, device_selection_value,
    probe_targets_are_pending, requires_disable_before_probe,
    selected_devices_are_available, selected_physical_channels,
    selected_probe_targets,
    supported_buffer_sizes, supported_sample_rates, supports_system_default,
};
use observation::{collect_bindings, playback_status_updates, telemetry_from_observation, values_nearly_equal};
#[cfg(test)]
pub(super) fn collect_bindings_for_test(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
) -> SoundCardValueBindings {
    collect_bindings(snapshot, module)
}

#[cfg(test)]
pub(super) fn playback_status_updates_for_test(
    bindings: &SoundCardValueBindings,
    playback: golden_audio::PlaybackObservation,
) -> [(Option<NodeId>, ParamValue); 2] {
    playback_status_updates(bindings, playback)
}

#[cfg(test)]
pub(super) fn output_channel_names_for_test(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
) -> Result<Vec<String>, ConfigurationError> {
    collect_output_channels(snapshot, module)
        .map(|channels| channels.into_iter().map(|channel| channel.label).collect())
}

use snapshot::*;
pub(super) use wake::{RuntimeWakeSender, SOUND_CARD_RUNTIME_WAKE_TOPIC};
pub(super) use worker::{
    SoundCardRuntimeRequest, SoundCardRuntimeStarted, SoundCardRuntimeWorker, SoundCardRuntimeWorkerPoll,
    retire_detached,
};
pub(super) const OBSERVATION_EPSILON: f64 = 0.000_01;
pub(super) const OUTPUT_ACTIVITY_RMS_THRESHOLD: f32 = 0.01;

pub(super) fn levels_update_rate_hz(value: i32) -> u32 {
    value
        .clamp(LEVELS_UPDATE_RATE_MIN_HZ, LEVELS_UPDATE_RATE_MAX_HZ)
        .try_into()
        .unwrap_or(LEVELS_UPDATE_RATE_DEFAULT_HZ as u32)
}

pub(super) fn bounded_levels_update_rate_hz(value: u32) -> u32 {
    value.clamp(
        LEVELS_UPDATE_RATE_MIN_HZ as u32,
        LEVELS_UPDATE_RATE_MAX_HZ as u32,
    )
}

pub(super) fn levels_update_interval(rate_hz: u32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(bounded_levels_update_rate_hz(rate_hz)))
}

pub(super) fn has_output_activity(enabled: bool, output_global_max_rms: f32) -> bool {
    enabled
        && output_global_max_rms.is_finite()
        && output_global_max_rms >= OUTPUT_ACTIVITY_RMS_THRESHOLD
}

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
struct PitchBinding {
    valid: NodeId,
    frequency_hz: NodeId,
    confidence: NodeId,
    midi_note: NodeId,
    note_name: NodeId,
    cents: NodeId,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SoundCardValueBindings {
    input_master_level: Option<NodeId>,
    output_master_level: Option<NodeId>,
    active_voices: Option<NodeId>,
    loading_voices: Option<NodeId>,
    channel_levels: HashMap<AudioChannelId, NodeId>,
    pitch: HashMap<AnalysisTapId, PitchBinding>,
    runtime_values: HashSet<NodeId>,
}

impl SoundCardValueBindings {
    pub(super) fn is_runtime_value(&self, node: NodeId) -> bool {
        self.runtime_values.contains(&node)
    }
}

#[derive(Debug)]
pub(crate) struct SoundCardRuntime {
    engine: AudioEngine,
    events: Option<AudioEventReceiver>,
    event_bridge: Option<events::SoundCardRuntimeEvents>,
    observations: AudioObservationReader,
    generation: ConfigGeneration,
    sample_rate: SampleRate,
    driver: Option<BackendId>,
    probe_interests: Vec<AudioDeviceTargetId>,
    levels_update_rate_hz: u32,
    bindings: SoundCardValueBindings,
    last_values: HashMap<NodeId, ParamValue>,
    active_playback_voices: HashMap<PlaybackId, SoundCardPlaybackVoiceDto>,
    last_telemetry: Option<SoundCardUiTelemetryDto>,
    last_error: Option<String>,
    outgoing_activity: Option<bool>,
    pending_outgoing_activity: Option<bool>,
}

impl SoundCardRuntime {
    pub(super) fn start_selected(sample_rate: SampleRate, driver: Option<BackendId>) -> Result<Self, String> {
        let driver =
            driver.ok_or_else(|| "Sound Card runtime requires an explicitly selected audio driver".to_owned())?;
        let mut builder = AudioEngineBuilder::default()
            .without_backends()
            .with_managed_render_runtime();
        builder.config.sample_rate = sample_rate;
        if driver == NullBackend::backend_id() {
            builder = builder.with_backend(NullBackend);
        } else {
            let backend = golden_audio::cpal_backend_by_id(&driver)
                .ok_or_else(|| format!("audio driver `{driver}` is not compiled into this build"))?;
            builder = builder.with_boxed_backend(backend);
        }
        builder.backend_policy = BackendPolicy {
            preferred: vec![driver.clone()],
            allow_null_fallback: false,
        };
        let mut engine = builder.build().map_err(|error| error.to_string())?;
        let events = engine
            .take_event_receiver()
            .ok_or_else(|| "golden_audio event receiver was already taken".to_owned())?;
        Ok(Self {
            observations: engine.observations(),
            engine,
            events: Some(events),
            event_bridge: None,
            generation: ConfigGeneration::INITIAL,
            sample_rate,
            driver: Some(driver),
            probe_interests: Vec::new(),
            levels_update_rate_hz: LEVELS_UPDATE_RATE_DEFAULT_HZ as u32,
            bindings: SoundCardValueBindings::default(),
            last_values: HashMap::new(),
            active_playback_voices: HashMap::new(),
            last_telemetry: None,
            last_error: None,
            outgoing_activity: None,
            pending_outgoing_activity: None,
        })
    }

    pub(super) fn inspector_state(&self) -> AudioDeviceInspectorState {
        self.observations.latest().device
    }

    pub(super) const fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    pub(super) fn driver(&self) -> Option<&BackendId> {
        self.driver.as_ref()
    }

    pub(super) fn bindings(&self) -> &SoundCardValueBindings {
        &self.bindings
    }

    pub(super) fn set_probe_interests(
        &mut self,
        targets: Vec<AudioDeviceTargetId>,
    ) -> Result<(), String> {
        if self.probe_interests == targets {
            return Ok(());
        }
        self.engine
            .control()
            .set_device_probe_interests(targets.clone())
            .map_err(|error| error.to_string())?;
        self.probe_interests = targets;
        Ok(())
    }

    pub(super) fn enable_notifications(&mut self, wake: RuntimeWakeSender) -> Result<(), String> {
        if self.event_bridge.is_some() {
            return Ok(());
        }
        let events = self
            .events
            .take()
            .ok_or_else(|| "Sound Card audio event receiver is unavailable".to_owned())?;
        self.event_bridge = Some(events::SoundCardRuntimeEvents::spawn(
            events,
            wake,
            self.levels_update_rate_hz,
        )?);
        Ok(())
    }

    pub(super) fn set_levels_update_rate_hz(&mut self, rate_hz: u32) {
        let rate_hz = bounded_levels_update_rate_hz(rate_hz);
        self.levels_update_rate_hz = rate_hz;
        if let Some(event_bridge) = &self.event_bridge {
            event_bridge.set_refresh_rate_hz(rate_hz);
        }
    }

    pub(super) fn take_outgoing_activity_change(&mut self) -> Option<bool> {
        self.pending_outgoing_activity.take()
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

    pub(super) fn admit(
        &self,
        command: AudioCommand,
    ) -> Result<golden_audio::CommandSequence, golden_audio::AudioError> {
        self.engine.control().submit(command)
    }

    pub(super) fn poll(
        &mut self,
        ctx: &mut ProcessCtx,
    ) -> (
        AudioObservationSnapshot,
        Option<SoundCardUiTelemetryDto>,
        Vec<AudioEvent>,
    ) {
        let events = self
            .event_bridge
            .as_ref()
            .map(events::SoundCardRuntimeEvents::drain)
            .or_else(|| self.events.as_ref().map(AudioEventReceiver::drain))
            .unwrap_or_default();
        for event in &events {
            self.observe_event(event);
        }
        let observation = self.observations.latest();
        let outgoing_activity =
            has_output_activity(observation.enabled, observation.output_global_max_rms);
        if self.outgoing_activity != Some(outgoing_activity) {
            self.outgoing_activity = Some(outgoing_activity);
            self.pending_outgoing_activity = Some(outgoing_activity);
        }
        self.apply_observation_values(ctx, &observation);
        let mut playback_voices = self.active_playback_voices.values().cloned().collect::<Vec<_>>();
        playback_voices.sort_by(|left, right| left.playback_id.cmp(&right.playback_id));
        let telemetry = telemetry_from_observation(&observation, playback_voices);
        let changed = self.last_telemetry.as_ref() != Some(&telemetry);
        if changed {
            self.last_telemetry = Some(telemetry.clone());
        }
        (observation, changed.then_some(telemetry), events)
    }

    pub(super) fn stop(&mut self) {
        let _ = self.engine.shutdown();
        if let Some(mut event_bridge) = self.event_bridge.take() {
            event_bridge.join();
        }
    }

    fn apply_observation_values(&mut self, ctx: &mut ProcessCtx, observation: &AudioObservationSnapshot) {
        self.set_float(ctx, self.bindings.input_master_level, observation.input_global_max_rms);
        self.set_float(
            ctx,
            self.bindings.output_master_level,
            observation.output_global_max_rms,
        );
        for (node, value) in playback_status_updates(&self.bindings, observation.playback) {
            self.set_value(ctx, node, value);
        }
        for channel in observation.inputs.iter().chain(&observation.outputs) {
            self.set_float(
                ctx,
                self.bindings.channel_levels.get(&channel.channel).copied(),
                channel.rms_linear,
            );
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
                self.set_value(ctx, Some(binding.note_name), ParamValue::Str(value.note_name.clone()));
                self.set_float(ctx, Some(binding.cents), value.cents);
            }
            Some(AnalysisResult::Spectrum(_)) => {}
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
    let driver_value = child_enum(snapshot, connection, "audio_driver", NO_AUDIO_DRIVER);
    let uses_duplex_device = driver_value == ASIO_AUDIO_DRIVER;
    let driver = if driver_value == NO_AUDIO_DRIVER {
        None
    } else {
        Some(BackendId::new(driver_value).map_err(|error| configuration_error(connection, error.to_string()))?)
    };
    let (input_value, output_value, input_parameter, output_parameter) =
        if uses_duplex_device {
            let (parameter, value) = required_child_enum(snapshot, connection, "device")?;
            (value.clone(), value, parameter, parameter)
        } else {
            let (input_parameter, input_value) =
                required_child_enum(snapshot, connection, "input_device")?;
            let (output_parameter, output_value) =
                required_child_enum(snapshot, connection, "output_device")?;
            (input_value, output_value, input_parameter, output_parameter)
        };
    let input_selection = resolve_selection(driver.as_ref(), inspector, input_value.as_str(), AudioDirection::Input)?;
    let output_selection = resolve_selection(
        driver.as_ref(),
        inspector,
        output_value.as_str(),
        AudioDirection::Output,
    )?;

    let input_device = input_selection
        .as_ref()
        .and_then(|selection| matched_device(inspector, selection, AudioDirection::Input));
    let output_device = output_selection
        .as_ref()
        .and_then(|selection| matched_device(inspector, selection, AudioDirection::Output));
    if input_selection.is_some() && input_device.is_none() {
        return Err(configuration_error(
            input_parameter,
            "selected input device is not currently available",
        ));
    }
    if output_selection.is_some() && output_device.is_none() {
        return Err(configuration_error(
            output_parameter,
            "selected output device is not currently available",
        ));
    }
    let physical_inputs: Vec<PhysicalChannelKey> = input_device
        .map(|device| {
            device
                .input_channels
                .iter()
                .map(|channel| channel.key.clone())
                .collect()
        })
        .unwrap_or_default();
    let physical_outputs: Vec<PhysicalChannelKey> = output_device
        .map(|device| {
            device
                .output_channels
                .iter()
                .map(|channel| channel.key.clone())
                .collect()
        })
        .unwrap_or_default();
    let available_inputs = physical_inputs.iter().cloned().collect::<HashSet<_>>();
    let available_outputs = physical_outputs.iter().cloned().collect::<HashSet<_>>();
    let sample_rate_value = child_enum(snapshot, connection, "sample_rate", AUTOMATIC_CONFIGURATION);
    let buffer_value = child_enum(snapshot, connection, "buffer_size", AUTOMATIC_CONFIGURATION);
    let fixed_buffer = parse_fixed_value(buffer_value.as_str());
    let sample_rates = supported_sample_rates(input_device, output_device, fixed_buffer);
    if (input_device.is_some() || output_device.is_some()) && sample_rates.is_empty() {
        return Err(configuration_error(
            child_id(snapshot, connection, "sample_rate"),
            "selected devices have no compatible sample-rate configuration",
        ));
    }
    let sample_rate = resolve_sample_rate(sample_rate_value.as_str(), sample_rates.as_slice(), connection)?;
    let buffer_sizes = supported_buffer_sizes(input_device, output_device, Some(sample_rate.get()));
    if (input_device.is_some() || output_device.is_some()) && buffer_sizes.is_empty() {
        return Err(configuration_error(
            child_id(snapshot, connection, "buffer_size"),
            "selected devices have no compatible buffer-size configuration",
        ));
    }
    let buffer_policy = resolve_buffer_policy(buffer_value.as_str(), buffer_sizes.as_slice(), connection)?;

    let input_channels = collect_input_channels(snapshot, module)?;
    let output_channels = collect_output_channels(snapshot, module)?;
    let input_ids = input_channels
        .iter()
        .map(|channel| (channel.node, channel.id))
        .collect::<HashMap<_, _>>();
    let output_ids = output_channels
        .iter()
        .map(|channel| (channel.node, channel.id))
        .collect::<HashMap<_, _>>();
    let input_master = gain(snapshot, module, "parameters/input/master_gain_db")?;
    let output_master = gain(snapshot, module, "parameters/output/master_gain_db")?;
    let mut warnings = Vec::new();
    let input_patch = collect_input_routes(
        snapshot,
        module,
        &input_ids,
        input_master,
        &available_inputs,
        &mut warnings,
    )?;
    let output_patch = collect_output_routes(snapshot, module, &output_ids, &available_outputs, &mut warnings)?;
    let playback_patch = output_channels
        .iter()
        .enumerate()
        .map(|(index, channel)| PlaybackRoute {
            id: derived_route_id(channel.uuid, b"sound-card-playback-route"),
            source_channel: u16::try_from(index).unwrap_or(u16::MAX),
            destination: channel.id,
            gain: GainDb::UNITY,
        })
        .collect();
    let analysis_taps = collect_analysis(snapshot, module, input_channels.first().map(|channel| channel.id));

    let configuration = AudioConfiguration {
        enabled: snapshot.node(module).is_some_and(|node| node.enabled),
        input: direction_configuration(input_selection, buffer_policy),
        output: direction_configuration(output_selection, buffer_policy),
        physical_inputs,
        physical_outputs,
        virtual_inputs: input_channels
            .iter()
            .map(|channel| VirtualInputChannel {
                id: channel.id,
                label: channel.label.clone(),
            })
            .collect(),
        virtual_outputs: output_channels
            .iter()
            .map(|channel| VirtualOutputChannel {
                id: channel.id,
                label: channel.label.clone(),
                gain: channel.gain,
            })
            .collect(),
        input_patch,
        monitoring: Vec::new(),
        playback_patch,
        output_patch,
        analysis_taps,
        master_gain: output_master,
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

fn collect_input_routes(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    input_ids: &HashMap<NodeId, AudioChannelId>,
    master_gain: GainDb,
    available_physical_channels: &HashSet<PhysicalChannelKey>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Result<Vec<InputPatchRoute>, ConfigurationError> {
    let mut routes = Vec::new();
    for route in typed_children(
        snapshot,
        find_path(snapshot, module, INPUT_ROUTES_PATH),
        SoundCardInputRoute::NODE_TYPE,
    ) {
        let Some(channel) = child_reference(snapshot, route, "channel") else {
            warnings.push(unresolved_warning(route, "channel", "Input route channel is missing"));
            continue;
        };
        let Some(channel_node) = snapshot.node_id_by_uuid(channel.uuid()) else {
            warnings.push(unresolved_warning(
                route,
                "channel",
                "Input route channel is unavailable",
            ));
            continue;
        };
        let Some(destination) = input_ids.get(&channel_node).copied() else {
            warnings.push(unresolved_warning(
                route,
                "channel",
                "Input route target is not an input channel",
            ));
            continue;
        };
        let channel_gain = gain_at_node(snapshot, channel_node)?;
        let gain = combined_gain(route, master_gain, channel_gain)?;
        let source = PhysicalChannelKey::new(child_string(snapshot, route, "physical_channel", ""))
            .map_err(|error| configuration_error(route, error.to_string()))?;
        if !available_physical_channels.contains(&source) {
            warnings.push(unresolved_warning(
                route,
                "physical-channel",
                format!(
                    "Input route physical channel `{}` is unavailable; the authored route is retained for recovery",
                    source.as_str()
                ),
            ));
            continue;
        }
        routes.push(InputPatchRoute {
            id: route_id(snapshot, route),
            source,
            destination,
            gain,
        });
    }
    Ok(routes)
}

fn collect_output_routes(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    output_ids: &HashMap<NodeId, AudioChannelId>,
    available_physical_channels: &HashSet<PhysicalChannelKey>,
    warnings: &mut Vec<RuntimeWarning>,
) -> Result<Vec<OutputPatchRoute>, ConfigurationError> {
    let mut routes = Vec::new();
    for route in typed_children(
        snapshot,
        find_path(snapshot, module, OUTPUT_ROUTES_PATH),
        SoundCardOutputRoute::NODE_TYPE,
    ) {
        let Some(channel) = child_reference(snapshot, route, "channel") else {
            warnings.push(unresolved_warning(route, "channel", "Output route channel is missing"));
            continue;
        };
        let Some(channel_node) = snapshot.node_id_by_uuid(channel.uuid()) else {
            warnings.push(unresolved_warning(
                route,
                "channel",
                "Output route channel is unavailable",
            ));
            continue;
        };
        let Some(source) = output_ids.get(&channel_node).copied() else {
            warnings.push(unresolved_warning(
                route,
                "channel",
                "Output route source is not an output channel",
            ));
            continue;
        };
        let destination = PhysicalChannelKey::new(child_string(snapshot, route, "physical_channel", ""))
            .map_err(|error| configuration_error(route, error.to_string()))?;
        if !available_physical_channels.contains(&destination) {
            warnings.push(unresolved_warning(
                route,
                "physical-channel",
                format!(
                    "Output route physical channel `{}` is unavailable; the authored route is retained for recovery",
                    destination.as_str()
                ),
            ));
            continue;
        }
        routes.push(OutputPatchRoute {
            id: route_id(snapshot, route),
            source,
            destination,
            gain: GainDb::UNITY,
        });
    }
    Ok(routes)
}

fn collect_analysis(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    source: Option<AudioChannelId>,
) -> Vec<AnalysisTapConfiguration> {
    let Some(source) = source else {
        return Vec::new();
    };
    let namespace = snapshot.node(module).expect("module exists").uuid;
    let pitch_enabled = find_path(snapshot, module, "parameters/processing/pitch_detection")
        .and_then(|node| snapshot.node(node))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_bool)
        .unwrap_or(false);
    let spectral_enabled = super::structure::spectral_analysis_enabled(snapshot, module);
    let mut taps = Vec::with_capacity(2);
    if pitch_enabled {
        taps.push(AnalysisTapConfiguration {
            id: pitch_tap_id(namespace),
            source,
            enabled: true,
            processor: AnalysisProcessorConfiguration::Pitch(PitchAnalysisConfiguration {
                frame_size: 2_048,
                minimum_frequency_hz: 50.0,
                maximum_frequency_hz: 2_000.0,
                power_threshold: 0.000_1,
                yin_threshold: 0.15,
                confidence_threshold: 0.75,
            }),
        });
    }
    if spectral_enabled {
        taps.push(AnalysisTapConfiguration {
            id: spectral_tap_id(namespace),
            source,
            enabled: true,
            processor: AnalysisProcessorConfiguration::Spectrum(SpectrumAnalysisConfiguration {
                fft_size: 2_048,
                window: SpectrumWindow::Hann,
                overlap: SpectrumOverlap::Half,
                spacing: SpectrumBandSpacing::Logarithmic,
                minimum_frequency_hz: 20.0,
                maximum_frequency_hz: 20_000.0,
                band_count: 64,
                attack_ms: 35.0,
                release_ms: 120.0,
            }),
        });
    }
    taps
}

fn direction_configuration(
    selection: Option<AudioDeviceSelection>,
    buffer_policy: AudioBufferPolicy,
) -> DirectionConfiguration {
    let enabled = selection.is_some();
    let recovery_policy = selection
        .as_ref()
        .map_or(AudioRecoveryPolicy::WaitForSelected, |selection| {
            match selection.target {
                AudioDeviceTargetId::SystemDefault { .. } => AudioRecoveryPolicy::FollowSystemDefault,
                AudioDeviceTargetId::Device { .. } => AudioRecoveryPolicy::WaitForSelected,
            }
        });
    DirectionConfiguration {
        enabled,
        device: selection,
        recovery_policy,
        buffer_policy,
    }
}

fn resolve_selection(
    driver: Option<&BackendId>,
    inspector: &AudioDeviceInspectorState,
    value: &str,
    direction: AudioDirection,
) -> Result<Option<AudioDeviceSelection>, ConfigurationError> {
    let Some(driver) = driver else {
        return Ok(None);
    };
    if value == NO_AUDIO_DEVICE {
        return Ok(None);
    }
    if value == SYSTEM_DEFAULT_DEVICE {
        if !supports_system_default(driver) {
            return Err(configuration_error(
                NodeId(0),
                format!("audio driver `{driver}` has no system-default device"),
            ));
        }
        return Ok(Some(AudioDeviceSelection::follow_system_default(
            driver.clone(),
            direction,
        )));
    }
    let selection = serde_json::from_str::<AudioDeviceSelection>(value)
        .map_err(|error| configuration_error(NodeId(0), format!("invalid audio device selection: {error}")))?;
    if selection.target.backend() != driver {
        return Err(configuration_error(
            NodeId(0),
            "selected audio device belongs to another audio driver",
        ));
    }
    if let Some(descriptor) = inspector
        .devices
        .iter()
        .find(|device| device.target == selection.target && device.supports(direction))
    {
        Ok(Some(AudioDeviceSelection::from_descriptor(descriptor)))
    } else {
        Ok(Some(selection))
    }
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

fn parse_fixed_value(value: &str) -> Option<u32> {
    (value != AUTOMATIC_CONFIGURATION)
        .then(|| value.parse::<u32>().ok())
        .flatten()
}

fn preferred_rate(supported: &[u32]) -> Option<u32> {
    [48_000, 44_100]
        .into_iter()
        .find(|candidate| supported.contains(candidate))
        .or_else(|| supported.first().copied())
}

fn resolve_sample_rate(value: &str, supported: &[u32], node: NodeId) -> Result<SampleRate, ConfigurationError> {
    let value = if value == AUTOMATIC_CONFIGURATION {
        preferred_rate(supported).unwrap_or(48_000)
    } else {
        value
            .parse::<u32>()
            .map_err(|_| configuration_error(node, format!("invalid sample rate `{value}`")))?
    };
    if !supported.is_empty() && !supported.contains(&value) {
        return Err(configuration_error(
            node,
            format!("sample rate {value} Hz is unavailable for the selected devices"),
        ));
    }
    SampleRate::new(value).map_err(|error| configuration_error(node, error.to_string()))
}

fn resolve_buffer_policy(
    value: &str,
    supported: &[u32],
    node: NodeId,
) -> Result<AudioBufferPolicy, ConfigurationError> {
    if value == AUTOMATIC_CONFIGURATION {
        return Ok(AudioBufferPolicy::Automatic);
    }
    let frames = value
        .parse::<u32>()
        .map_err(|_| configuration_error(node, format!("invalid buffer size `{value}`")))?;
    if !supported.is_empty() && !supported.contains(&frames) {
        return Err(configuration_error(
            node,
            format!("buffer size {frames} is unavailable for the selected devices"),
        ));
    }
    Ok(AudioBufferPolicy::Fixed(frames))
}

fn combined_gain(node: NodeId, first: GainDb, second: GainDb) -> Result<GainDb, ConfigurationError> {
    GainDb::new((first.get() + second.get()).clamp(GainDb::SILENCE_DB, GainDb::MAX_DB))
        .map_err(|error| configuration_error(node, error.to_string()))
}

fn derived_route_id(namespace: NodeUuid, key: &[u8]) -> AudioRouteId {
    AudioRouteId::from_uuid(Uuid::new_v5(&namespace.0, key))
}

pub(super) fn pitch_tap_id(module: NodeUuid) -> AnalysisTapId {
    AnalysisTapId::from_uuid(Uuid::new_v5(&module.0, b"sound-card-pitch-tap"))
}

pub(super) fn spectral_tap_id(module: NodeUuid) -> AnalysisTapId {
    AnalysisTapId::from_uuid(Uuid::new_v5(&module.0, b"sound-card-spectral-tap"))
}
