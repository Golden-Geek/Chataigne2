use crate::{
    AudioBufferPolicy, AudioDirection, AudioErrorCategory, AudioSampleFormat, ChannelCountPolicy, DeviceNegotiator,
    SampleFormatPolicy, SampleRate, SampleRatePolicy, StreamNegotiationRequest,
};

use super::support::{configuration, device, fingerprint};

#[test]
fn negotiation_is_deterministic_across_capability_enumeration_order() {
    let mut first = device("device", "Device", true, fingerprint("Device", 0, 2), 0, 2, false, true);
    first.supported_configurations = vec![
        configuration(AudioDirection::Output, 2, AudioSampleFormat::I16, 48_000, 48_000, 256),
        configuration(AudioDirection::Output, 2, AudioSampleFormat::F32, 44_100, 96_000, 128),
    ];
    let mut reversed = first.clone();
    reversed.supported_configurations.reverse();
    let request = StreamNegotiationRequest {
        direction: AudioDirection::Output,
        channels: ChannelCountPolicy::Exact(2),
        sample_rate: SampleRatePolicy::ClosestTo(SampleRate::new(48_000).unwrap()),
        sample_format: SampleFormatPolicy::PreferF32,
        buffer: AudioBufferPolicy::Automatic,
    };

    let first_result = DeviceNegotiator.negotiate(&first, request).unwrap();
    let reversed_result = DeviceNegotiator.negotiate(&reversed, request).unwrap();
    assert_eq!(first_result, reversed_result);
    assert_eq!(first_result.sample_format, AudioSampleFormat::F32);
    assert_eq!(first_result.sample_rate, 48_000);
    assert_eq!(first_result.buffer_frames, 128);
}

#[test]
fn strict_unsupported_request_is_rejected_with_context() {
    let device = device("device", "Device", true, fingerprint("Device", 0, 2), 0, 2, false, true);
    let error = DeviceNegotiator
        .negotiate(
            &device,
            StreamNegotiationRequest {
                direction: AudioDirection::Output,
                channels: ChannelCountPolicy::Exact(8),
                sample_rate: SampleRatePolicy::Exact(SampleRate::new(192_000).unwrap()),
                sample_format: SampleFormatPolicy::Exact(AudioSampleFormat::I16),
                buffer: AudioBufferPolicy::Fixed(4_096),
            },
        )
        .unwrap_err();

    assert_eq!(error.category, AudioErrorCategory::UnsupportedFormat);
    assert_eq!(error.context.get("device").map(String::as_str), Some("Device"));
}
