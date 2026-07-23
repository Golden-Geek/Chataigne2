use std::sync::{Arc, RwLock};

use crate::{
    AudioBackend, AudioBackendState, AudioDeviceDescriptor, AudioDeviceReadiness, AudioError, AudioErrorCategory,
    AudioPermissionState, AudioSampleFormat, AudioStream, AudioStreamStatus, BackendDescriptor, BackendId,
    NegotiatedStreamFormat, StreamRequest,
};

#[derive(Clone, Debug)]
pub struct MockBackend {
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
}

impl MockBackend {
    #[must_use]
    pub fn new(id: BackendId, label: impl Into<String>) -> (Self, MockBackendControl) {
        let shared = Arc::new(RwLock::new(MockBackendState {
            descriptor: BackendDescriptor {
                id,
                label: label.into(),
                state: AudioBackendState::Available,
                detail: None,
            },
            devices: Vec::new(),
            open_error: None,
        }));
        (
            Self {
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

    fn discover(&self) -> Result<Vec<AudioDeviceDescriptor>, AudioError> {
        let state = self.read_state()?;
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
        Ok(state.devices.clone())
    }

    fn open_stream(&self, request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        request.validate()?;
        let state = self.read_state()?;
        if let Some(error) = &state.open_error {
            return Err(error.clone());
        }
        if !state
            .devices
            .iter()
            .any(|device| device.target == request.target && device.supports(request.direction))
        {
            return Err(AudioError::new(
                AudioErrorCategory::DeviceMissing,
                "requested mock audio device is missing",
            ));
        }
        Ok(Box::new(MockStream {
            status: AudioStreamStatus {
                direction: request.direction,
                enabled: true,
                selected_target: Some(request.target.clone()),
                active_target: Some(request.target.clone()),
                readiness: AudioDeviceReadiness::Ready,
                permission: AudioPermissionState::Granted,
                format: Some(NegotiatedStreamFormat {
                    sample_rate: request.engine_sample_rate.get(),
                    channels: request.channels,
                    sample_format: AudioSampleFormat::F32,
                    buffer_frames: match request.buffer_policy {
                        crate::AudioBufferPolicy::Automatic => 128,
                        crate::AudioBufferPolicy::Fixed(frames) => frames,
                    },
                    estimated_latency_ms: 2.0,
                }),
                error: None,
            },
        }))
    }
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
