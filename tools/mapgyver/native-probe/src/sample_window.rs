//! One CPU QPC window shared by independent-consumer diagnostics.

use crate::statistics::MAX_SAMPLES;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub start_qpc: u64,
    pub frequency: u64,
    pub warmup_seconds: u32,
    pub maximum_seconds: u32,
}

impl Window {
    pub fn validate(self) -> Result<(), String> {
        if self.start_qpc == 0
            || self.frequency == 0
            || self.maximum_seconds == 0
            || self.maximum_seconds > 3600
            || self.warmup_seconds >= self.maximum_seconds
            || self
                .frequency
                .checked_mul(u64::from(self.maximum_seconds))
                .and_then(|ticks| self.start_qpc.checked_add(ticks))
                .is_none()
        {
            return Err("invalid sample clock/window configuration".into());
        }
        Ok(())
    }

    pub fn capacity(self) -> usize {
        ((self.maximum_seconds - self.warmup_seconds) as usize * 60 + 1).min(MAX_SAMPLES)
    }

    pub fn sample_begin_qpc(self) -> u64 {
        self.start_qpc + self.frequency * u64::from(self.warmup_seconds)
    }

    pub fn collecting(self, qpc: u64) -> bool {
        qpc >= self.sample_begin_qpc() && qpc < self.start_qpc + self.frequency * u64::from(self.maximum_seconds)
    }
}

#[cfg(test)]
mod tests;
