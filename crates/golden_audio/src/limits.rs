use serde::{Deserialize, Serialize};

use crate::AudioError;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SampleRate(u32);

impl SampleRate {
    pub const MIN: u32 = 8_000;
    pub const MAX: u32 = 768_000;

    pub fn new(value: u32) -> Result<Self, AudioError> {
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(AudioError::invalid_configuration(format!(
                "sample rate {value} Hz is outside {}-{} Hz",
                Self::MIN,
                Self::MAX
            )));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for SampleRate {
    fn default() -> Self {
        Self(48_000)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FrameCount(u32);

impl FrameCount {
    pub fn new(value: u32) -> Result<Self, AudioError> {
        if value == 0 {
            return Err(AudioError::invalid_configuration(
                "frame count must be greater than zero",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for FrameCount {
    fn default() -> Self {
        Self(128)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EngineLimits {
    pub max_virtual_inputs: u16,
    pub max_virtual_outputs: u16,
    pub max_routes: u32,
    pub max_voices: u16,
    pub max_analysis_taps: u16,
    pub max_fft_frames: u32,
    pub max_spectrum_bands: u16,
    pub resident_asset_threshold_bytes: u64,
    pub resident_cache_budget_bytes: u64,
    pub command_queue_capacity: usize,
    pub event_queue_capacity: usize,
    pub stream_ring_frames: u32,
    pub decoder_worker_count: u16,
}

impl EngineLimits {
    pub fn validate(&self) -> Result<(), AudioError> {
        let required_nonzero = [
            ("max_virtual_inputs", u64::from(self.max_virtual_inputs)),
            ("max_virtual_outputs", u64::from(self.max_virtual_outputs)),
            ("max_routes", u64::from(self.max_routes)),
            ("max_voices", u64::from(self.max_voices)),
            ("max_analysis_taps", u64::from(self.max_analysis_taps)),
            ("max_fft_frames", u64::from(self.max_fft_frames)),
            ("max_spectrum_bands", u64::from(self.max_spectrum_bands)),
            ("resident_asset_threshold_bytes", self.resident_asset_threshold_bytes),
            ("resident_cache_budget_bytes", self.resident_cache_budget_bytes),
            ("command_queue_capacity", self.command_queue_capacity as u64),
            ("event_queue_capacity", self.event_queue_capacity as u64),
            ("stream_ring_frames", u64::from(self.stream_ring_frames)),
            ("decoder_worker_count", u64::from(self.decoder_worker_count)),
        ];
        if let Some((name, _)) = required_nonzero.into_iter().find(|(_, value)| *value == 0) {
            return Err(AudioError::invalid_configuration(format!(
                "engine limit {name} must be greater than zero"
            )));
        }
        if self.resident_asset_threshold_bytes > self.resident_cache_budget_bytes {
            return Err(AudioError::invalid_configuration(
                "resident asset threshold must not exceed the total resident cache budget",
            ));
        }
        if !self.max_fft_frames.is_power_of_two() || !(256..=16_384).contains(&self.max_fft_frames) {
            return Err(AudioError::invalid_configuration(
                "maximum FFT size must be a power of two from 256 through 16,384",
            ));
        }
        Ok(())
    }
}

impl Default for EngineLimits {
    fn default() -> Self {
        Self {
            max_virtual_inputs: 256,
            max_virtual_outputs: 256,
            max_routes: 16_384,
            max_voices: 256,
            max_analysis_taps: 64,
            max_fft_frames: 16_384,
            max_spectrum_bands: 256,
            resident_asset_threshold_bytes: 32 * 1024 * 1024,
            resident_cache_budget_bytes: 512 * 1024 * 1024,
            command_queue_capacity: 4_096,
            event_queue_capacity: 4_096,
            stream_ring_frames: 65_536,
            decoder_worker_count: 2,
        }
    }
}
