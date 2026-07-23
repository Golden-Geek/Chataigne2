use crate::{AudioError, AudioErrorCategory};

#[derive(Clone, Debug, PartialEq)]
pub struct PlanarBuffer {
    channels: usize,
    frame_capacity: usize,
    data: Vec<f32>,
}

impl PlanarBuffer {
    pub fn new(channels: usize, frame_capacity: usize) -> Result<Self, AudioError> {
        if frame_capacity == 0 {
            return Err(AudioError::invalid_configuration(
                "planar buffer frame capacity must be greater than zero",
            ));
        }
        let sample_capacity = channels.checked_mul(frame_capacity).ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::CapacityExceeded,
                "planar audio buffer sample capacity overflowed",
            )
        })?;
        Ok(Self {
            channels,
            frame_capacity,
            data: vec![0.0; sample_capacity],
        })
    }

    #[must_use]
    pub const fn channels(&self) -> usize {
        self.channels
    }

    #[must_use]
    pub const fn frame_capacity(&self) -> usize {
        self.frame_capacity
    }

    #[must_use]
    pub fn channel(&self, channel: usize) -> &[f32] {
        let start = channel * self.frame_capacity;
        &self.data[start..start + self.frame_capacity]
    }

    #[must_use]
    pub fn channel_mut(&mut self, channel: usize) -> &mut [f32] {
        let start = channel * self.frame_capacity;
        &mut self.data[start..start + self.frame_capacity]
    }

    #[must_use]
    pub fn sample(&self, channel: usize, frame: usize) -> f32 {
        self.data[channel * self.frame_capacity + frame]
    }

    pub fn set_sample(&mut self, channel: usize, frame: usize, sample: f32) {
        self.data[channel * self.frame_capacity + frame] = sample;
    }

    pub fn zero(&mut self, frames: usize) -> Result<(), AudioError> {
        self.zero_range(0, frames)
    }

    pub fn zero_range(&mut self, frame_offset: usize, frames: usize) -> Result<(), AudioError> {
        self.validate_range(frame_offset, frames)?;
        for channel in 0..self.channels {
            self.channel_mut(channel)[frame_offset..frame_offset + frames].fill(0.0);
        }
        Ok(())
    }

    pub(crate) fn validate_range(&self, frame_offset: usize, frames: usize) -> Result<(), AudioError> {
        if frame_offset
            .checked_add(frames)
            .is_none_or(|end| end > self.frame_capacity)
        {
            return Err(AudioError::invalid_configuration(format!(
                "audio buffer range {frame_offset}..{} exceeds frame capacity {}",
                frame_offset.saturating_add(frames),
                self.frame_capacity
            )));
        }
        Ok(())
    }
}
