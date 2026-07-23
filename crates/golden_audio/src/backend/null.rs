use crate::{
    AudioBackend, AudioBackendState, AudioDeviceDescriptor, AudioDeviceId, AudioDeviceReadiness, AudioDeviceTargetId,
    AudioError, AudioPermissionState, AudioSampleFormat, AudioStream, AudioStreamStatus, BackendDescriptor, BackendId,
    NegotiatedStreamFormat, PhysicalChannelDescriptor, PhysicalChannelKey, StreamRequest,
};

#[derive(Clone, Debug, Default)]
pub struct NullBackend;

impl NullBackend {
    #[must_use]
    pub fn backend_id() -> BackendId {
        BackendId::from_static("null")
    }

    #[must_use]
    pub fn target() -> AudioDeviceTargetId {
        AudioDeviceTargetId::Device {
            backend: Self::backend_id(),
            device: AudioDeviceId::from_static("null-duplex"),
        }
    }
}

impl AudioBackend for NullBackend {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            id: Self::backend_id(),
            label: "Null / Offline".to_owned(),
            state: AudioBackendState::Available,
            detail: Some("Deterministic silent device with a software clock.".to_owned()),
        }
    }

    fn discover(&self) -> Result<Vec<AudioDeviceDescriptor>, AudioError> {
        let input_channels = physical_channels("input", 2);
        let output_channels = physical_channels("output", 2);
        Ok(vec![AudioDeviceDescriptor {
            target: Self::target(),
            label: "Null Duplex".to_owned(),
            fingerprint: Some("golden-audio:null-duplex:v1".to_owned()),
            input_channels,
            output_channels,
            is_system_default_input: true,
            is_system_default_output: true,
        }])
    }

    fn open_stream(&self, request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError> {
        request.validate()?;
        if request.target.backend() != &Self::backend_id() {
            return Err(AudioError::new(
                crate::AudioErrorCategory::DeviceMissing,
                "null backend cannot open a target owned by another backend",
            ));
        }
        Ok(Box::new(NullStream::new(request)))
    }
}

fn physical_channels(prefix: &str, count: u16) -> Vec<PhysicalChannelDescriptor> {
    (0..count)
        .map(|index| PhysicalChannelDescriptor {
            key: PhysicalChannelKey::new(format!("{prefix}:{index}"))
                .expect("generated null physical channel key is valid"),
            label: format!("{} {}", capitalize(prefix), index + 1),
            position: None,
        })
        .collect()
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

#[derive(Debug)]
struct NullStream {
    status: AudioStreamStatus,
}

impl NullStream {
    fn new(request: &StreamRequest) -> Self {
        Self {
            status: AudioStreamStatus {
                direction: request.direction,
                enabled: true,
                selected_target: Some(request.target.clone()),
                active_target: Some(request.target.clone()),
                readiness: AudioDeviceReadiness::Ready,
                permission: AudioPermissionState::NotRequired,
                format: Some(NegotiatedStreamFormat {
                    sample_rate: request.engine_sample_rate.get(),
                    channels: request.channels,
                    sample_format: AudioSampleFormat::F32,
                    buffer_frames: match request.buffer_policy {
                        crate::AudioBufferPolicy::Automatic => 128,
                        crate::AudioBufferPolicy::Fixed(frames) => frames,
                    },
                    estimated_latency_ms: 0.0,
                }),
                error: None,
            },
        }
    }
}

impl AudioStream for NullStream {
    fn status(&self) -> AudioStreamStatus {
        self.status.clone()
    }

    fn start(&mut self) -> Result<(), AudioError> {
        self.status.readiness = AudioDeviceReadiness::Ready;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.status.readiness = AudioDeviceReadiness::Disabled;
        self.status.enabled = false;
        self.status.active_target = None;
        Ok(())
    }
}
