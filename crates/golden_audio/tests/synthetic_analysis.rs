#![cfg(feature = "analysis")]

use golden_audio::{
    AnalysisProcessorConfiguration, AnalysisTapConfiguration, AnalysisTapId, AudioChannelId, AudioConfiguration,
    AudioEngineConfig, ConfigGeneration, EngineLimits, PitchAnalysisConfiguration, PitchAnalyzer, RenderCompileContext,
    RenderPlanCompiler, SampleRate, SpectrumAnalysisConfiguration, SpectrumAnalyzer, SpectrumBandSpacing,
    SpectrumOverlap, SpectrumWindow, VirtualInputChannel, analysis_pipeline,
};
use uuid::Uuid;

fn sine(frequency: f32, amplitude: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|frame| amplitude * (std::f32::consts::TAU * frequency * frame as f32 / 48_000.0).sin())
        .collect()
}

#[test]
fn public_pitch_surface_handles_tones_detuning_harmonics_and_rejection() {
    let mut analyzer = PitchAnalyzer::new(
        PitchAnalysisConfiguration {
            confidence_threshold: 0.75,
            ..PitchAnalysisConfiguration::default()
        },
        SampleRate::new(48_000).unwrap(),
    )
    .unwrap();
    for frequency in [82.406_9, 220.0, 440.0, 466.163_8, 880.0] {
        let result = analyzer.analyze(&sine(frequency, 0.8, 2_048));
        assert!(result.valid, "{frequency}: {result:?}");
        let cents = (1_200.0 * (result.frequency_hz / frequency).log2()).abs();
        assert!(cents < 6.0, "{frequency}: {result:?}");
    }
    let mixed = sine(220.0, 0.7, 2_048)
        .into_iter()
        .zip(sine(440.0, 0.3, 2_048))
        .map(|(fundamental, harmonic)| fundamental + harmonic)
        .collect::<Vec<_>>();
    assert!((analyzer.analyze(&mixed).frequency_hz - 220.0).abs() < 2.0);
    assert!(!analyzer.analyze(&vec![0.0; 2_048]).valid);
    assert!(!analyzer.analyze(&sine(440.0, 0.000_1, 2_048)).valid);
}

#[test]
fn public_spectrum_surface_reports_normalized_bands_and_clips_to_nyquist() {
    let configuration = SpectrumAnalysisConfiguration {
        fft_size: 2_048,
        window: SpectrumWindow::Hann,
        overlap: SpectrumOverlap::Half,
        spacing: SpectrumBandSpacing::Logarithmic,
        minimum_frequency_hz: 20.0,
        maximum_frequency_hz: 96_000.0,
        band_count: 96,
        attack_ms: 0.0,
        release_ms: 0.0,
    };
    let mut analyzer = SpectrumAnalyzer::new(configuration, SampleRate::new(48_000).unwrap()).unwrap();
    let result = analyzer.analyze(&sine(1_500.0, 1.0, 2_048)).unwrap();
    assert_eq!(result.bands.len(), 96);
    assert!((result.bands.last().unwrap().high_hz - 24_000.0).abs() < 0.01);
    let strongest = result
        .bands
        .iter()
        .max_by(|left, right| left.amplitude_linear.total_cmp(&right.amplitude_linear))
        .unwrap();
    assert!(strongest.low_hz <= 1_500.0 && strongest.high_hz >= 1_500.0);
}

#[test]
fn separate_topology_generations_never_share_observation_identity() {
    let source_one = AudioChannelId::from_uuid(Uuid::from_u128(1));
    let source_two = AudioChannelId::from_uuid(Uuid::from_u128(2));
    let tap_one = AnalysisTapId::from_uuid(Uuid::from_u128(11));
    let tap_two = AnalysisTapId::from_uuid(Uuid::from_u128(12));
    let first = analysis_plan(source_one, tap_one);
    let second = analysis_plan(source_two, tap_two);
    let (mut first_controller, first_renderer) =
        analysis_pipeline(ConfigGeneration::new(1), &first, &EngineLimits::default()).unwrap();
    let (mut second_controller, second_renderer) =
        analysis_pipeline(ConfigGeneration::new(2), &second, &EngineLimits::default()).unwrap();
    let first_snapshot = first_controller.observations().latest();
    let second_snapshot = second_controller.observations().latest();
    assert_eq!(first_snapshot.generation, ConfigGeneration::new(1));
    assert_eq!(first_snapshot.inputs[0].channel, source_one);
    assert_eq!(first_snapshot.taps[0].tap, tap_one);
    assert_eq!(second_snapshot.generation, ConfigGeneration::new(2));
    assert_eq!(second_snapshot.inputs[0].channel, source_two);
    assert_eq!(second_snapshot.taps[0].tap, tap_two);
    first_controller.shutdown().unwrap();
    second_controller.shutdown().unwrap();
    first_renderer.into_retirement().reclaim();
    second_renderer.into_retirement().reclaim();
}

fn analysis_plan(source: AudioChannelId, tap: AnalysisTapId) -> golden_audio::RenderPlan {
    let mut configuration = AudioConfiguration::empty();
    configuration.virtual_inputs.push(VirtualInputChannel {
        id: source,
        label: "Synthetic".to_owned(),
    });
    configuration.analysis_taps.push(AnalysisTapConfiguration {
        id: tap,
        source,
        enabled: true,
        processor: AnalysisProcessorConfiguration::Pitch(PitchAnalysisConfiguration::default()),
    });
    RenderPlanCompiler::new(AudioEngineConfig::default(), EngineLimits::default())
        .compile(&configuration, &RenderCompileContext::default())
        .unwrap()
        .plan
}
