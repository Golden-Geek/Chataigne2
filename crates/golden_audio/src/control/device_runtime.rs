use std::{
    sync::{Arc, RwLock, mpsc::SyncSender},
    time::{Duration, Instant},
};

use crate::{
    AudioBackend, AudioBackendState, AudioBackendStatus, AudioConfiguration, AudioEngineConfig, AudioError,
    AudioErrorCategory, AudioEvent, AudioObservationSnapshot, AudioStream, DeviceSupervisor, DeviceSupervisorConfig,
    DeviceSwitchPhase, RenderPlan, StreamRequest,
};

use super::engine::{publish_event, update_observation};

const DEVICE_DISCOVERY_INTERVAL: Duration = Duration::from_millis(500);

pub(super) struct DeviceRuntime {
    supervisor: DeviceSupervisor,
    input_stream: Option<Box<dyn AudioStream>>,
    output_stream: Option<Box<dyn AudioStream>>,
    configuration: Option<(AudioConfiguration, RenderPlan)>,
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
            configuration: None,
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
    ) {
        self.configuration = Some((configuration.clone(), plan));
        self.enabled = configuration.enabled;
        self.apply_direction_configuration();
        self.refresh(event_sender, observation, backends, true);
    }

    pub(super) fn set_enabled(
        &mut self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
        backends: &[Arc<dyn AudioBackend>],
        enabled: bool,
    ) {
        self.enabled = enabled;
        self.apply_direction_configuration();
        self.refresh(event_sender, observation, backends, true);
    }

    fn apply_direction_configuration(&mut self) {
        let Some((configuration, _)) = &self.configuration else {
            return;
        };
        self.supervisor.input.configure(
            self.enabled && configuration.input.enabled,
            configuration.input.device.clone(),
            configuration.input.recovery_policy,
        );
        self.supervisor.output.configure(
            self.enabled && configuration.output.enabled,
            configuration.output.device.clone(),
            configuration.output.recovery_policy,
        );
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
    ) {
        let now = Instant::now();
        if !force
            && self
                .last_discovery
                .is_some_and(|last| now.duration_since(last) < DEVICE_DISCOVERY_INTERVAL)
            && !self.supervisor.tick(elapsed_millis())
        {
            return;
        }
        self.last_discovery = Some(now);
        let (statuses, discovered) = discover_backend_inventory(event_sender, observation, backends);
        self.supervisor
            .observe_discovery(elapsed_millis(), statuses, discovered);
        self.reconcile_streams(backends);
        self.publish_state(event_sender, observation);
    }

    fn reconcile_streams(&mut self, backends: &[Arc<dyn AudioBackend>]) {
        let Some((configuration, plan)) = &self.configuration else {
            return;
        };
        let inspector = self.supervisor.inspector_state();
        let sample_rate = inspector.engine_sample_rate;
        let input_channels = stream_channel_count(
            &inspector.input,
            inspector.devices.as_slice(),
            crate::AudioDirection::Input,
            plan.physical_inputs.len(),
        );
        let output_channels = stream_channel_count(
            &inspector.output,
            inspector.devices.as_slice(),
            crate::AudioDirection::Output,
            plan.physical_outputs.len(),
        );
        reconcile_direction(
            &mut self.supervisor.input,
            &mut self.input_stream,
            backends,
            configuration.input.buffer_policy,
            sample_rate,
            input_channels,
        );
        reconcile_direction(
            &mut self.supervisor.output,
            &mut self.output_stream,
            backends,
            configuration.output.buffer_policy,
            sample_rate,
            output_channels,
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
    }

    pub(super) fn shutdown(
        &mut self,
        event_sender: &SyncSender<AudioEvent>,
        observation: &Arc<RwLock<AudioObservationSnapshot>>,
    ) {
        stop_stream(&mut self.input_stream);
        stop_stream(&mut self.output_stream);
        self.enabled = false;
        self.apply_direction_configuration();
        self.publish_state(event_sender, observation);
    }
}

fn reconcile_direction(
    direction: &mut crate::SupervisorDirection,
    stream: &mut Option<Box<dyn AudioStream>>,
    backends: &[Arc<dyn AudioBackend>],
    buffer_policy: crate::AudioBufferPolicy,
    sample_rate: u32,
    stream_channels: usize,
) {
    match direction.phase() {
        DeviceSwitchPhase::Disabled
        | DeviceSwitchPhase::Missing
        | DeviceSwitchPhase::RetryWaiting
        | DeviceSwitchPhase::Failed => {
            stop_stream(stream);
        }
        DeviceSwitchPhase::Preparing => {
            let Some(target) = direction.status().selected_target.clone() else {
                return;
            };
            let Some(backend) = backends
                .iter()
                .find(|backend| backend.descriptor().id == *target.backend())
            else {
                direction.report_open_error(
                    elapsed_millis(),
                    AudioError::new(
                        AudioErrorCategory::BackendUnavailable,
                        format!("selected audio backend {} is not registered", target.backend()),
                    ),
                );
                return;
            };
            let channels = u16::try_from(stream_channels.max(1)).unwrap_or(u16::MAX);
            let request = StreamRequest {
                direction: direction.status().direction,
                target,
                engine_sample_rate: crate::SampleRate::new(sample_rate)
                    .expect("device supervisor sample rate is validated"),
                channels,
                buffer_policy,
            };
            match backend.open_stream(&request) {
                Ok(mut prepared) => {
                    let Some(format) = prepared.status().format else {
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
                            stop_stream(stream);
                            *stream = Some(prepared);
                        }
                        Err(error) => direction.report_open_error(elapsed_millis(), error),
                    }
                }
                Err(error) => direction.report_open_error(elapsed_millis(), error),
            }
        }
        DeviceSwitchPhase::Discovering
        | DeviceSwitchPhase::Primed
        | DeviceSwitchPhase::Switching
        | DeviceSwitchPhase::Stable => {}
    }
}

fn stream_channel_count(
    status: &crate::AudioStreamStatus,
    devices: &[crate::AudioDeviceDescriptor],
    direction: crate::AudioDirection,
    required_channels: usize,
) -> usize {
    let selected = status.selected_target.as_ref();
    let selected_device = devices.iter().find(|device| {
        device.supports(direction)
            && selected.is_some_and(|target| match target {
                crate::AudioDeviceTargetId::Device { .. } => device.target == *target,
                crate::AudioDeviceTargetId::SystemDefault { backend } => {
                    device.target.backend() == backend
                        && match direction {
                            crate::AudioDirection::Input => device.is_system_default_input,
                            crate::AudioDirection::Output => device.is_system_default_output,
                        }
                }
            })
    });
    let discovered_channels = selected_device.map_or(0, |device| match direction {
        crate::AudioDirection::Input => device.input_channels.len(),
        crate::AudioDirection::Output => device.output_channels.len(),
    });
    discovered_channels.max(required_channels).max(1)
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

fn discover_backend_inventory(
    event_sender: &SyncSender<AudioEvent>,
    observation: &Arc<RwLock<AudioObservationSnapshot>>,
    backends: &[Arc<dyn AudioBackend>],
) -> (Vec<AudioBackendStatus>, Vec<crate::AudioDeviceDescriptor>) {
    let previous = observation
        .read()
        .map(|snapshot| snapshot.device.backends.clone())
        .unwrap_or_default();
    let mut statuses = Vec::with_capacity(backends.len());
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
            match backend.discover() {
                Ok(discovered) => devices.extend(discovered),
                Err(error) => {
                    status.state = AudioBackendState::Failed;
                    status.detail = Some(error.to_string());
                }
            }
        }
        if previous.iter().find(|candidate| candidate.backend == status.backend) != Some(&status) {
            publish_event(
                event_sender,
                observation,
                AudioEvent::BackendStatusChanged(status.clone()),
            );
        }
        statuses.push(status);
    }
    update_observation(observation, |snapshot| {
        snapshot.device.backends = statuses.clone();
        snapshot.device.devices = devices.clone();
        snapshot.device.discovery_in_progress = false;
    });
    (statuses, devices)
}
