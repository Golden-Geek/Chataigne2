use cpal::{BufferSize, SampleFormat};

use crate::{
    AudioBackendState, AudioBufferPolicy, AudioDeviceId, AudioDeviceTargetId, AudioDirection, AudioSampleFormat,
    BackendId, NegotiatedStreamFormat, SampleRate, StreamRequest,
};

use super::{
    compiled_cpal_backend_catalog, compiled_cpal_backends, cpal_backend_by_id,
    discovery::{sample_format_from_cpal, sample_format_to_cpal},
    probe_cpal_backends,
    stream::cpal_buffer_size,
};

#[test]
fn native_catalog_has_one_platform_default_and_constructs_exact_hosts() {
    let catalog = compiled_cpal_backend_catalog();

    assert_eq!(catalog.iter().filter(|backend| backend.is_platform_default).count(), 1);
    for backend in &catalog {
        let selected = cpal_backend_by_id(&backend.id).expect("every catalog entry constructs its exact backend");
        assert_eq!(selected.id(), backend.id);
        assert_eq!(backend.supports_system_default, backend.id.as_str() != "asio");
    }
}

#[test]
fn probe_reports_every_compiled_host_without_opening_a_stream() {
    let compiled = compiled_cpal_backends();
    let probes = probe_cpal_backends();

    assert_eq!(probes.len(), compiled.len());
    for (backend, probe) in compiled.iter().zip(probes) {
        assert_eq!(probe.id, backend.id());
        assert!(matches!(
            probe.state,
            AudioBackendState::Available
                | AudioBackendState::Unavailable
                | AudioBackendState::MissingServer
                | AudioBackendState::MissingDriver
                | AudioBackendState::Failed
        ));
    }
}

#[test]
fn primitive_cpal_formats_have_backend_neutral_round_trips() {
    for (cpal, golden) in [
        (SampleFormat::I8, AudioSampleFormat::I8),
        (SampleFormat::I16, AudioSampleFormat::I16),
        (SampleFormat::I24, AudioSampleFormat::I24),
        (SampleFormat::I32, AudioSampleFormat::I32),
        (SampleFormat::I64, AudioSampleFormat::I64),
        (SampleFormat::U8, AudioSampleFormat::U8),
        (SampleFormat::U16, AudioSampleFormat::U16),
        (SampleFormat::U24, AudioSampleFormat::U24),
        (SampleFormat::U32, AudioSampleFormat::U32),
        (SampleFormat::U64, AudioSampleFormat::U64),
        (SampleFormat::F32, AudioSampleFormat::F32),
        (SampleFormat::F64, AudioSampleFormat::F64),
    ] {
        assert_eq!(sample_format_from_cpal(cpal), Some(golden));
        assert_eq!(sample_format_to_cpal(golden), Some(cpal));
    }
}

#[test]
fn dsd_formats_are_not_advertised_without_a_safe_public_representation() {
    for format in [SampleFormat::DsdU8, SampleFormat::DsdU16, SampleFormat::DsdU32] {
        assert_eq!(sample_format_from_cpal(format), None);
    }
}

#[test]
fn automatic_buffers_defer_to_the_native_host_while_fixed_buffers_remain_exact() {
    let mut request = StreamRequest {
        direction: AudioDirection::Output,
        target: AudioDeviceTargetId::Device {
            backend: BackendId::new("wasapi").expect("backend"),
            device: AudioDeviceId::new("fixture").expect("device"),
        },
        engine_sample_rate: SampleRate::new(44_100).expect("sample rate"),
        channels: 2,
        buffer_policy: AudioBufferPolicy::Automatic,
    };
    let format = NegotiatedStreamFormat {
        sample_rate: 44_100,
        channels: 2,
        sample_format: AudioSampleFormat::F32,
        buffer_frames: 2_048,
        estimated_latency_ms: 46.4,
    };

    assert!(matches!(cpal_buffer_size(&request, &format), BufferSize::Default));
    request.buffer_policy = AudioBufferPolicy::Fixed(512);
    let fixed_format = NegotiatedStreamFormat {
        buffer_frames: 512,
        ..format
    };
    assert!(matches!(
        cpal_buffer_size(&request, &fixed_format),
        BufferSize::Fixed(512)
    ));
}
