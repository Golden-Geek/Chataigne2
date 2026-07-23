use crate::{PitchAnalysisConfiguration, PitchAnalyzer, SampleRate};

fn sine(frequency: f32, amplitude: f32, frames: usize, sample_rate: u32) -> Vec<f32> {
    (0..frames)
        .map(|frame| amplitude * (std::f32::consts::TAU * frequency * frame as f32 / sample_rate as f32).sin())
        .collect()
}

fn cents_error(measured: f32, expected: f32) -> f32 {
    (1_200.0 * (measured / expected).log2()).abs()
}

#[test]
fn yin_tracks_tones_harmonics_and_detuning_with_bounded_error() {
    let sample_rate = SampleRate::new(48_000).unwrap();
    let configuration = PitchAnalysisConfiguration {
        confidence_threshold: 0.75,
        ..PitchAnalysisConfiguration::default()
    };
    let mut analyzer = PitchAnalyzer::new(configuration, sample_rate).unwrap();
    for frequency in [110.0, 220.0, 440.0, 445.0, 880.0] {
        let result = analyzer.analyze(&sine(frequency, 0.8, 2_048, 48_000));
        assert!(result.valid, "{frequency}: {result:?}");
        assert!(
            cents_error(result.frequency_hz, frequency) < 5.0,
            "{frequency}: {result:?}"
        );
    }

    let fundamental = sine(220.0, 0.7, 2_048, 48_000);
    let harmonic = sine(440.0, 0.3, 2_048, 48_000);
    let mixed = fundamental
        .iter()
        .zip(harmonic)
        .map(|(fundamental, harmonic)| fundamental + harmonic)
        .collect::<Vec<_>>();
    let result = analyzer.analyze(&mixed);
    assert!(result.valid, "{result:?}");
    assert!(cents_error(result.frequency_hz, 220.0) < 5.0, "{result:?}");
}

#[test]
fn yin_rejects_silence_noise_and_below_threshold_signals() {
    let sample_rate = SampleRate::new(48_000).unwrap();
    let mut analyzer = PitchAnalyzer::new(PitchAnalysisConfiguration::default(), sample_rate).unwrap();
    assert!(!analyzer.analyze(&vec![0.0; 2_048]).valid);
    assert!(!analyzer.analyze(&sine(440.0, 0.000_1, 2_048, 48_000)).valid);

    let mut state = 0x1234_5678_u32;
    let noise = (0..2_048)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32 - 0.5) * 0.2
        })
        .collect::<Vec<_>>();
    let result = analyzer.analyze(&noise);
    assert!(!result.valid, "{result:?}");
}

#[test]
fn pitch_result_exposes_midi_note_name_and_cents() {
    let mut analyzer =
        PitchAnalyzer::new(PitchAnalysisConfiguration::default(), SampleRate::new(48_000).unwrap()).unwrap();
    let result = analyzer.analyze(&sine(440.0, 0.8, 2_048, 48_000));
    assert!(result.valid);
    assert_eq!(result.midi_note, 69);
    assert_eq!(result.note_name, "A4");
    assert!(result.cents.abs() < 5.0);
}

#[test]
fn chirp_never_produces_a_confident_result_outside_its_swept_range() {
    let mut analyzer =
        PitchAnalyzer::new(PitchAnalysisConfiguration::default(), SampleRate::new(48_000).unwrap()).unwrap();
    let mut phase = 0.0;
    let chirp = (0..2_048)
        .map(|frame| {
            let frequency = 300.0 + 300.0 * frame as f32 / 2_047.0;
            phase += std::f32::consts::TAU * frequency / 48_000.0;
            0.8 * phase.sin()
        })
        .collect::<Vec<_>>();
    let result = analyzer.analyze(&chirp);
    assert!(
        !result.valid || (300.0..=600.0).contains(&result.frequency_hz),
        "{result:?}"
    );
}
