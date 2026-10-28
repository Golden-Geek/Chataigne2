use std::time::Duration;

use crate::rate::divide_signed;
use crate::{
    EngineSamplePosition, MonotonicTime, PlaybackRate, RationalRate, Rounding, ShowPosition, TimeEpoch, TimeError,
};

/// Immutable mapping published by one authority for one discontinuity epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeMapping {
    epoch: TimeEpoch,
    monotonic_origin: MonotonicTime,
    show_origin: ShowPosition,
    rate: PlaybackRate,
}

impl TimeMapping {
    pub(crate) const fn new(
        epoch: TimeEpoch,
        monotonic_origin: MonotonicTime,
        show_origin: ShowPosition,
        rate: PlaybackRate,
    ) -> Self {
        Self {
            epoch,
            monotonic_origin,
            show_origin,
            rate,
        }
    }

    /// Authority/discontinuity identity for this mapping.
    pub const fn epoch(self) -> TimeEpoch {
        self.epoch
    }

    /// Epoch's monotonic anchor.
    pub const fn monotonic_origin(self) -> MonotonicTime {
        self.monotonic_origin
    }

    /// Show position at the epoch's monotonic anchor.
    pub const fn show_origin(self) -> ShowPosition {
        self.show_origin
    }

    /// App-selected affine transport rate.
    pub const fn rate(self) -> PlaybackRate {
        self.rate
    }

    /// Maps monotonic time into signed show nanoseconds, quantized toward negative infinity.
    ///
    /// The original anchor is retained, so repeated observations accumulate no rounded delta.
    pub fn show_at(self, monotonic: MonotonicTime) -> Result<ShowPosition, TimeError> {
        let elapsed = monotonic.duration_since(self.monotonic_origin)?;
        let numerator =
            i128::try_from(elapsed.as_nanos()).map_err(|_| TimeError::Overflow)? * i128::from(self.rate.numerator());
        let delta = divide_signed(numerator, i128::from(self.rate.denominator()), Rounding::Floor)?;
        let position = i128::from(self.show_origin.as_nanos()) + delta;
        i64::try_from(position)
            .map(ShowPosition::from_nanos)
            .map_err(|_| TimeError::Overflow)
    }

    /// Inverts a moving show mapping with explicit monotonic-nanosecond quantization.
    ///
    /// Paused mappings are noninvertible. A position that would precede this epoch's anchor
    /// is rejected; applications retain late-admission/seek policy instead of silently clamping.
    pub fn monotonic_at(self, show: ShowPosition, rounding: Rounding) -> Result<MonotonicTime, TimeError> {
        if self.rate.numerator() == 0 {
            return Err(TimeError::PausedMapping);
        }
        let delta = i128::from(show.as_nanos()) - i128::from(self.show_origin.as_nanos());
        if (delta < 0 && self.rate.numerator() > 0) || (delta > 0 && self.rate.numerator() < 0) {
            return Err(TimeError::BeforeAnchor);
        }
        let numerator = delta * i128::from(self.rate.denominator());
        let nanos = divide_signed(numerator, i128::from(self.rate.numerator()), rounding)?;
        if nanos < 0 {
            return Err(TimeError::BeforeAnchor);
        }
        let nanos = u64::try_from(nanos).map_err(|_| TimeError::Overflow)?;
        self.monotonic_origin.checked_add(Duration::from_nanos(nanos))
    }
}

/// Explicit conversion between monotonic coordinates and a continuous engine sample timeline.
///
/// Both anchors belong to one declared clock coordinate system. A show seek/loop/pause does
/// not reset engine samples or mutate this mapping. Authority replacement with a different
/// monotonic origin requires an explicit new sample mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleMapping {
    monotonic_origin: MonotonicTime,
    sample_origin: EngineSamplePosition,
    rate: RationalRate,
}

impl SampleMapping {
    /// Declares the monotonic/sample anchor and engine sample rate.
    pub const fn new(monotonic_origin: MonotonicTime, sample_origin: EngineSamplePosition, rate: RationalRate) -> Self {
        Self {
            monotonic_origin,
            sample_origin,
            rate,
        }
    }

    /// Converts elapsed monotonic time into engine samples using explicit quantization.
    pub fn sample_at(self, monotonic: MonotonicTime, rounding: Rounding) -> Result<EngineSamplePosition, TimeError> {
        let elapsed = monotonic.duration_since(self.monotonic_origin)?;
        self.sample_origin
            .as_samples()
            .checked_add(self.rate.ticks_for(elapsed, rounding)?)
            .map(EngineSamplePosition::from_samples)
            .ok_or(TimeError::Overflow)
    }

    /// Converts an engine sample target into the declared monotonic coordinate system.
    pub fn monotonic_at(self, sample: EngineSamplePosition, rounding: Rounding) -> Result<MonotonicTime, TimeError> {
        let elapsed_samples = sample
            .as_samples()
            .checked_sub(self.sample_origin.as_samples())
            .ok_or(TimeError::BeforeAnchor)?;
        self.monotonic_origin
            .checked_add(self.rate.duration_for(elapsed_samples, rounding)?)
    }
}

#[cfg(test)]
mod tests;
