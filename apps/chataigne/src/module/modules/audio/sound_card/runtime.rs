use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use golden_audio::{
    match_device_selection, AnalysisProcessorConfiguration, AnalysisTapConfiguration, AnalysisTapId, AudioBufferPolicy,
    AudioChannelId, AudioCommand, AudioConfiguration, AudioControl, AudioDeviceDescriptor, AudioDeviceInspectorState,
    AudioDeviceMatch, AudioDeviceSelection, AudioDeviceTargetId, AudioDirection, AudioEngineBuilder,
    AudioRecoveryPolicy, AudioRouteId, BackendId, BackendPolicy, ConfigGeneration, DirectionConfiguration, GainDb,
    InputPatchRoute, NullBackend, OutputPatchRoute, PhysicalChannelKey, PitchAnalysisConfiguration, PlaybackRoute,
    SampleRate, SpectrumAnalysisConfiguration, SpectrumBandSpacing, SpectrumOverlap, SpectrumWindow,
    VirtualInputChannel, VirtualOutputChannel,
};
use golden_core::{
    node::{NodeId, NodeReference, NodeUuid},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use uuid::Uuid;

use super::{
    find_path, ASIO_AUDIO_DRIVER, AUTOMATIC_CONFIGURATION, INPUT_CHANNELS_PATH, INPUT_ROUTES_PATH,
    LEVELS_UPDATE_RATE_DEFAULT_HZ, LEVELS_UPDATE_RATE_MAX_HZ, LEVELS_UPDATE_RATE_MIN_HZ, MAX_PLAYBACK_SOURCE_CHANNELS,
    NO_AUDIO_DEVICE, NO_AUDIO_DRIVER, OUTPUT_CHANNELS_PATH, OUTPUT_ROUTES_PATH, PITCH_VALUES_PATH,
    SYSTEM_DEFAULT_DEVICE,
};
use crate::app::module_modules_audio_sound_card_schema::{SoundCardInputRoute, SoundCardOutputRoute};

mod capabilities;
mod configuration;
#[cfg(test)]
pub(super) use configuration::build_configuration;
#[cfg(test)]
pub(super) use configuration::choices::options::{device_options_for_current as native_device_options, DeviceChoice};
pub(super) use configuration::choices::ChoicePlan;
pub(super) use configuration::preparation::{Preparation, PreparedConfiguration};
use configuration::selection::{ProbeTargets, SelectionPlan};
mod events;
pub(super) mod observation;
mod snapshot;
mod wake;
mod worker;

pub(super) use events::{EVENT_BYTES, EVENT_SERVICE_ITEMS};

#[cfg(test)]
pub(super) use capabilities::device_selection_value;
pub(super) use capabilities::{
    configuration_capabilities, probe_targets_are_pending, requires_disable_before_probe, selected_physical_channels,
    selected_probe_targets, supported_buffer_sizes, supported_sample_rates, supports_system_default,
};
use configuration::warnings::delivery::WarningDelivery;
use configuration::{matched_device, parse_fixed_value, preferred_rate, resolve_selection};
use observation::collect_bindings;
#[cfg(test)]
pub(super) fn collect_bindings_for_test(snapshot: &ProcessTreeSnapshot, module: NodeId) -> SoundCardValueBindings {
    collect_bindings(snapshot, module)
}

#[cfg(test)]
pub(super) fn playback_status_updates_for_test(
    bindings: &SoundCardValueBindings,
    playback: golden_audio::PlaybackObservation,
) -> [(Option<NodeId>, ParamValue); 2] {
    observation::playback_status_updates(bindings, playback)
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
pub(super) use wake::RuntimeWakeSender;
pub(super) use worker::{
    SoundCardRuntimeRequest, SoundCardRuntimeStarted, SoundCardRuntimeWorker, SoundCardRuntimeWorkerPoll,
    StartRequestError,
};
pub(super) const MAX_PLAYBACK_VOICES: u16 = 256;
pub(super) const OBSERVATION_EPSILON: f64 = 0.000_01;
pub(super) const OUTPUT_ACTIVITY_RMS_THRESHOLD: f32 = 0.01;

pub(super) fn direction_ready(status: &golden_audio::AudioStreamStatus) -> bool {
    status.enabled && status.readiness == golden_audio::AudioDeviceReadiness::Ready
}

pub(super) fn levels_update_rate_hz(value: i32) -> u32 {
    value
        .clamp(LEVELS_UPDATE_RATE_MIN_HZ, LEVELS_UPDATE_RATE_MAX_HZ)
        .try_into()
        .unwrap_or(LEVELS_UPDATE_RATE_DEFAULT_HZ as u32)
}

pub(super) fn bounded_levels_update_rate_hz(value: u32) -> u32 {
    value.clamp(LEVELS_UPDATE_RATE_MIN_HZ as u32, LEVELS_UPDATE_RATE_MAX_HZ as u32)
}

pub(super) fn levels_update_interval(rate_hz: u32) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(bounded_levels_update_rate_hz(rate_hz)))
}

pub(super) fn has_output_activity(enabled: bool, output_global_max_rms: f32) -> bool {
    enabled && output_global_max_rms.is_finite() && output_global_max_rms >= OUTPUT_ACTIVITY_RMS_THRESHOLD
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
    pub retryable: bool,
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

pub(super) enum ConfigurationFeedback {
    Applied,
    Rejected(super::prepared_event::PreparedAudioEvent),
}

pub(crate) struct SoundCardRuntime {
    control: AudioControl,
    command_lifetime: Arc<AtomicBool>,
    enabled_pending: Option<bool>,
    enabled_sent: Option<bool>,
    event_bridge: events::SoundCardRuntimeEvents,
    generation: ConfigGeneration,
    sample_rate: SampleRate,
    driver: Option<BackendId>,
    probe_interests: Option<ProbeTargets>,
    levels_update_rate_hz: u32,
    active_configuration: Option<golden_io::RetainedPayload<PreparedConfiguration>>,
    pending_application: Option<(ConfigGeneration, golden_io::RetainedPayload<PreparedConfiguration>)>,
    preparation: Option<Preparation>,
    selection_preparation: Option<Preparation<SelectionPlan>>,
    preparation_quiescing: bool,
    warnings: WarningDelivery,
    preparation_retry_at: Option<std::time::Instant>,
    binding_generation: ConfigGeneration,
    configuration_feedback: Option<ConfigurationFeedback>,
    values: observation::ValueDelivery,
    telemetry: observation::TelemetryDelivery,
    outgoing_activity: Option<bool>,
    pending_outgoing_activity: Option<bool>,
}

impl SoundCardRuntime {
    pub(super) fn start_selected(sample_rate: SampleRate, driver: Option<BackendId>) -> Result<Self, String> {
        let driver =
            driver.ok_or_else(|| "Sound Card runtime requires an explicitly selected audio driver".to_owned())?;
        let mut builder = AudioEngineBuilder::default()
            .without_backends()
            .with_ordered_events()
            .with_managed_render_runtime();
        builder.config.sample_rate = sample_rate;
        builder.limits.max_voices = MAX_PLAYBACK_VOICES;
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
        let lifetime = events::reserve_lifetime()?;
        let mut engine = builder.build().map_err(|error| error.to_string())?;
        let events = engine
            .take_event_receiver()
            .ok_or_else(|| "golden_audio event receiver was already taken".to_owned())?;
        let control = engine.control();
        let event_bridge =
            events::SoundCardRuntimeEvents::spawn(engine, events, lifetime, LEVELS_UPDATE_RATE_DEFAULT_HZ as u32)?;
        Ok(Self {
            control,
            command_lifetime: Arc::new(AtomicBool::new(true)),
            enabled_pending: None,
            enabled_sent: None,
            event_bridge,
            generation: ConfigGeneration::INITIAL,
            sample_rate,
            driver: Some(driver),
            probe_interests: None,
            levels_update_rate_hz: LEVELS_UPDATE_RATE_DEFAULT_HZ as u32,
            active_configuration: None,
            pending_application: None,
            preparation: None,
            selection_preparation: None,
            preparation_quiescing: false,
            warnings: WarningDelivery::default(),
            preparation_retry_at: None,
            binding_generation: ConfigGeneration::INITIAL,
            configuration_feedback: None,
            values: observation::ValueDelivery::default(),
            telemetry: observation::TelemetryDelivery::default(),
            outgoing_activity: None,
            pending_outgoing_activity: None,
        })
    }

    pub(super) fn device_revision(&self) -> u64 {
        self.event_bridge.device_revision()
    }

    pub(super) fn device_inventory(&self) -> Option<observation::DeviceInventory> {
        self.event_bridge
            .observation()
            .inventory
            .catalog(self.event_bridge.device_revision())
    }

    pub(super) fn validate_physical_channel(
        &self,
        direction: AudioDirection,
        key: &str,
    ) -> Result<(), observation::RouteValidationError> {
        let view = self.event_bridge.observation();
        let revision = self.event_bridge.device_revision();
        view.inventory.validate(revision, direction, key)?;
        if self.event_bridge.device_revision() != revision {
            return Err(observation::RouteValidationError::Stale);
        }
        Ok(())
    }

    pub(super) const fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    pub(super) fn driver(&self) -> Option<&BackendId> {
        self.driver.as_ref()
    }

    pub(super) fn bindings(&self) -> &SoundCardValueBindings {
        static EMPTY: std::sync::LazyLock<SoundCardValueBindings> =
            std::sync::LazyLock::new(SoundCardValueBindings::default);
        self.active_configuration
            .as_ref()
            .map(|data| &data.bindings)
            .unwrap_or(&EMPTY)
    }

    pub(super) fn set_probe_interests(
        &mut self,
        selection: golden_io::RetainedPayload<SelectionPlan>,
    ) -> Result<(), String> {
        if selection.device_revision != self.event_bridge.device_revision() {
            return Err("Audio devices changed while preparing the connection; retrying".into());
        }
        if self
            .probe_interests
            .as_ref()
            .is_some_and(|previous| selection.same_interests(previous))
        {
            return Ok(());
        }
        selection.submit(&self.control).map_err(|error| error.to_string())?;
        self.probe_interests = Some(selection.interests());
        Ok(())
    }

    pub(super) fn enable_notifications(&mut self, wake: RuntimeWakeSender) -> Result<(), String> {
        self.event_bridge.enable_notifications(wake)
    }

    pub(super) fn set_levels_update_rate_hz(&mut self, rate_hz: u32) {
        let rate_hz = bounded_levels_update_rate_hz(rate_hz);
        self.levels_update_rate_hz = rate_hz;
        self.event_bridge.set_refresh_rate_hz(rate_hz);
    }

    pub(super) fn take_outgoing_activity_change(&mut self) -> Option<bool> {
        self.pending_outgoing_activity.take()
    }

    pub(super) fn has_current_preparation(&mut self, snapshot: &ProcessTreeSnapshot, module: NodeId) -> bool {
        if self
            .preparation
            .as_ref()
            .is_some_and(|prepared| !prepared.matches(snapshot, module))
        {
            self.preparation = None;
        }
        self.preparation.is_some()
    }

    pub(super) fn take_configuration_feedback(&mut self) -> Option<ConfigurationFeedback> {
        self.configuration_feedback.take()
    }

    pub(super) fn schedule_preparation_retry(&mut self, wake: &RuntimeWakeSender) {
        self.cancel_preparation();
        self.defer_preparation_retry(wake);
    }

    pub(super) fn defer_preparation_retry(&mut self, wake: &RuntimeWakeSender) {
        let delay = Duration::from_millis(250);
        self.preparation_retry_at = Some(std::time::Instant::now() + delay);
        wake.wake_after(delay);
    }

    pub(super) fn take_preparation_retry_due(&mut self) -> bool {
        if self
            .preparation_retry_at
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            self.preparation_retry_at = None;
            true
        } else {
            false
        }
    }

    pub(super) fn is_applying_configuration(&self) -> bool {
        self.pending_application.is_some()
    }

    pub(super) fn take_projection_failure(&mut self) -> bool {
        self.warnings.take_failure() | self.values.take_failure()
    }

    pub(super) fn prepare_selection(
        &mut self,
        snapshot: &std::sync::Arc<ProcessTreeSnapshot>,
        module: NodeId,
        wake: RuntimeWakeSender,
    ) -> Result<Option<golden_io::RetainedPayload<SelectionPlan>>, ConfigurationError> {
        if self
            .selection_preparation
            .as_ref()
            .is_some_and(|request| !request.matches(snapshot, module))
        {
            self.cancel_preparation();
        }
        if self.selection_preparation.is_none() {
            self.selection_preparation = Some(Preparation::start_selection(
                snapshot,
                module,
                self.event_bridge.configuration_reader(),
                wake,
                self.probe_interests.clone(),
            )?);
            return Ok(None);
        }
        let request = self.selection_preparation.as_mut().unwrap();
        let ready = request.poll();
        if !request.matches_device_revision(self.event_bridge.device_revision()) {
            self.cancel_preparation();
            self.event_bridge.continue_service();
            return Ok(None);
        }
        ready
    }

    pub(super) fn prepare_configuration(
        &mut self,
        snapshot: &std::sync::Arc<ProcessTreeSnapshot>,
        module: NodeId,
        wake: RuntimeWakeSender,
    ) -> Result<Option<golden_io::RetainedPayload<PreparedConfiguration>>, ConfigurationError> {
        self.preparation_retry_at = None;
        if self.pending_application.is_some() {
            return Ok(None);
        }
        if !self.warnings.quiesce() {
            self.preparation_quiescing = true;
            self.event_bridge.continue_service();
            return Ok(None);
        }
        self.preparation_quiescing = false;
        if self
            .preparation
            .as_ref()
            .is_some_and(|preparation| !preparation.matches(snapshot, module))
        {
            self.preparation = None;
        }
        if self.preparation.is_none() {
            self.preparation = Some(Preparation::start(
                snapshot,
                module,
                self.event_bridge.configuration_reader(),
                wake,
            )?);
            return Ok(None);
        }
        let request = self.preparation.as_mut().unwrap();
        let ready = request.poll();
        if !request.matches_device_revision(self.event_bridge.device_revision()) {
            self.cancel_preparation();
            self.event_bridge.continue_service();
            return Ok(None);
        }
        ready
    }

    pub(super) fn cancel_preparation(&mut self) {
        self.preparation = None;
        self.selection_preparation = None;
        self.preparation_quiescing = false;
    }

    pub(super) fn submit(
        &mut self,
        prepared: golden_io::RetainedPayload<PreparedConfiguration>,
    ) -> Result<ConfigGeneration, String> {
        if self
            .preparation
            .as_ref()
            .is_some_and(|request| !request.matches_device_revision(self.event_bridge.device_revision()))
        {
            return Err("Audio devices changed while preparing the configuration; retrying".into());
        }
        if self.pending_application.is_some() {
            return Err("Sound Card configuration application is still pending".into());
        }
        let generation = self.generation.next();
        prepared
            .submit(&self.control, generation)
            .map_err(|error| error.to_string())?;
        self.generation = generation;
        self.values.reset();
        self.preparation = None;
        self.selection_preparation = None;
        self.preparation_quiescing = false;
        self.warnings.start(prepared.warnings.clone());
        self.pending_application = Some((generation, prepared));
        self.enabled_sent = None;
        Ok(generation)
    }

    pub(super) fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        if !enabled {
            self.cancel_preparation();
            self.preparation_retry_at = None;
        }
        if self.enabled_sent == Some(enabled) && self.enabled_pending.is_none() {
            return Ok(());
        }
        self.enabled_pending = Some(enabled);
        self.retry_enabled()
    }

    fn retry_enabled(&mut self) -> Result<(), String> {
        let Some(enabled) = self.enabled_pending else {
            return Ok(());
        };
        self.control
            .submit(AudioCommand::SetEnabled(enabled))
            .map_err(|error| error.to_string())?;
        self.enabled_sent = Some(enabled);
        self.enabled_pending = None;
        Ok(())
    }

    pub(super) fn admit(
        &self,
        command: AudioCommand,
    ) -> Result<golden_audio::CommandSequence, golden_audio::AudioError> {
        self.control.submit(command)
    }

    pub(super) fn poll(
        &mut self,
        ctx: &mut ProcessCtx,
        module: NodeId,
        drain_events: bool,
    ) -> (
        observation::ObservationView,
        Vec<super::prepared_event::PreparedAudioEvent>,
    ) {
        let events = if drain_events {
            self.event_bridge.drain()
        } else {
            Vec::new()
        };
        let _ = self.retry_enabled();
        for event in &events {
            let applied = match &event.event {
                golden_audio::AudioEvent::ConfigurationApplied { generation } => Some((*generation, true)),
                golden_audio::AudioEvent::ConfigurationRejected { generation, .. } => Some((*generation, false)),
                _ => None,
            };
            if let Some((generation, applied)) = applied {
                if self
                    .pending_application
                    .as_ref()
                    .is_some_and(|(expected, _)| *expected == generation)
                {
                    let (_, prepared) = self.pending_application.take().unwrap();
                    if applied {
                        self.active_configuration = Some(prepared);
                        self.binding_generation = generation;
                        self.values.reset();
                        self.configuration_feedback = Some(ConfigurationFeedback::Applied);
                    } else {
                        self.configuration_feedback = Some(ConfigurationFeedback::Rejected(event.clone()));
                    }
                }
            }
        }
        let observation = self.event_bridge.observation();
        let outgoing_activity = has_output_activity(observation.header.enabled, observation.header.output_rms);
        if self.outgoing_activity != Some(outgoing_activity) {
            self.outgoing_activity = Some(outgoing_activity);
            self.pending_outgoing_activity = Some(outgoing_activity);
        }
        let preparing =
            self.preparation_quiescing || self.preparation.is_some() || self.selection_preparation.is_some();
        self.warnings.service(ctx, preparing);
        let configuring = preparing || self.pending_application.is_some();
        if !configuring {
            if let Some(active) = &self.active_configuration {
                let mut cache = active.value_cache.lock().unwrap_or_else(|p| p.into_inner());
                self.values.service(
                    ctx,
                    &active.bindings,
                    &active.projection_reads,
                    self.binding_generation,
                    &observation,
                    &mut cache,
                );
            }
        }
        self.telemetry.service(ctx, module, &observation);
        if (!configuring && self.values.has_pending()) || self.telemetry.has_pending() || self.warnings.has_pending() {
            self.event_bridge.continue_service();
        }
        (observation, events)
    }

    pub(super) fn command_lifetime(&self) -> Arc<AtomicBool> {
        self.command_lifetime.clone()
    }

    pub(super) fn stop(&mut self) {
        self.command_lifetime.store(false, Ordering::Release);
        self.values.reset();
        // Request ordered shutdown promptly when the control queue has room;
        // the native owner handles full queues and all blocking cleanup.
        let _ = self.control.shutdown();
        self.event_bridge.stop();
    }

    pub(super) fn completion(&self) -> golden_io::RetirementPool {
        self.event_bridge.completion()
    }
    pub(super) fn is_disconnected(&self) -> bool {
        self.event_bridge.is_disconnected() && !self.event_bridge.has_pending()
    }
}

impl std::fmt::Debug for SoundCardRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundCardRuntime")
            .field("driver", &self.driver)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Drop for SoundCardRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(super) use configuration::pitch_tap_id;
