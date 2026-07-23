use serde::{Deserialize, Serialize};

use crate::{AudioError, EngineLimits, SampleRate};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum SpectrumWindow {
    Hann,
    BlackmanHarris,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum SpectrumOverlap {
    None,
    Half,
    ThreeQuarters,
}

impl SpectrumOverlap {
    #[must_use]
    pub const fn hop_frames(self, fft_frames: usize) -> usize {
        match self {
            Self::None => fft_frames,
            Self::Half => fft_frames / 2,
            Self::ThreeQuarters => fft_frames / 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum SpectrumBandSpacing {
    Linear,
    Logarithmic,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct PitchAnalysisConfiguration {
    pub frame_size: u32,
    pub minimum_frequency_hz: f32,
    pub maximum_frequency_hz: f32,
    pub power_threshold: f32,
    pub yin_threshold: f32,
    pub confidence_threshold: f32,
}

impl PitchAnalysisConfiguration {
    pub fn validate(self, sample_rate: SampleRate, limits: &EngineLimits) -> Result<(), AudioError> {
        validate_frame_size("pitch", self.frame_size, limits)?;
        validate_frequency_range(
            "pitch",
            self.minimum_frequency_hz,
            self.maximum_frequency_hz,
            sample_rate,
        )?;
        validate_unit_interval("pitch power threshold", self.power_threshold)?;
        validate_unit_interval("pitch YIN threshold", self.yin_threshold)?;
        validate_unit_interval("pitch confidence threshold", self.confidence_threshold)
    }
}

impl Default for PitchAnalysisConfiguration {
    fn default() -> Self {
        Self {
            frame_size: 2_048,
            minimum_frequency_hz: 50.0,
            maximum_frequency_hz: 2_000.0,
            power_threshold: 0.001,
            yin_threshold: 0.15,
            confidence_threshold: 0.80,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct SpectrumAnalysisConfiguration {
    pub fft_size: u32,
    pub window: SpectrumWindow,
    pub overlap: SpectrumOverlap,
    pub spacing: SpectrumBandSpacing,
    pub minimum_frequency_hz: f32,
    pub maximum_frequency_hz: f32,
    pub band_count: u16,
    pub attack_ms: f32,
    pub release_ms: f32,
}

impl SpectrumAnalysisConfiguration {
    pub fn validate(self, sample_rate: SampleRate, limits: &EngineLimits) -> Result<(), AudioError> {
        validate_frame_size("spectrum FFT", self.fft_size, limits)?;
        validate_frequency_range(
            "spectrum",
            self.minimum_frequency_hz,
            self.maximum_frequency_hz,
            sample_rate,
        )?;
        if !(1..=limits.max_spectrum_bands).contains(&self.band_count) {
            return Err(AudioError::invalid_configuration(format!(
                "spectrum band count {} is outside 1 through {}",
                self.band_count, limits.max_spectrum_bands
            )));
        }
        validate_smoothing("spectrum attack", self.attack_ms)?;
        validate_smoothing("spectrum release", self.release_ms)
    }
}

impl Default for SpectrumAnalysisConfiguration {
    fn default() -> Self {
        Self {
            fft_size: 2_048,
            window: SpectrumWindow::Hann,
            overlap: SpectrumOverlap::Half,
            spacing: SpectrumBandSpacing::Logarithmic,
            minimum_frequency_hz: 20.0,
            maximum_frequency_hz: 20_000.0,
            band_count: 64,
            attack_ms: 20.0,
            release_ms: 200.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(tag = "kind", content = "configuration", rename_all = "snake_case")]
pub enum AnalysisProcessorConfiguration {
    Pitch(PitchAnalysisConfiguration),
    Spectrum(SpectrumAnalysisConfiguration),
}

impl AnalysisProcessorConfiguration {
    pub fn validate(self, sample_rate: SampleRate, limits: &EngineLimits) -> Result<(), AudioError> {
        match self {
            Self::Pitch(configuration) => configuration.validate(sample_rate, limits),
            Self::Spectrum(configuration) => configuration.validate(sample_rate, limits),
        }
    }

    #[must_use]
    pub const fn frame_size(self) -> u32 {
        match self {
            Self::Pitch(configuration) => configuration.frame_size,
            Self::Spectrum(configuration) => configuration.fft_size,
        }
    }

    #[must_use]
    pub const fn hop_frames(self) -> usize {
        match self {
            Self::Pitch(configuration) => configuration.frame_size as usize / 2,
            Self::Spectrum(configuration) => configuration.overlap.hop_frames(configuration.fft_size as usize),
        }
    }
}

fn validate_frame_size(name: &str, size: u32, limits: &EngineLimits) -> Result<(), AudioError> {
    if !size.is_power_of_two() || !(256..=limits.max_fft_frames).contains(&size) {
        return Err(AudioError::invalid_configuration(format!(
            "{name} frame size must be a power of two from 256 through {}",
            limits.max_fft_frames
        )));
    }
    Ok(())
}

fn validate_frequency_range(name: &str, minimum: f32, maximum: f32, sample_rate: SampleRate) -> Result<(), AudioError> {
    let nyquist = sample_rate.get() as f32 / 2.0;
    if !minimum.is_finite() || !maximum.is_finite() || minimum <= 0.0 || maximum <= minimum || minimum >= nyquist {
        return Err(AudioError::invalid_configuration(format!(
            "{name} frequency range must be finite, positive, increasing, and start below Nyquist ({nyquist} Hz)"
        )));
    }
    Ok(())
}

fn validate_unit_interval(name: &str, value: f32) -> Result<(), AudioError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(AudioError::invalid_configuration(format!(
            "{name} must be finite and between zero and one"
        )));
    }
    Ok(())
}

fn validate_smoothing(name: &str, value: f32) -> Result<(), AudioError> {
    if !value.is_finite() || !(0.0..=5_000.0).contains(&value) {
        return Err(AudioError::invalid_configuration(format!(
            "{name} must be finite and between zero and 5,000 milliseconds"
        )));
    }
    Ok(())
}
