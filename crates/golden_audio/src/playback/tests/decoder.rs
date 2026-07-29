use std::{io::Write, time::Duration};

use tempfile::Builder;

use crate::{AudioErrorCategory, SampleRate};

use super::{
    super::decoder::DecodingSession,
    super::{decode_audio_file, probe_audio_file, supported_audio_extensions},
    encoded_fixtures::encoded_fixture,
    fixtures::{ramp_wave_file, sine_wave_file},
};

#[test]
fn wave_probe_decode_and_worker_rate_conversion_are_deterministic() {
    let file = sine_wave_file(2, 44_100, 441);
    let probe = probe_audio_file(file.path()).unwrap();
    assert_eq!(probe.channels, 2);
    assert_eq!(probe.sample_rate.get(), 44_100);
    assert_eq!(probe.source_frames, Some(441));

    let asset = decode_audio_file(file.path(), SampleRate::new(48_000).unwrap()).unwrap();
    assert_eq!(asset.channels(), 2);
    assert_eq!(asset.sample_rate().get(), 48_000);
    assert_eq!(asset.frames(), 480);
    assert!(asset.channel(0).iter().any(|sample| sample.abs() > 0.1));
}

#[test]
fn every_advertised_extension_family_probes_and_decodes_a_real_fixture() {
    let source = sine_wave_file(1, 48_000, 64);
    for extension in supported_audio_extensions() {
        let bytes = if matches!(*extension, "wav" | "wave") {
            std::fs::read(source.path()).unwrap()
        } else {
            encoded_fixture(extension)
        };
        let mut file = Builder::new().suffix(&format!(".{extension}")).tempfile().unwrap();
        file.write_all(&bytes).unwrap();
        file.flush().unwrap();
        let asset = decode_audio_file(file.path(), SampleRate::default()).unwrap();
        assert!(asset.frames() > 0, "extension fixture {extension}");
    }
}

#[test]
fn unsupported_corrupt_and_truncated_sources_fail_without_panicking() {
    let mut corrupt = Builder::new().suffix(".wav").tempfile().unwrap();
    corrupt.write_all(b"this is not audio").unwrap();
    let error = probe_audio_file(corrupt.path()).unwrap_err();
    assert!(matches!(
        error.category,
        AudioErrorCategory::UnsupportedFormat | AudioErrorCategory::DecodeFailed
    ));

    let valid = sine_wave_file(1, 48_000, 128);
    let bytes = std::fs::read(valid.path()).unwrap();
    let mut truncated = Builder::new().suffix(".wav").tempfile().unwrap();
    truncated.write_all(&bytes[..48]).unwrap();
    truncated.flush().unwrap();
    let error = decode_audio_file(truncated.path(), SampleRate::default()).unwrap_err();
    assert_eq!(error.category, AudioErrorCategory::DecodeFailed);
}

#[test]
fn accurate_seek_trims_decoded_wave_frames_before_returning_audio() {
    const SAMPLE_RATE: u32 = 8_000;
    const START_FRAME: u32 = 87;
    let file = ramp_wave_file(1, SAMPLE_RATE, 512);
    let mut session = DecodingSession::open(file.path()).unwrap();

    session
        .seek_to(Duration::from_micros(
            u64::from(START_FRAME) * 1_000_000 / u64::from(SAMPLE_RATE),
        ))
        .unwrap();
    let chunk = session.next_planar_chunk().unwrap().unwrap();

    let expected = START_FRAME as f32 / 32_768.0;
    assert!(
        (chunk[0][0] - expected).abs() < 1.0e-6,
        "seek returned {}, expected the exact source frame {START_FRAME} ({expected})",
        chunk[0][0],
    );
}
