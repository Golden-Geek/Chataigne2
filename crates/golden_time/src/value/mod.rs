use std::time::Duration;

use crate::TimeError;

/// Nanoseconds in one authority's monotonic coordinate system.
///
/// The scalar alone does not identify its origin. Carry the authority's epoch when scheduling
/// work or comparing observations from different authorities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonotonicTime(u64);

impl MonotonicTime {
    /// Creates an explicit coordinate relative to the authority's monotonic origin.
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// Nanoseconds relative to that origin.
    pub const fn as_nanos(self) -> u64 {
        self.0
    }

    /// Adds a duration without saturating or wrapping a scheduling deadline.
    pub fn checked_add(self, duration: Duration) -> Result<Self, TimeError> {
        let nanos = u64::try_from(duration.as_nanos()).map_err(|_| TimeError::Overflow)?;
        self.0.checked_add(nanos).map(Self).ok_or(TimeError::Overflow)
    }

    /// Elapsed monotonic duration, rejecting an observation before the supplied anchor.
    pub fn duration_since(self, earlier: Self) -> Result<Duration, TimeError> {
        self.0
            .checked_sub(earlier.0)
            .map(Duration::from_nanos)
            .ok_or(TimeError::BeforeAnchor)
    }
}

/// Signed show position in nanoseconds, allowing explicit pre-roll and reverse transport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShowPosition(i64);

impl ShowPosition {
    /// Creates an authored/observed signed show position.
    pub const fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    /// Signed nanoseconds on the show's transport timeline.
    pub const fn as_nanos(self) -> i64 {
        self.0
    }
}

/// Position in a continuous engine sample timeline, separate from source playback offset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineSamplePosition(u64);

impl EngineSamplePosition {
    /// Creates an explicit engine sample position.
    pub const fn from_samples(samples: u64) -> Self {
        Self(samples)
    }

    /// Engine samples from the declared sample origin.
    pub const fn as_samples(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests;
