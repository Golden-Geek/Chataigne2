use std::{
    collections::HashMap,
    sync::{Arc, RwLock, mpsc::SyncSender},
    time::{Duration, Instant},
};

#[cfg(all(feature = "analysis", feature = "playback"))]
use crate::DeviceNegotiator;
use crate::{
    AudioBackend, AudioBackendState, AudioBackendStatus, AudioChannelId, AudioConfiguration, AudioDeviceCatalogEntry,
    AudioDeviceDescriptor, AudioDeviceTargetId, AudioEngineConfig, AudioError, AudioErrorCategory, AudioEvent,
    AudioObservationSnapshot, AudioStream, DeviceSupervisor, DeviceSupervisorConfig, DeviceSwitchPhase, GainDb,
    RenderPlan, StreamRequest,
};

use super::engine::{publish_event, update_observation};
#[cfg(all(feature = "analysis", feature = "playback"))]
use super::render_runtime::ManagedRenderRuntime;

const DEVICE_DISCOVERY_INTERVAL: Duration = Duration::from_millis(500);

pub(super) struct DeviceRuntime {
    supervisor: DeviceSupervisor,
    input_stream: Option<Box<dyn AudioStream>>,
    output_stream: Option<Box<dyn AudioStream>>,
    input_handler_active: bool,
    output_handler_active: bool,
    configuration: Option<(AudioConfiguration, RenderPlan)>,
    probe_interests: Vec<AudioDeviceTargetId>,
    probed_devices: HashMap<AudioDeviceTargetId, AudioDeviceDescriptor>,
    probe_failures: HashMap<AudioDeviceTargetId, AudioError>,
    enabled: bool,
    last_discovery: Option<Instant>,
}

impl DeviceRuntime {
    pub(super) fn new(engine_config: &AudioEngineConfig) -> Result<Self, AudioError> {
        Ok(Self {
            supervisor: DeviceSupervisor::new(
                DeviceSupervisorConfig::default(),
                engine_config.sample_rate,
                crate::AudioBufferPolicy::Automatic,
            )?,
            input_stream: None,
            output_stream: None,
            input_handler_active: false,
            output_handler_active: false,
            configuration: None,
            probe_interests: Vec::new(),
            probed_devices: HashMap::new(),
            probe_failures: HashMap::new(),
            enabled: true,
            last_discovery: None,
        })
    }

    pub(super) fn configure(
        &mut self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
        backends: &[Arc<dyn AudioBackend>],
        configuration: &AudioConfiguration,
        plan: RenderPlan,
        #[cfg(all(feature = "analysis", feature = "playback"))] managed_render_runtime: Option<
            &mut ManagedRenderRuntime,
        >,
    ) {
        let input_stream_changed = self.direction_stream_changed(configuration, crate::AudioDirection::Input);
        let output_stream_changed = self.direction_stream_changed(configuration, crate::AudioDirection::Output);
        self.configuration = Some((configuration.clone(), plan));
        self.enabled = configuration.enabled;
        self.apply_direction_configuration(input_stream_changed, output_stream_changed);
        self.refresh(
            event_sender,
            observation,
            backends,
            true,
            #[cfg(all(feature = "analysis", feature = "playback"))]
            managed_render_runtime,
        );
    }

    pub(super) fn set_enabled(
        &mut self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
        backends: &[Arc<dyn AudioBackend>],
        enabled: bool,
        #[cfg(all(feature = "analysis", feature = "playback"))] managed_render_runtime: Option<
            &mut ManagedRenderRuntime,
        >,
    ) {
        let changed = self.enabled != enabled;
        self.enabled = enabled;
        self.apply_direction_configuration(changed, changed);
        self.refresh(
            event_sender,
            observation,
            backends,
            true,
            #[cfg(all(feature = "analysis", feature = "playback"))]
            managed_render_runtime,
        );
    }

    pub(super) fn set_probe_interests(
        &mut self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
        backends: &[Arc<dyn AudioBackend>],
        targets: Vec<AudioDeviceTargetId>,
        #[cfg(all(feature = "analysis", feature = "playback"))] managed_render_runtime: Option<
            &mut ManagedRenderRuntime,
        >,
    ) {
        self.probe_interests = deduplicate_targets(targets);
        self.refresh(
            event_sender,
            observation,
            backends,
            true,
            #[cfg(all(feature = "analysis", feature = "playback"))]
            managed_render_runtime,
        );
    }

    pub(super) fn set_master_gain(&mut self, gain: GainDb) -> Result<(), AudioError> {
        let Some((configuration, plan)) = &mut self.configuration else {
            return Err(AudioError::invalid_configuration(
                "audio runtime has no active configuration",
            ));
        };
        configuration.master_gain = gain;
        plan.master_gain = gain.to_linear();
        Ok(())
    }

    pub(super) fn set_channel_gain(&mut self, channel: AudioChannelId, gain: GainDb) -> Result<(), AudioError> {
        let Some((configuration, plan)) = &mut self.configuration else {
            return Err(AudioError::invalid_configuration(
                "audio runtime has no active configuration",
            ));
        };
        let Some(configuration_channel) = configuration
            .virtual_outputs
            .iter_mut()
            .find(|candidate| candidate.id == channel)
        else {
            return Err(AudioError::invalid_configuration(format!(
                "active audio configuration does not contain output channel {channel}"
            )));
        };
        let Some(plan_index) = plan.virtual_outputs.iter().position(|candidate| *candidate == channel) else {
            return Err(AudioError::new(
                AudioErrorCategory::InternalInvariant,
                format!("active render plan does not contain output channel {channel}"),
            ));
        };
        configuration_channel.gain = gain;
        plan.output_gains[plan_index] = gain.to_linear();
        Ok(())
    }

    fn direction_stream_changed(&self, next: &AudioConfiguration, direction: crate::AudioDirection) -> bool {
        let Some((current, _)) = &self.configuration else {
            return true;
        };
        if self.enabled != next.enabled {
            return true;
        }
        match direction {
            crate::AudioDirection::Input => {
                current.input != next.input || current.physical_inputs != next.physical_inputs
            }
            crate::AudioDirection::Output => {
                current.output != next.output || current.physical_outputs != next.physical_outputs
            }
        }
    }

    fn apply_direction_configuration(&mut self, input_changed: bool, output_changed: bool) {
        let Some((configuration, _)) = &self.configuration else {
            return;
        };
        if input_changed {
            self.supervisor.input.configure(
                self.enabled && configuration.input.enabled,
                configuration.input.device.clone(),
                configuration.input.recovery_policy,
            );
        }
        if output_changed {
            self.supervisor.output.configure(
                self.enabled && configuration.output.enabled,
                configuration.output.device.clone(),
                configuration.output.recovery_policy,
            );
        }
        if !self.enabled || !configuration.input.enabled {
            stop_stream(&mut self.input_stream);
        }
        if !self.enabled || !configuration.output.enabled {
            stop_stream(&mut self.output_stream);
        }
    }

    pub(super) fn refresh(
        &mut self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
        backends: &[Arc<dyn AudioBackend>],
        force: bool,
        #[cfg(all(feature = "analysis", feature = "playback"))] managed_render_runtime: Option<
            &mut ManagedRenderRuntime,
        >,
    ) {
        if self.observe_runtime_statuses(event_sender, observation) {
            self.reconcile_streams(
                backends,
                #[cfg(all(feature = "analysis", feature = "playback"))]
                managed_render_runtime,
            );
            self.publish_state(event_sender, observation);
            return;
        }
        let now = Instant::now();
        if !force
            && self
                .last_discovery
                .is_some_and(|last| now.duration_since(last) < DEVICE_DISCOVERY_INTERVAL)
            && !self.supervisor.tick(elapsed_millis())
        {
            return;
        }
        // Discovery and capability probing can also enter native driver code.
        // Publish the discovering/recovering state before starting that work.
        self.publish_state(event_sender, observation);
        self.last_discovery = Some(now);
        self.evict_unstable_configured_probes();
        let probe_targets = self.desired_probe_targets();
        let (statuses, catalog, discovered) = discover_backend_inventory(
            event_sender,
            observation,
            backends,
            probe_targets.as_slice(),
            &mut self.probed_devices,
            &mut self.probe_failures,
        );
        self.supervisor
            .observe_discovery(elapsed_millis(), statuses, catalog, discovered);
        // Opening a native audio stream can block for several seconds. Publish
        // the prepared state first so hosts can surface connection progress
        // while the backend call is still in flight.
        self.publish_state(event_sender, observation);
        self.reconcile_streams(
            backends,
            #[cfg(all(feature = "analysis", feature = "playback"))]
            managed_render_runtime,
        );
        self.publish_state(event_sender, observation);
    }

    fn desired_probe_targets(&self) -> Vec<AudioDeviceTargetId> {
        let mut targets = self.probe_interests.clone();
        for target in [
            self.supervisor.input.status().selected_target.as_ref(),
            self.supervisor.output.status().selected_target.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if !targets.contains(target) {
                targets.push(target.clone());
            }
        }
        targets
    }

    fn evict_unstable_configured_probes(&mut self) {
        let configured = [
            self.supervisor.input.status().selected_target.as_ref(),
            self.supervisor.output.status().selected_target.as_ref(),
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
        let stable = [
            (&self.supervisor.input, self.supervisor.input.status()),
            (&self.supervisor.output, self.supervisor.output.status()),
        ]
        .into_iter()
        .filter(|(direction, _)| direction.phase() == DeviceSwitchPhase::Stable)
        .filter_map(|(_, status)| status.active_target.clone())
        .collect::<Vec<_>>();
        self.probed_devices
            .retain(|target, _| !configured.contains(target) || stable.contains(target));
    }

    fn observe_runtime_statuses(
        &mut self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
    ) -> bool {
        let now_ms = elapsed_millis();
        let input_failed = observe_runtime_status(
            &mut self.supervisor.input,
            self.input_stream.as_deref(),
            now_ms,
            event_sender,
            observation,
        );
        let output_failed = observe_runtime_status(
            &mut self.supervisor.output,
            self.output_stream.as_deref(),
            now_ms,
            event_sender,
            observation,
        );
        input_failed || output_failed
    }

    fn reconcile_streams(
        &mut self,
        backends: &[Arc<dyn AudioBackend>],
        #[cfg(all(feature = "analysis", feature = "playback"))] mut managed_render_runtime: Option<
            &mut ManagedRenderRuntime,
        >,
    ) {
        let Some((configuration, plan)) = &self.configuration else {
            return;
        };
        let inspector = self.supervisor.inspector_state();
        let sample_rate = inspector.engine_sample_rate;
        reconcile_direction(
            &mut self.supervisor.input,
            &mut self.input_stream,
            &mut self.input_handler_active,
            backends,
            inspector.devices.as_slice(),
            configuration.input.buffer_policy,
            StreamShape {
                sample_rate,
                physical_channels: plan.physical_inputs.as_slice(),
            },
            #[cfg(all(feature = "analysis", feature = "playback"))]
            managed_render_runtime.as_deref_mut(),
        );
        reconcile_direction(
            &mut self.supervisor.output,
            &mut self.output_stream,
            &mut self.output_handler_active,
            backends,
            inspector.devices.as_slice(),
            configuration.output.buffer_policy,
            StreamShape {
                sample_rate,
                physical_channels: plan.physical_outputs.as_slice(),
            },
            #[cfg(all(feature = "analysis", feature = "playback"))]
            managed_render_runtime,
        );
    }

    fn publish_state(
        &self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
    ) {
        let state = self.supervisor.inspector_state();
        let previous = observation
            .read()
            .map(|snapshot| snapshot.device.clone())
            .unwrap_or_default();
        update_observation(observation, |snapshot| {
            snapshot.device = state.clone();
        });
        if previous.input != state.input {
            publish_event(event_sender, observation, AudioEvent::DeviceStatusChanged(state.input));
        }
        if previous.output != state.output {
            publish_event(event_sender, observation, AudioEvent::DeviceStatusChanged(state.output));
        }
        if previous.inventory_revision != state.inventory_revision {
            publish_event(
                event_sender,
                observation,
                AudioEvent::DeviceInventoryChanged {
                    revision: state.inventory_revision,
                },
            );
        }
    }

    pub(super) fn shutdown(
        &mut self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
        #[cfg(all(feature = "analysis", feature = "playback"))] managed_render_runtime: Option<
            &mut ManagedRenderRuntime,
        >,
    ) {
        stop_stream(&mut self.input_stream);
        stop_stream(&mut self.output_stream);
        #[cfg(all(feature = "analysis", feature = "playback"))]
        if let Some(runtime) = managed_render_runtime {
            if self.input_handler_active {
                let _ = runtime.disable_stream_handler(crate::AudioDirection::Input);
            }
            if self.output_handler_active {
                let _ = runtime.disable_stream_handler(crate::AudioDirection::Output);
            }
        }
        self.input_handler_active = false;
        self.output_handler_active = false;
        self.enabled = false;
        self.apply_direction_configuration(true, true);
        self.publish_state(event_sender, observation);
    }
}

fn observe_runtime_status(
    direction: &mut crate::SupervisorDirection,
    stream: Option<&dyn AudioStream>,
    now_ms: u64,
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
) -> bool {
    let Some(stream) = stream else {
        return false;
    };
    let status = stream.status();
    if status.readiness == crate::AudioDeviceReadiness::Ready {
        if let Some(error) = status.error {
            publish_event(
                event_sender,
                observation,
                AudioEvent::Diagnostic(
                    crate::DiagnosticEvent::new(
                        crate::DiagnosticSeverity::Warning,
                        "audio_stream_runtime_warning",
                        error.message,
                    )
                    .with_context("direction", format!("{:?}", status.direction))
                    .with_context("category", format!("{:?}", error.category)),
                ),
            );
        }
        false
    } else {
        direction.report_runtime_status(now_ms, &status)
    }
}

#[derive(Clone, Copy, Debug)]
struct StreamShape<'a> {
    sample_rate: u32,
    physical_channels: &'a [crate::PhysicalChannelKey],
}

// Stream reconciliation needs both the supervisor state and the discovered host
// inventory; keeping those inputs explicit avoids hiding backend I/O in a context object.
#[allow(clippy::too_many_arguments)]
fn reconcile_direction(
    direction: &mut crate::SupervisorDirection,
    stream: &mut Option<Box<dyn AudioStream>>,
    handler_active: &mut bool,
    backends: &[Arc<dyn AudioBackend>],
    devices: &[crate::AudioDeviceDescriptor],
    buffer_policy: crate::AudioBufferPolicy,
    shape: StreamShape<'_>,
    #[cfg(all(feature = "analysis", feature = "playback"))] mut managed_render_runtime: Option<
        &mut ManagedRenderRuntime,
    >,
) {
    #[cfg(all(feature = "analysis", feature = "playback"))]
    let stream_direction = direction.status().direction;
    match direction.phase() {
        DeviceSwitchPhase::Disabled
        | DeviceSwitchPhase::Missing
        | DeviceSwitchPhase::RetryWaiting
        | DeviceSwitchPhase::Failed => {
            stop_stream(stream);
            #[cfg(all(feature = "analysis", feature = "playback"))]
            disable_managed_stream_handler(&mut managed_render_runtime, handler_active, stream_direction);
            #[cfg(not(all(feature = "analysis", feature = "playback")))]
            {
                *handler_active = false;
            }
        }
        DeviceSwitchPhase::Preparing => {
            let Some(target) = direction.status().selected_target.clone() else {
                return;
            };
            let Some(backend) = backends.iter().find(|backend| backend.id() == *target.backend()) else {
                direction.report_open_error(
                    elapsed_millis(),
                    AudioError::new(
                        AudioErrorCategory::BackendUnavailable,
                        format!("selected audio backend {} is not registered", target.backend()),
                    ),
                );
                return;
            };
            if let Err(error) = validate_stream_channel_inventory(
                devices,
                &target,
                direction.status().direction,
                shape.physical_channels,
            ) {
                direction.report_open_error(elapsed_millis(), error);
                return;
            }
            let channels = u16::try_from(shape.physical_channels.len())
                .expect("validated physical channel limits fit the stream channel width");
            let request = StreamRequest {
                direction: direction.status().direction,
                target,
                engine_sample_rate: crate::SampleRate::new(shape.sample_rate)
                    .expect("device supervisor sample rate is validated"),
                channels,
                buffer_policy,
            };
            #[cfg(all(feature = "analysis", feature = "playback"))]
            let uses_managed_handler = backend.supports_stream_handlers() && managed_render_runtime.is_some();
            #[cfg(all(feature = "analysis", feature = "playback"))]
            let prepared = if uses_managed_handler {
                let callback_buffer_frames = match negotiated_buffer_frames(devices, &request) {
                    Ok(frames) => frames,
                    Err(error) => {
                        direction.report_open_error(elapsed_millis(), error);
                        return;
                    }
                };
                let runtime = managed_render_runtime
                    .as_deref_mut()
                    .expect("managed handler use requires a managed runtime");
                match runtime.prepare_stream_handler(stream_direction, channels, callback_buffer_frames) {
                    Ok(handler) => {
                        *handler_active = true;
                        backend.open_stream_with_handler(&request, handler)
                    }
                    Err(error) => Err(error),
                }
            } else {
                backend.open_stream(&request)
            };
            #[cfg(not(all(feature = "analysis", feature = "playback")))]
            let prepared = backend.open_stream(&request);
            match prepared {
                Ok(mut prepared) => {
                    let Some(format) = prepared.status().format else {
                        #[cfg(all(feature = "analysis", feature = "playback"))]
                        disable_managed_stream_handler(&mut managed_render_runtime, handler_active, stream_direction);
                        #[cfg(not(all(feature = "analysis", feature = "playback")))]
                        {
                            *handler_active = false;
                        }
                        direction.report_open_error(
                            elapsed_millis(),
                            AudioError::new(
                                AudioErrorCategory::InternalInvariant,
                                "audio backend returned a stream without a negotiated format",
                            ),
                        );
                        return;
                    };
                    let switched = direction
                        .mark_primed(format)
                        .and_then(|()| direction.begin_switch())
                        .and_then(|()| prepared.start())
                        .and_then(|()| direction.commit_switch());
                    match switched {
                        Ok(()) => {
                            #[cfg(all(feature = "analysis", feature = "playback"))]
                            if !uses_managed_handler {
                                disable_managed_stream_handler(
                                    &mut managed_render_runtime,
                                    handler_active,
                                    stream_direction,
                                );
                            }
                            stop_stream(stream);
                            *stream = Some(prepared);
                            #[cfg(not(all(feature = "analysis", feature = "playback")))]
                            {
                                *handler_active = false;
                            }
                        }
                        Err(error) => {
                            #[cfg(all(feature = "analysis", feature = "playback"))]
                            disable_managed_stream_handler(
                                &mut managed_render_runtime,
                                handler_active,
                                stream_direction,
                            );
                            #[cfg(not(all(feature = "analysis", feature = "playback")))]
                            {
                                *handler_active = false;
                            }
                            direction.report_open_error(elapsed_millis(), error);
                        }
                    }
                }
                Err(error) => {
                    #[cfg(all(feature = "analysis", feature = "playback"))]
                    disable_managed_stream_handler(&mut managed_render_runtime, handler_active, stream_direction);
                    #[cfg(not(all(feature = "analysis", feature = "playback")))]
                    {
                        *handler_active = false;
                    }
                    direction.report_open_error(elapsed_millis(), error);
                }
            }
        }
        DeviceSwitchPhase::Discovering
        | DeviceSwitchPhase::Primed
        | DeviceSwitchPhase::Switching
        | DeviceSwitchPhase::Stable => {}
    }
}

#[cfg(all(feature = "analysis", feature = "playback"))]
fn disable_managed_stream_handler(
    runtime: &mut Option<&mut ManagedRenderRuntime>,
    handler_active: &mut bool,
    direction: crate::AudioDirection,
) {
    if !*handler_active {
        return;
    }
    let Some(runtime) = runtime.as_deref_mut() else {
        *handler_active = false;
        return;
    };
    if runtime.disable_stream_handler(direction).is_ok() {
        *handler_active = false;
    }
}

#[cfg(all(feature = "analysis", feature = "playback"))]
fn negotiated_buffer_frames(
    devices: &[crate::AudioDeviceDescriptor],
    request: &StreamRequest,
) -> Result<u32, AudioError> {
    let device = selected_device(devices, &request.target, request.direction).ok_or_else(|| {
        AudioError::new(
            AudioErrorCategory::DeviceMissing,
            "selected audio device disappeared before stream negotiation",
        )
    })?;
    DeviceNegotiator
        .negotiate(device, request.negotiation_request())
        .map(|format| format.buffer_frames)
}

fn selected_device<'a>(
    devices: &'a [crate::AudioDeviceDescriptor],
    target: &crate::AudioDeviceTargetId,
    direction: crate::AudioDirection,
) -> Option<&'a crate::AudioDeviceDescriptor> {
    devices.iter().find(|device| {
        device.supports(direction)
            && match target {
                crate::AudioDeviceTargetId::Device { .. } => device.target == *target,
                crate::AudioDeviceTargetId::SystemDefault { backend } => {
                    device.target.backend() == backend
                        && match direction {
                            crate::AudioDirection::Input => device.is_system_default_input,
                            crate::AudioDirection::Output => device.is_system_default_output,
                        }
                }
            }
    })
}

pub(super) fn validate_stream_channel_inventory(
    devices: &[crate::AudioDeviceDescriptor],
    target: &crate::AudioDeviceTargetId,
    direction: crate::AudioDirection,
    configured: &[crate::PhysicalChannelKey],
) -> Result<(), AudioError> {
    let device = selected_device(devices, target, direction).ok_or_else(|| {
        AudioError::new(
            AudioErrorCategory::DeviceMissing,
            "selected audio device disappeared before stream preparation",
        )
    })?;
    let discovered = match direction {
        crate::AudioDirection::Input => device.input_channels.as_slice(),
        crate::AudioDirection::Output => device.output_channels.as_slice(),
    };
    let matches = discovered.len() == configured.len()
        && discovered
            .iter()
            .zip(configured)
            .all(|(descriptor, configured)| descriptor.key == *configured);
    if !matches {
        return Err(AudioError::new(
            AudioErrorCategory::StreamNegotiationFailed,
            format!(
                "configured physical {} channel inventory no longer matches the selected device",
                match direction {
                    crate::AudioDirection::Input => "input",
                    crate::AudioDirection::Output => "output",
                }
            ),
        ));
    }
    Ok(())
}

fn stop_stream(stream: &mut Option<Box<dyn AudioStream>>) {
    if let Some(mut active) = stream.take() {
        let _ = active.stop();
    }
}

fn elapsed_millis() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(super) fn discover_backend_inventory(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    backends: &[Arc<dyn AudioBackend>],
    probe_targets: &[AudioDeviceTargetId],
    probed_devices: &mut HashMap<AudioDeviceTargetId, AudioDeviceDescriptor>,
    probe_failures: &mut HashMap<AudioDeviceTargetId, AudioError>,
) -> (
    Vec<AudioBackendStatus>,
    Vec<AudioDeviceCatalogEntry>,
    Vec<AudioDeviceDescriptor>,
) {
    let previous_state = observation
        .read()
        .map(|snapshot| snapshot.device.clone())
        .unwrap_or_default();
    let mut statuses = Vec::with_capacity(backends.len());
    let mut catalog = Vec::new();
    let mut devices = Vec::new();
    for backend in backends {
        let descriptor = backend.descriptor();
        let mut status = AudioBackendStatus {
            backend: descriptor.id,
            label: descriptor.label,
            state: descriptor.state,
            detail: descriptor.detail,
        };
        if status.state == AudioBackendState::Available {
            match backend.device_inventory() {
                Ok(inventory) => {
                    extend_unique_catalog(&mut catalog, inventory.catalog);
                    extend_unique_devices(&mut devices, inventory.devices);
                }
                Err(error) => {
                    status.state = AudioBackendState::Failed;
                    status.detail = Some(error.to_string());
                    preserve_backend_catalog(&mut catalog, &previous_state.device_catalog, &status.backend);
                }
            }
        } else {
            preserve_backend_catalog(&mut catalog, &previous_state.device_catalog, &status.backend);
        }
        if previous_state
            .backends
            .iter()
            .find(|candidate| candidate.backend == status.backend)
            != Some(&status)
        {
            publish_event(
                event_sender,
                observation,
                AudioEvent::BackendStatusChanged(status.clone()),
            );
        }
        statuses.push(status);
    }

    probed_devices.retain(|target, _| probe_targets.contains(target) && catalog_contains(&catalog, target));
    probe_failures.retain(|target, _| probe_targets.contains(target) && catalog_contains(&catalog, target));
    for target in probe_targets {
        if devices.iter().any(|device| device.target == *target) {
            probe_failures.remove(target);
            continue;
        }
        if !catalog_contains(&catalog, target) {
            probed_devices.remove(target);
            probe_failures.remove(target);
            continue;
        }
        if statuses
            .iter()
            .find(|status| status.backend == *target.backend())
            .is_none_or(|status| status.state != AudioBackendState::Available)
        {
            continue;
        }
        if let Some(device) = probed_devices.get(target) {
            devices.push(device.clone());
            continue;
        }
        let Some(backend) = backends.iter().find(|backend| backend.id() == *target.backend()) else {
            continue;
        };
        match backend.probe_device(target) {
            Ok(Some(device)) if device.target == *target => {
                probe_failures.remove(target);
                probed_devices.insert(target.clone(), device.clone());
                devices.push(device);
            }
            Ok(Some(device)) => {
                publish_probe_failure(
                    event_sender,
                    observation,
                    probe_failures,
                    target,
                    AudioError::new(
                        AudioErrorCategory::InternalInvariant,
                        format!(
                            "backend returned descriptor for {:?} while probing {:?}",
                            device.target, target
                        ),
                    ),
                );
            }
            Ok(None) => {
                probed_devices.remove(target);
                probe_failures.remove(target);
            }
            Err(error) => {
                publish_probe_failure(event_sender, observation, probe_failures, target, error);
            }
        }
    }
    (statuses, catalog, devices)
}

fn publish_probe_failure(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    failures: &mut HashMap<AudioDeviceTargetId, AudioError>,
    target: &AudioDeviceTargetId,
    error: AudioError,
) {
    if failures.get(target) == Some(&error) {
        return;
    }
    failures.insert(target.clone(), error.clone());
    publish_event(
        event_sender,
        observation,
        AudioEvent::Diagnostic(
            crate::DiagnosticEvent::new(
                crate::DiagnosticSeverity::Warning,
                "audio_device_probe_failed",
                "failed to probe the selected audio device",
            )
            .with_context("target", format!("{target:?}"))
            .with_context("error", error.to_string()),
        ),
    );
}

fn catalog_contains(catalog: &[AudioDeviceCatalogEntry], target: &AudioDeviceTargetId) -> bool {
    catalog.iter().any(|entry| entry.target == *target)
}

fn extend_unique_catalog(destination: &mut Vec<AudioDeviceCatalogEntry>, source: Vec<AudioDeviceCatalogEntry>) {
    for entry in source {
        if !catalog_contains(destination, &entry.target) {
            destination.push(entry);
        }
    }
}

fn preserve_backend_catalog(
    destination: &mut Vec<AudioDeviceCatalogEntry>,
    previous: &[AudioDeviceCatalogEntry],
    backend: &crate::BackendId,
) {
    extend_unique_catalog(
        destination,
        previous
            .iter()
            .filter(|entry| entry.target.backend() == backend)
            .cloned()
            .collect(),
    );
}

fn extend_unique_devices(destination: &mut Vec<AudioDeviceDescriptor>, source: Vec<AudioDeviceDescriptor>) {
    for device in source {
        if !destination.iter().any(|candidate| candidate.target == device.target) {
            destination.push(device);
        }
    }
}

fn deduplicate_targets(targets: Vec<AudioDeviceTargetId>) -> Vec<AudioDeviceTargetId> {
    let mut unique = Vec::with_capacity(targets.len());
    for target in targets {
        if !unique.contains(&target) {
            unique.push(target);
        }
    }
    unique
}
