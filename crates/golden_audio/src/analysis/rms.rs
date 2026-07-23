use crate::{AudioChannelId, AudioError, PlanarBuffer};

use super::ChannelObservation;

pub const DBFS_FLOOR: f32 = -120.0;

#[must_use]
pub fn linear_to_dbfs(linear: f32) -> f32 {
    if !linear.is_finite() || linear <= 0.0 {
        DBFS_FLOOR
    } else {
        (20.0 * linear.log10()).max(DBFS_FLOOR)
    }
}

#[derive(Clone, Debug)]
pub struct MeterAccumulator {
    channels: Vec<AudioChannelId>,
    sum_squares: Vec<f64>,
    peaks: Vec<f32>,
    observations: Vec<ChannelObservation>,
    window_frames: usize,
    accumulated_frames: usize,
}

impl MeterAccumulator {
    pub fn new(channels: Vec<AudioChannelId>, window_frames: usize) -> Result<Self, AudioError> {
        if window_frames == 0 {
            return Err(AudioError::invalid_configuration(
                "meter observation window must contain at least one frame",
            ));
        }
        let observations = channels
            .iter()
            .map(|channel| ChannelObservation {
                channel: *channel,
                rms_linear: 0.0,
                rms_dbfs: DBFS_FLOOR,
                peak_dbfs: DBFS_FLOOR,
                clipped: false,
            })
            .collect();
        Ok(Self {
            sum_squares: vec![0.0; channels.len()],
            peaks: vec![0.0; channels.len()],
            observations,
            channels,
            window_frames,
            accumulated_frames: 0,
        })
    }

    #[must_use]
    pub const fn window_frames(&self) -> usize {
        self.window_frames
    }

    #[must_use]
    pub const fn accumulated_frames(&self) -> usize {
        self.accumulated_frames
    }

    pub fn accumulate(
        &mut self,
        source: &PlanarBuffer,
        frames: usize,
        mut publish: impl FnMut(&[ChannelObservation]),
    ) -> Result<(), AudioError> {
        if source.channels() < self.channels.len() {
            return Err(AudioError::capacity_exceeded(
                "meter source has fewer channels than the configured meter bank",
            ));
        }
        source.validate_range(0, frames)?;
        let mut offset = 0;
        while offset < frames {
            let remaining = self.window_frames - self.accumulated_frames;
            let chunk = remaining.min(frames - offset);
            for channel in 0..self.channels.len() {
                for sample in &source.channel(channel)[offset..offset + chunk] {
                    let sample = if sample.is_finite() { *sample } else { 0.0 };
                    self.sum_squares[channel] += f64::from(sample) * f64::from(sample);
                    self.peaks[channel] = self.peaks[channel].max(sample.abs());
                }
            }
            self.accumulated_frames += chunk;
            offset += chunk;
            if self.accumulated_frames == self.window_frames {
                self.update_observations();
                publish(&self.observations);
                self.reset();
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn current(&self) -> Vec<ChannelObservation> {
        let mut current = self.clone();
        current.update_observations();
        current.observations
    }

    #[must_use]
    pub fn latest(&self) -> &[ChannelObservation] {
        &self.observations
    }

    fn update_observations(&mut self) {
        let divisor = self.accumulated_frames.max(1) as f64;
        for (index, observation) in self.observations.iter_mut().enumerate() {
            let rms = (self.sum_squares[index] / divisor).sqrt() as f32;
            let peak = self.peaks[index];
            observation.rms_linear = rms;
            observation.rms_dbfs = linear_to_dbfs(rms);
            observation.peak_dbfs = linear_to_dbfs(peak);
            observation.clipped = peak >= 1.0;
        }
    }

    fn reset(&mut self) {
        self.sum_squares.fill(0.0);
        self.peaks.fill(0.0);
        self.accumulated_frames = 0;
    }
}
