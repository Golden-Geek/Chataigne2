use std::time::Duration;

use crate::{AudioError, AudioErrorCategory, FrameCount, SampleRate};

#[derive(Clone, Debug)]
pub struct OfflineClock {
    sample_rate: SampleRate,
    frame: u64,
}

impl OfflineClock {
    pub fn new(sample_rate: SampleRate) -> Result<Self, AudioError> {
        SampleRate::new(sample_rate.get())?;
        Ok(Self { sample_rate, frame: 0 })
    }

    #[must_use]
    pub const fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    #[must_use]
    pub const fn frame(&self) -> u64 {
        self.frame
    }

    pub fn advance(&mut self, frames: FrameCount) -> Result<u64, AudioError> {
        self.frame = self.frame.checked_add(u64::from(frames.get())).ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::CapacityExceeded,
                "offline clock frame counter overflowed",
            )
        })?;
        Ok(self.frame)
    }

    #[must_use]
    pub fn elapsed(&self) -> Duration {
        Duration::from_secs_f64(self.frame as f64 / f64::from(self.sample_rate.get()))
    }
}
