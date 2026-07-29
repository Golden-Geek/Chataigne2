use std::time::Duration;

use crate::{AudioError, SampleRate};

/// Returns the first source frame whose presentation time is at or after `offset`.
pub(crate) fn frame_at_or_after(offset: Duration, sample_rate: SampleRate) -> Result<u64, AudioError> {
    frames_for_nanos_at_or_after(offset.as_nanos(), sample_rate)
}

pub(crate) fn frames_for_nanos_at_or_after(nanos: u128, sample_rate: SampleRate) -> Result<u64, AudioError> {
    let frames = nanos
        .saturating_mul(u128::from(sample_rate.get()))
        .div_ceil(1_000_000_000);
    u64::try_from(frames).map_err(|_| AudioError::capacity_exceeded("playback start offset exceeds frame limits"))
}
