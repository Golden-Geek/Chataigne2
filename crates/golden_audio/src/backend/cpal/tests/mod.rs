use cpal::SampleFormat;

use crate::{AudioBackendState, AudioSampleFormat};

use super::{
    compiled_cpal_backends,
    discovery::{sample_format_from_cpal, sample_format_to_cpal},
    probe_cpal_backends,
};

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
