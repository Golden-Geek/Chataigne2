use super::*;

#[test]
fn stereo_audio_resamples_to_a_contiguous_clock_and_seeks_without_stale_samples() {
    const INPUT_RATE: u32 = 44_100;
    const OUTPUT_RATE: u32 = 48_000;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stereo.wav");
    let frames = INPUT_RATE / 5;
    let length = frames * 4;
    let mut wave = Vec::new();
    wave.extend(b"RIFF");
    wave.extend((36 + length).to_le_bytes());
    wave.extend(b"WAVEfmt ");
    wave.extend(16u32.to_le_bytes());
    wave.extend(1u16.to_le_bytes());
    wave.extend(2u16.to_le_bytes());
    wave.extend(INPUT_RATE.to_le_bytes());
    wave.extend((INPUT_RATE * 4).to_le_bytes());
    wave.extend(4u16.to_le_bytes());
    wave.extend(16u16.to_le_bytes());
    wave.extend(b"data");
    wave.extend(length.to_le_bytes());
    for frame in 0..frames {
        let value = if frame < frames / 2 { 8192i16 } else { -8192 };
        wave.extend(value.to_le_bytes());
        wave.extend((-value).to_le_bytes());
    }
    std::fs::write(&path, wave).unwrap();
    assert!(
        AudioReader::open_stream(&path, OUTPUT_RATE, Some(1), || false)
            .unwrap()
            .is_none()
    );

    let mut reader = AudioReader::open_stream(&path, OUTPUT_RATE, Some(0), || false)
        .unwrap()
        .unwrap();
    let mut rendered = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        assert_eq!(chunk.start_frame, rendered.len() as i64);
        rendered.extend(chunk.samples);
    }
    assert_eq!(rendered.len(), OUTPUT_RATE as usize / 5);
    assert_stereo(rendered[1000], 0.25);
    assert_stereo(rendered[7000], -0.25);

    reader.seek(6800).unwrap();
    let actual = loop {
        let chunk = reader.next_chunk().unwrap().expect("seek lands inside file");
        let offset = 6900 - chunk.start_frame;
        assert!(offset >= 0, "seek skipped target sample: {chunk:?}");
        if let Some(actual) = chunk.samples.get(offset as usize) {
            break *actual;
        }
    };
    assert_stereo(actual, -0.25);
}

#[test]
fn a_video_without_an_embedded_audio_stream_is_silent() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decode/tests/fixtures/h264.mp4");
    assert!(AudioReader::open(&path, 48_000, || false).unwrap().is_none());
    assert!(
        AudioReader::open_stream(&path, 48_000, Some(0), || false)
            .unwrap()
            .is_none()
    );
}

fn assert_stereo((left, right): (f32, f32), expected: f32) {
    assert!((left - expected).abs() < 0.001, "left sample {left}");
    assert!((right + expected).abs() < 0.001, "right sample {right}");
}
