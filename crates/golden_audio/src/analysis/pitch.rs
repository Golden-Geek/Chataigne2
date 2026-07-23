use crate::{AudioError, SampleRate};

use super::{PitchAnalysisConfiguration, PitchObservation};

#[derive(Debug)]
pub struct PitchAnalyzer {
    configuration: PitchAnalysisConfiguration,
    sample_rate: SampleRate,
    input: Vec<f32>,
    difference: Vec<f32>,
}

impl PitchAnalyzer {
    pub fn new(configuration: PitchAnalysisConfiguration, sample_rate: SampleRate) -> Result<Self, AudioError> {
        let frame_size = configuration.frame_size as usize;
        let maximum_tau = ((sample_rate.get() as f32 / configuration.minimum_frequency_hz).ceil() as usize)
            .min(frame_size.saturating_sub(2));
        let minimum_tau = ((sample_rate.get() as f32 / configuration.maximum_frequency_hz).floor() as usize).max(2);
        if maximum_tau < 3 || minimum_tau > maximum_tau {
            return Err(AudioError::invalid_configuration(
                "pitch frame is too short for the configured frequency range",
            ));
        }
        Ok(Self {
            configuration,
            sample_rate,
            input: vec![0.0; frame_size],
            difference: vec![0.0; maximum_tau + 1],
        })
    }

    #[must_use]
    pub fn analyze(&mut self, samples: &[f32]) -> PitchObservation {
        let frame_size = self.configuration.frame_size as usize;
        if samples.len() != frame_size {
            return PitchObservation::invalid();
        }
        for (destination, source) in self.input.iter_mut().zip(samples) {
            *destination = if source.is_finite() { *source } else { 0.0 };
        }
        let power = self.input.iter().map(|sample| sample * sample).sum::<f32>() / frame_size as f32;
        if power.sqrt() < self.configuration.power_threshold {
            return PitchObservation::invalid();
        }

        let minimum_tau =
            ((self.sample_rate.get() as f32 / self.configuration.maximum_frequency_hz).floor() as usize).max(2);
        let maximum_tau = self.difference.len() - 1;
        self.difference.fill(0.0);
        for tau in 1..=maximum_tau {
            let mut difference = 0.0;
            for index in 0..frame_size - tau {
                let delta = self.input[index] - self.input[index + tau];
                difference += delta * delta;
            }
            self.difference[tau] = difference;
        }

        let mut running_sum = 0.0;
        for tau in 1..=maximum_tau {
            running_sum += self.difference[tau];
            self.difference[tau] = if running_sum > f32::EPSILON {
                self.difference[tau] * tau as f32 / running_sum
            } else {
                1.0
            };
        }

        let mut selected = None;
        let mut tau = minimum_tau;
        while tau <= maximum_tau {
            if self.difference[tau] < self.configuration.yin_threshold {
                while tau < maximum_tau && self.difference[tau + 1] < self.difference[tau] {
                    tau += 1;
                }
                selected = Some(tau);
                break;
            }
            tau += 1;
        }
        let selected = selected.unwrap_or_else(|| {
            (minimum_tau..=maximum_tau)
                .min_by(|left, right| self.difference[*left].total_cmp(&self.difference[*right]))
                .unwrap_or(minimum_tau)
        });
        let confidence = (1.0 - self.difference[selected]).clamp(0.0, 1.0);
        if confidence < self.configuration.confidence_threshold {
            return PitchObservation {
                confidence,
                ..PitchObservation::invalid()
            };
        }

        let refined_tau = parabolic_period(&self.difference, selected);
        let frequency = self.sample_rate.get() as f32 / refined_tau;
        if !frequency.is_finite()
            || frequency < self.configuration.minimum_frequency_hz
            || frequency > self.configuration.maximum_frequency_hz
        {
            return PitchObservation {
                confidence,
                ..PitchObservation::invalid()
            };
        }
        pitch_observation(frequency, confidence)
    }
}

fn parabolic_period(values: &[f32], index: usize) -> f32 {
    if index == 0 || index + 1 >= values.len() {
        return index as f32;
    }
    let left = values[index - 1];
    let center = values[index];
    let right = values[index + 1];
    let denominator = 2.0 * (2.0 * center - right - left);
    if denominator.abs() <= f32::EPSILON {
        index as f32
    } else {
        index as f32 + (right - left) / denominator
    }
}

fn pitch_observation(frequency: f32, confidence: f32) -> PitchObservation {
    const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let midi_exact = 69.0 + 12.0 * (frequency / 440.0).log2();
    let midi_note = midi_exact.round().clamp(0.0, 127.0) as i16;
    let note_index = midi_note.rem_euclid(12) as usize;
    let octave = midi_note.div_euclid(12) - 1;
    PitchObservation {
        valid: true,
        frequency_hz: frequency,
        confidence,
        midi_note,
        note_name: format!("{}{octave}", NOTE_NAMES[note_index]),
        cents: (midi_exact - f32::from(midi_note)) * 100.0,
    }
}
