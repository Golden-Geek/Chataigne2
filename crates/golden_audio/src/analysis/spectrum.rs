use std::{fmt, sync::Arc};

use realfft::{RealFftPlanner, RealToComplex, num_complex::Complex32};

use crate::{AudioError, AudioErrorCategory, SampleRate};

use super::{
    SpectrumAnalysisConfiguration, SpectrumBandObservation, SpectrumBandSpacing, SpectrumObservation, SpectrumWindow,
    linear_to_dbfs,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumBandGeometry {
    pub index: u16,
    pub low_hz: f32,
    pub center_hz: f32,
    pub high_hz: f32,
}

pub fn spectrum_band_geometry(
    spacing: SpectrumBandSpacing,
    minimum_frequency_hz: f32,
    maximum_frequency_hz: f32,
    band_count: u16,
    sample_rate: SampleRate,
) -> Result<Vec<SpectrumBandGeometry>, AudioError> {
    if band_count == 0 {
        return Err(AudioError::invalid_configuration(
            "spectrum geometry requires at least one band",
        ));
    }
    let nyquist = sample_rate.get() as f32 / 2.0;
    let maximum = maximum_frequency_hz.min(nyquist);
    if !minimum_frequency_hz.is_finite()
        || !maximum.is_finite()
        || minimum_frequency_hz <= 0.0
        || maximum <= minimum_frequency_hz
    {
        return Err(AudioError::invalid_configuration(
            "spectrum geometry requires a finite increasing range below Nyquist",
        ));
    }
    let count = usize::from(band_count);
    let mut edges = Vec::with_capacity(count + 1);
    for index in 0..=count {
        let fraction = index as f32 / count as f32;
        let frequency = match spacing {
            SpectrumBandSpacing::Linear => minimum_frequency_hz + (maximum - minimum_frequency_hz) * fraction,
            SpectrumBandSpacing::Logarithmic => minimum_frequency_hz * (maximum / minimum_frequency_hz).powf(fraction),
        };
        edges.push(frequency);
    }
    Ok((0..count)
        .map(|index| {
            let low_hz = edges[index];
            let high_hz = edges[index + 1];
            let center_hz = match spacing {
                SpectrumBandSpacing::Linear => (low_hz + high_hz) * 0.5,
                SpectrumBandSpacing::Logarithmic => (low_hz * high_hz).sqrt(),
            };
            SpectrumBandGeometry {
                index: index as u16,
                low_hz,
                center_hz,
                high_hz,
            }
        })
        .collect())
}

pub struct SpectrumAnalyzer {
    configuration: SpectrumAnalysisConfiguration,
    sample_rate: SampleRate,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    power_normalization: f32,
    input: Vec<f32>,
    output: Vec<Complex32>,
    scratch: Vec<Complex32>,
    geometry: Vec<SpectrumBandGeometry>,
    smoothed: Vec<f32>,
}

impl fmt::Debug for SpectrumAnalyzer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SpectrumAnalyzer")
            .field("configuration", &self.configuration)
            .field("sample_rate", &self.sample_rate)
            .field("band_count", &self.geometry.len())
            .finish_non_exhaustive()
    }
}

impl SpectrumAnalyzer {
    pub fn new(configuration: SpectrumAnalysisConfiguration, sample_rate: SampleRate) -> Result<Self, AudioError> {
        let size = configuration.fft_size as usize;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(size);
        let window = analysis_window(configuration.window, size);
        let window_energy = window.iter().map(|sample| sample * sample).sum::<f32>();
        let power_normalization = (size as f32 * window_energy).sqrt();
        let input = fft.make_input_vec();
        let output = fft.make_output_vec();
        let scratch = fft.make_scratch_vec();
        let geometry = spectrum_band_geometry(
            configuration.spacing,
            configuration.minimum_frequency_hz,
            configuration.maximum_frequency_hz,
            configuration.band_count,
            sample_rate,
        )?;
        Ok(Self {
            configuration,
            sample_rate,
            fft,
            window,
            power_normalization,
            input,
            output,
            scratch,
            smoothed: vec![0.0; geometry.len()],
            geometry,
        })
    }

    pub fn analyze(&mut self, samples: &[f32]) -> Result<SpectrumObservation, AudioError> {
        if samples.len() != self.input.len() {
            return Err(AudioError::invalid_configuration(format!(
                "spectrum frame has {} samples but FFT size is {}",
                samples.len(),
                self.input.len()
            )));
        }
        for ((destination, source), window) in self.input.iter_mut().zip(samples).zip(&self.window) {
            *destination = if source.is_finite() { *source * *window } else { 0.0 };
        }
        self.fft
            .process_with_scratch(&mut self.input, &mut self.output, &mut self.scratch)
            .map_err(|error| {
                AudioError::new(
                    AudioErrorCategory::InternalInvariant,
                    format!("real FFT processing failed: {error}"),
                )
            })?;
        let bin_width = self.sample_rate.get() as f32 / self.configuration.fft_size as f32;
        let nyquist_bin = self.output.len().saturating_sub(1);
        let update_seconds =
            self.configuration.overlap.hop_frames(self.input.len()) as f32 / self.sample_rate.get() as f32;
        let mut bands = Vec::with_capacity(self.geometry.len());
        for (band_index, geometry) in self.geometry.iter().enumerate() {
            let first = ((geometry.low_hz / bin_width).ceil() as usize).min(nyquist_bin);
            let exclusive = if band_index + 1 == self.geometry.len() {
                nyquist_bin + 1
            } else {
                (geometry.high_hz / bin_width).ceil() as usize
            };
            let mut last = exclusive.saturating_sub(1).min(nyquist_bin);
            if last < first {
                last = first;
            }
            let mut power = 0.0;
            for bin in first..=last {
                let edge_scale = if bin == 0 || bin == nyquist_bin { 1.0 } else { 2.0 };
                let amplitude = self.output[bin].norm() * edge_scale / self.power_normalization.max(f32::EPSILON);
                power += amplitude * amplitude;
            }
            let amplitude = power.sqrt();
            let time_ms = if amplitude >= self.smoothed[band_index] {
                self.configuration.attack_ms
            } else {
                self.configuration.release_ms
            };
            let smoothed = smooth(self.smoothed[band_index], amplitude, time_ms, update_seconds);
            self.smoothed[band_index] = smoothed;
            bands.push(SpectrumBandObservation {
                index: geometry.index,
                low_hz: geometry.low_hz,
                center_hz: geometry.center_hz,
                high_hz: geometry.high_hz,
                amplitude_linear: smoothed,
                amplitude_dbfs: linear_to_dbfs(smoothed),
            });
        }
        Ok(SpectrumObservation {
            fft_size: self.configuration.fft_size,
            bands,
        })
    }
}

fn analysis_window(kind: SpectrumWindow, size: usize) -> Vec<f32> {
    if size <= 1 {
        return vec![1.0; size];
    }
    let denominator = (size - 1) as f32;
    (0..size)
        .map(|index| {
            let phase = std::f32::consts::TAU * index as f32 / denominator;
            match kind {
                SpectrumWindow::Hann => 0.5 - 0.5 * phase.cos(),
                SpectrumWindow::BlackmanHarris => {
                    0.358_75 - 0.488_29 * phase.cos() + 0.141_28 * (2.0 * phase).cos() - 0.011_68 * (3.0 * phase).cos()
                }
            }
        })
        .collect()
}

fn smooth(previous: f32, next: f32, time_ms: f32, update_seconds: f32) -> f32 {
    if time_ms <= f32::EPSILON {
        return next;
    }
    let retention = (-update_seconds / (time_ms / 1_000.0)).exp();
    previous * retention + next * (1.0 - retention)
}
