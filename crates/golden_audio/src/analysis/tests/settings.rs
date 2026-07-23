use crate::{EngineLimits, PitchAnalysisConfiguration, SampleRate, SpectrumAnalysisConfiguration};
#[cfg(feature = "analysis")]
use crate::{SpectrumBandSpacing, spectrum_band_geometry};

#[test]
fn analyzer_settings_reject_unbounded_frames_ranges_thresholds_and_bands() {
    let rate = SampleRate::new(48_000).unwrap();
    let limits = EngineLimits::default();
    let pitch = PitchAnalysisConfiguration {
        frame_size: 300,
        ..PitchAnalysisConfiguration::default()
    };
    assert!(pitch.validate(rate, &limits).is_err());
    let pitch = PitchAnalysisConfiguration {
        confidence_threshold: 1.1,
        ..PitchAnalysisConfiguration::default()
    };
    assert!(pitch.validate(rate, &limits).is_err());

    let spectrum = SpectrumAnalysisConfiguration {
        band_count: 0,
        ..SpectrumAnalysisConfiguration::default()
    };
    assert!(spectrum.validate(rate, &limits).is_err());
    let spectrum = SpectrumAnalysisConfiguration {
        minimum_frequency_hz: 24_000.0,
        ..SpectrumAnalysisConfiguration::default()
    };
    assert!(spectrum.validate(rate, &limits).is_err());
    #[cfg(feature = "analysis")]
    assert!(spectrum_band_geometry(SpectrumBandSpacing::Linear, 20.0, 10.0, 8, rate).is_err());
}
