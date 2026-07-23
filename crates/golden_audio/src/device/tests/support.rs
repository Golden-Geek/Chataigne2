use crate::{
    AudioBackendState, AudioBackendStatus, AudioDeviceDescriptor, AudioDeviceFingerprint, AudioDeviceId,
    AudioDeviceTargetId, AudioDirection, AudioSampleFormat, BackendId, PhysicalChannelDescriptor, PhysicalChannelKey,
    SupportedBufferFrames, SupportedStreamConfiguration, profile_key_for,
};

pub fn backend_id() -> BackendId {
    BackendId::from_static("fixture")
}

pub fn backend_status() -> AudioBackendStatus {
    AudioBackendStatus {
        backend: backend_id(),
        label: "Fixture".to_owned(),
        state: AudioBackendState::Available,
        detail: None,
    }
}

pub fn fingerprint(product: &str, input_channels: u16, output_channels: u16) -> AudioDeviceFingerprint {
    AudioDeviceFingerprint {
        vendor: Some("Golden".to_owned()),
        product: Some(product.to_owned()),
        transport: Some("virtual".to_owned()),
        input_channels,
        output_channels,
        ..AudioDeviceFingerprint::default()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn device(
    id: &str,
    label: &str,
    stable_id: bool,
    fingerprint: AudioDeviceFingerprint,
    input_channels: u16,
    output_channels: u16,
    default_input: bool,
    default_output: bool,
) -> AudioDeviceDescriptor {
    let target = AudioDeviceTargetId::Device {
        backend: backend_id(),
        device: AudioDeviceId::new(id).unwrap(),
    };
    let profile_key = profile_key_for(&target, (!stable_id).then_some(&fingerprint), None);
    let mut supported_configurations = Vec::new();
    if input_channels > 0 {
        supported_configurations.push(configuration(
            AudioDirection::Input,
            input_channels,
            AudioSampleFormat::F32,
            44_100,
            96_000,
            128,
        ));
    }
    if output_channels > 0 {
        supported_configurations.push(configuration(
            AudioDirection::Output,
            output_channels,
            AudioSampleFormat::F32,
            44_100,
            96_000,
            128,
        ));
    }
    AudioDeviceDescriptor {
        target,
        label: label.to_owned(),
        stable_id,
        fingerprint,
        profile_key,
        input_channels: channels("input", input_channels),
        output_channels: channels("output", output_channels),
        supported_configurations,
        is_system_default_input: default_input,
        is_system_default_output: default_output,
    }
}

pub fn configuration(
    direction: AudioDirection,
    channels: u16,
    sample_format: AudioSampleFormat,
    min_sample_rate: u32,
    max_sample_rate: u32,
    preferred_buffer: u32,
) -> SupportedStreamConfiguration {
    SupportedStreamConfiguration {
        direction,
        channels,
        sample_format,
        min_sample_rate,
        max_sample_rate,
        buffer_frames: SupportedBufferFrames {
            min: 32,
            max: 1_024,
            preferred: preferred_buffer,
        },
    }
}

fn channels(prefix: &str, count: u16) -> Vec<PhysicalChannelDescriptor> {
    (0..count)
        .map(|index| PhysicalChannelDescriptor {
            key: PhysicalChannelKey::new(format!("{prefix}:{index}")).unwrap(),
            label: format!("{prefix} {}", index + 1),
            position: None,
        })
        .collect()
}
