use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use crate::{AudioError, AudioErrorCategory, SampleRate, assert_not_realtime};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AudioSourceFingerprint {
    pub canonical_path: PathBuf,
    pub length_bytes: u64,
    pub modified_nanos: u128,
}

impl AudioSourceFingerprint {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, AudioError> {
        assert_not_realtime("audio source fingerprinting");
        let canonical_path = std::fs::canonicalize(path.as_ref()).map_err(|error| {
            decode_error(
                format!("failed to resolve audio source {}: {error}", path.as_ref().display()),
                path.as_ref(),
            )
        })?;
        let metadata = std::fs::metadata(&canonical_path).map_err(|error| {
            decode_error(
                format!("failed to inspect audio source {}: {error}", canonical_path.display()),
                &canonical_path,
            )
        })?;
        let modified_nanos = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_nanos());
        Ok(Self {
            canonical_path,
            length_bytes: metadata.len(),
            modified_nanos,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResidentAssetKey {
    pub source: AudioSourceFingerprint,
    pub track: u32,
    pub engine_sample_rate: SampleRate,
}

#[derive(Clone, Debug)]
pub struct ResidentAudioAsset {
    key: ResidentAssetKey,
    channels: u16,
    frames: usize,
    samples: Box<[f32]>,
}

impl ResidentAudioAsset {
    pub fn new(key: ResidentAssetKey, channels: u16, frames: usize, samples: Vec<f32>) -> Result<Self, AudioError> {
        assert_not_realtime("resident audio asset construction");
        if channels == 0 || frames == 0 {
            return Err(AudioError::invalid_configuration(
                "resident audio asset must have at least one channel and frame",
            ));
        }
        let expected = usize::from(channels)
            .checked_mul(frames)
            .ok_or_else(|| AudioError::capacity_exceeded("resident asset sample count overflowed"))?;
        if samples.len() != expected {
            return Err(AudioError::invalid_configuration(format!(
                "resident asset has {} samples but {channels} channels by {frames} frames requires {expected}",
                samples.len()
            )));
        }
        Ok(Self {
            key,
            channels,
            frames,
            samples: samples.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn key(&self) -> &ResidentAssetKey {
        &self.key
    }

    #[must_use]
    pub const fn channels(&self) -> u16 {
        self.channels
    }

    #[must_use]
    pub const fn frames(&self) -> usize {
        self.frames
    }

    #[must_use]
    pub const fn sample_rate(&self) -> SampleRate {
        self.key.engine_sample_rate
    }

    #[must_use]
    pub fn channel(&self, channel: usize) -> &[f32] {
        let start = channel * self.frames;
        &self.samples[start..start + self.frames]
    }

    #[must_use]
    pub fn sample(&self, channel: usize, frame: usize) -> f32 {
        self.samples[channel * self.frames + frame]
    }

    #[must_use]
    pub fn memory_bytes(&self) -> u64 {
        u64::try_from(self.samples.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(std::mem::size_of::<f32>() as u64)
    }
}

pub(crate) fn decode_error(message: impl Into<String>, path: &Path) -> AudioError {
    AudioError::new(AudioErrorCategory::DecodeFailed, message).with_context("path", path.display().to_string())
}
