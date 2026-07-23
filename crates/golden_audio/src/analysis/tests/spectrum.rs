use crate::{
    SampleRate, SpectrumAnalysisConfiguration, SpectrumAnalyzer, SpectrumBandSpacing, SpectrumOverlap, SpectrumWindow,
    spectrum_band_geometry,
};

fn sine(frequency: f32, frames: usize, sample_rate: u32) -> Vec<f32> {
    (0..frames)
        .map(|frame| (std::f32::consts::TAU * frequency * frame as f32 / sample_rate as f32).sin())
        .collect()
}

fn configuration(fft_size: u32) -> SpectrumAnalysisConfiguration {
    SpectrumAnalysisConfiguration {
        fft_size,
        window: SpectrumWindow::Hann,
        overlap: SpectrumOverlap::None,
        spacing: SpectrumBandSpacing::Linear,
        minimum_frequency_hz: 20.0,
        maximum_frequency_hz: 30_000.0,
        band_count: 48,
        attack_ms: 0.0,
        release_ms: 0.0,
    }
}

#[test]
fn fft_finds_bin_centered_and_off_bin_tones_in_the_expected_band() {
    let sample_rate = SampleRate::new(48_000).unwrap();
    let mut analyzer = SpectrumAnalyzer::new(configuration(1_024), sample_rate).unwrap();
    for frequency in [2_812.5, 2_830.0] {
        let result = analyzer.analyze(&sine(frequency, 1_024, 48_000)).unwrap();
        let strongest = result
            .bands
            .iter()
            .max_by(|left, right| left.amplitude_linear.total_cmp(&right.amplitude_linear))
            .unwrap();
        assert!(
            strongest.low_hz <= frequency && strongest.high_hz >= frequency,
            "{frequency}: {strongest:?}"
        );
        assert!(strongest.amplitude_linear > 0.8, "{frequency}: {strongest:?}");
    }
}

#[test]
fn band_geometry_supports_linear_logarithmic_and_nyquist_clipping() {
    let sample_rate = SampleRate::new(48_000).unwrap();
    for spacing in [SpectrumBandSpacing::Linear, SpectrumBandSpacing::Logarithmic] {
        let bands = spectrum_band_geometry(spacing, 20.0, 96_000.0, 32, sample_rate).unwrap();
        assert_eq!(bands.len(), 32);
        assert!((bands.last().unwrap().high_hz - 24_000.0).abs() < 0.01);
        for pair in bands.windows(2) {
            assert!((pair[0].high_hz - pair[1].low_hz).abs() < 0.01);
            assert!(pair[0].center_hz < pair[1].center_hz);
        }
    }
}

#[test]
fn smoothing_and_fft_size_changes_are_bounded() {
    let sample_rate = SampleRate::new(48_000).unwrap();
    let mut smoothed_configuration = configuration(512);
    smoothed_configuration.band_count = 1;
    smoothed_configuration.attack_ms = 0.0;
    smoothed_configuration.release_ms = 500.0;
    let mut analyzer = SpectrumAnalyzer::new(smoothed_configuration, sample_rate).unwrap();
    let loud = analyzer.analyze(&sine(1_000.0, 512, 48_000)).unwrap().bands[0].amplitude_linear;
    let released = analyzer.analyze(&vec![0.0; 512]).unwrap().bands[0].amplitude_linear;
    assert!(released > 0.0 && released < loud);

    let mut larger = SpectrumAnalyzer::new(configuration(2_048), sample_rate).unwrap();
    let result = larger.analyze(&sine(1_500.0, 2_048, 48_000)).unwrap();
    assert_eq!(result.fft_size, 2_048);
}

#[test]
fn blackman_harris_window_is_normalized_for_a_bin_centered_tone() {
    let sample_rate = SampleRate::new(48_000).unwrap();
    let mut config = configuration(1_024);
    config.window = SpectrumWindow::BlackmanHarris;
    let mut analyzer = SpectrumAnalyzer::new(config, sample_rate).unwrap();
    let result = analyzer.analyze(&sine(2_812.5, 1_024, 48_000)).unwrap();
    let strongest = result
        .bands
        .iter()
        .map(|band| band.amplitude_linear)
        .fold(0.0, f32::max);
    assert!((strongest - 1.0).abs() < 0.05, "{strongest}");
}
