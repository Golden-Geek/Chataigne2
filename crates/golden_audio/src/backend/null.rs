use crate::{
    AudioBackend, AudioBackendState, AudioDeviceDescriptor, AudioDeviceFingerprint, AudioDeviceId,
    AudioDeviceReadiness, AudioDeviceTargetId, AudioDirection, AudioError, AudioPermissionState, AudioSampleFormat,
    AudioStream, AudioStreamStatus, BackendDescriptor, BackendId, DeviceNegotiator, PhysicalChannelDescriptor,
    PhysicalChannelKey, StreamRequest, SupportedBufferFrames, SupportedStreamConfiguration, profile_key_for,
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
    fn id(&self) -> BackendId {
        Self::backend_id()
    }

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
        let target = Self::target();
        let fingerprint = AudioDeviceFingerprint {
            product: Some("Golden Audio Null Duplex".to_owned()),
            input_channels: 2,
            output_channels: 2,
            ..AudioDeviceFingerprint::default()
        };
        Ok(vec![AudioDeviceDescriptor {
            profile_key: profile_key_for(&target, None, None),
            target,
            label: "Null Duplex".to_owned(),
            stable_id: true,
            fingerprint,
            input_channels,
            output_channels,
            supported_configurations: vec![
                null_supported_configuration(AudioDirection::Input),
                null_supported_configuration(AudioDirection::Output),
            ],
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
        let device = self
            .discover()?
            .into_iter()
            .next()
            .expect("null backend always provides one device");
        let format = DeviceNegotiator.negotiate(&device, request.negotiation_request())?;
        Ok(Box::new(NullStream::new(request, format)))
    }
}

fn null_supported_configuration(direction: AudioDirection) -> SupportedStreamConfiguration {
    SupportedStreamConfiguration {
        direction,
        channels: 2,
        sample_format: AudioSampleFormat::F32,
        min_sample_rate: crate::SampleRate::MIN,
        max_sample_rate: crate::SampleRate::MAX,
        buffer_frames: SupportedBufferFrames {
            min: 1,
            max: 65_536,
            preferred: 128,
        },
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
    fn new(request: &StreamRequest, format: crate::NegotiatedStreamFormat) -> Self {
        Self {
            status: AudioStreamStatus {
                direction: request.direction,
                enabled: true,
                selected_target: Some(request.target.clone()),
                selected_label: Some("Null Duplex".to_owned()),
                profile_key: Some(profile_key_for(&request.target, None, None)),
                active_target: Some(request.target.clone()),
                readiness: AudioDeviceReadiness::Ready,
                permission: AudioPermissionState::NotRequired,
                recovery_policy: crate::AudioRecoveryPolicy::WaitForSelected,
                retry_attempt: 0,
                next_retry_ms: None,
                format: Some(format),
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
