use std::sync::{Arc, RwLock};

use crate::{
    AudioBackend, AudioBackendState, AudioDeviceDescriptor, AudioDeviceReadiness, AudioDeviceTargetId, AudioDirection,
    AudioError, AudioErrorCategory, AudioPermissionState, AudioRecoveryPolicy, AudioStream, AudioStreamStatus,
    BackendDescriptor, BackendId, DeviceNegotiator, StreamRequest, SupportedStreamConfiguration,
};

#[derive(Clone, Debug)]
pub struct MockBackend {
    id: BackendId,
    shared: Arc<RwLock<MockBackendState>>,
}

#[derive(Clone, Debug)]
pub struct MockBackendControl {
    shared: Arc<RwLock<MockBackendState>>,
}

#[derive(Clone, Debug)]
struct MockBackendState {
    descriptor: BackendDescriptor,
    devices: Vec<AudioDeviceDescriptor>,
    open_error: Option<AudioError>,
    event_revision: u64,
    events: Vec<MockBackendEvent>,
    flapping: bool,
    flapping_phase: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockBackendEvent {
    pub revision: u64,
    pub kind: MockBackendEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MockBackendEventKind {
    Connected(AudioDeviceTargetId),
    Disconnected(AudioDeviceTargetId),
    DefaultChanged {
        direction: AudioDirection,
        target: AudioDeviceTargetId,
    },
    FormatChanged(AudioDeviceTargetId),
    ServerRestarted,
    FlappingChanged(bool),
}

impl MockBackend {
    #[must_use]
    pub fn new(id: BackendId, label: impl Into<String>) -> (Self, MockBackendControl) {
        let shared = Arc::new(RwLock::new(MockBackendState {
            descriptor: BackendDescriptor {
                id: id.clone(),
                label: label.into(),
                state: AudioBackendState::Available,
                detail: None,
            },
            devices: Vec::new(),
            open_error: None,
            event_revision: 0,
            events: Vec::new(),
            flapping: false,
            flapping_phase: false,
        }));
        (
            Self {
                id,
                shared: Arc::clone(&shared),
            },
            MockBackendControl { shared },
        )
    }

    fn read_state(&self) -> Result<std::sync::RwLockReadGuard<'_, MockBackendState>, AudioError> {
        self.shared.read().map_err(|_| {
            AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "mock backend state lock was poisoned",
            )
        })
    }
}

impl MockBackendControl {
    pub fn set_devices(&self, devices: Vec<AudioDeviceDescriptor>) -> Result<(), AudioError> {
        self.write_state()?.devices = devices;
        Ok(())
    }

    pub fn connect_device(&self, device: AudioDeviceDescriptor) -> Result<(), AudioError> {
        let mut state = self.write_state()?;
        let target = device.target.clone();
        state.devices.retain(|current| current.target != target);
        state.devices.push(device);
        push_event(&mut state, MockBackendEventKind::Connected(target));
        Ok(())
    }

    pub fn disconnect_device(&self, target: &AudioDeviceTargetId) -> Result<(), AudioError> {
        let mut state = self.write_state()?;
        state.devices.retain(|device| &device.target != target);
        push_event(&mut state, MockBackendEventKind::Disconnected(target.clone()));
        Ok(())
    }

    pub fn set_default(&self, direction: AudioDirection, target: &AudioDeviceTargetId) -> Result<(), AudioError> {
        let mut state = self.write_state()?;
        if !state.devices.iter().any(|device| &device.target == target) {
            return Err(AudioError::new(
                AudioErrorCategory::DeviceMissing,
                "mock default target is not connected",
            ));
        }
        for device in &mut state.devices {
            let selected = &device.target == target;
            match direction {
                AudioDirection::Input => device.is_system_default_input = selected,
                AudioDirection::Output => device.is_system_default_output = selected,
            }
        }
        push_event(
            &mut state,
            MockBackendEventKind::DefaultChanged {
                direction,
                target: target.clone(),
            },
        );
        Ok(())
    }

    pub fn change_supported_configurations(
        &self,
        target: &AudioDeviceTargetId,
        configurations: Vec<SupportedStreamConfiguration>,
    ) -> Result<(), AudioError> {
        let mut state = self.write_state()?;
        let Some(device) = state.devices.iter_mut().find(|device| &device.target == target) else {
            return Err(AudioError::new(
                AudioErrorCategory::DeviceMissing,
                "mock format-change target is not connected",
            ));
        };
        device.supported_configurations = configurations;
        push_event(&mut state, MockBackendEventKind::FormatChanged(target.clone()));
        Ok(())
    }

    pub fn restart_server(&self) -> Result<(), AudioError> {
        let mut state = self.write_state()?;
        state.descriptor.state = AudioBackendState::Available;
        state.descriptor.detail = None;
        push_event(&mut state, MockBackendEventKind::ServerRestarted);
        Ok(())
    }

    pub fn set_flapping(&self, enabled: bool) -> Result<(), AudioError> {
        let mut state = self.write_state()?;
        state.flapping = enabled;
        state.flapping_phase = false;
        push_event(&mut state, MockBackendEventKind::FlappingChanged(enabled));
        Ok(())
    }

    pub fn drain_events(&self) -> Result<Vec<MockBackendEvent>, AudioError> {
        Ok(std::mem::take(&mut self.write_state()?.events))
    }

    pub fn set_state(&self, state: AudioBackendState, detail: Option<String>) -> Result<(), AudioError> {
        let mut shared = self.write_state()?;
        shared.descriptor.state = state;
        shared.descriptor.detail = detail;
        Ok(())
    }

    pub fn fail_open(&self, error: Option<AudioError>) -> Result<(), AudioError> {
        self.write_state()?.open_error = error;
        Ok(())
    }

    fn write_state(&self) -> Result<std::sync::RwLockWriteGuard<'_, MockBackendState>, AudioError> {
        self.shared.write().map_err(|_| {
            AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "mock backend state lock was poisoned",
            )
        })
    }
}

impl AudioBackend for MockBackend {
    fn id(&self) -> BackendId {
        self.id.clone()
    }

    fn descriptor(&self) -> BackendDescriptor {
        self.read_state()
            .map(|state| state.descriptor.clone())
            .unwrap_or_else(|error| BackendDescriptor {
                id: BackendId::from_static("mock-poisoned"),
                label: "Mock Backend".to_owned(),
                state: AudioBackendState::Failed,
                detail: Some(error.to_string()),
            })
    }

    fn device_inventory(&self) -> Result<crate::AudioDeviceInventory, AudioError> {
        let mut state = self.shared.write().map_err(|_| {
            AudioError::new(
                AudioErrorCategory::InternalInvariant,
                "mock backend state lock was poisoned",
            )
        })?;
        if state.descriptor.state != AudioBackendState::Available {
            return Err(AudioError::new(
                AudioErrorCategory::BackendUnavailable,
                state
                    .descriptor
                    .detail
                    .clone()
                    .unwrap_or_else(|| "mock backend is unavailable".to_owned()),
            ));
        }
        if state.flapping {
            state.flapping_phase = !state.flapping_phase;
            if state.flapping_phase {
                return Err(AudioError::new(
                    AudioErrorCategory::BackendUnavailable,
                    "mock backend is flapping",
                ));
            }
        }
        Ok(crate::AudioDeviceInventory::from_devices(state.devices.clone()))
    }

    fn open_stream(&self, request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        request.validate()?;
        let state = self.read_state()?;
        if let Some(error) = &state.open_error {
            return Err(error.clone());
        }
        let device = state
            .devices
            .iter()
            .find(|device| match &request.target {
                AudioDeviceTargetId::Device { .. } => {
                    device.target == request.target && device.supports(request.direction)
                }
                AudioDeviceTargetId::SystemDefault { backend } => {
                    device.target.backend() == backend
                        && device.supports(request.direction)
                        && match request.direction {
                            AudioDirection::Input => device.is_system_default_input,
                            AudioDirection::Output => device.is_system_default_output,
                        }
                }
            })
            .ok_or_else(|| {
                AudioError::new(
                    AudioErrorCategory::DeviceMissing,
                    "requested mock audio device is missing",
                )
            })?;
        let format = DeviceNegotiator.negotiate(device, request.negotiation_request())?;
        Ok(Box::new(MockStream {
            status: AudioStreamStatus {
                direction: request.direction,
                enabled: true,
                selected_target: Some(request.target.clone()),
                selected_label: Some(device.label.clone()),
                profile_key: Some(device.profile_key.clone()),
                active_target: Some(device.target.clone()),
                readiness: AudioDeviceReadiness::Ready,
                permission: AudioPermissionState::Granted,
                recovery_policy: AudioRecoveryPolicy::WaitForSelected,
                retry_attempt: 0,
                next_retry_ms: None,
                format: Some(format),
                error: None,
            },
        }))
    }
}

fn push_event(state: &mut MockBackendState, kind: MockBackendEventKind) {
    state.event_revision = state.event_revision.saturating_add(1);
    let revision = state.event_revision;
    state.events.push(MockBackendEvent { revision, kind });
}

#[derive(Debug)]
struct MockStream {
    status: AudioStreamStatus,
}

impl AudioStream for MockStream {
    fn status(&self) -> AudioStreamStatus {
        self.status.clone()
    }

    fn start(&mut self) -> Result<(), AudioError> {
        self.status.enabled = true;
        self.status.readiness = AudioDeviceReadiness::Ready;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.status.enabled = false;
        self.status.readiness = AudioDeviceReadiness::Disabled;
        self.status.active_target = None;
        Ok(())
    }
}
