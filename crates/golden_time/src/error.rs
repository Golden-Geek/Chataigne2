use thiserror::Error;

use crate::MonotonicTime;

/// Invalid time coordinates, conversion or activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum TimeError {
    /// The declared monotonic source could not provide a valid clock observation.
    #[error("monotonic clock observation is unavailable")]
    ClockUnavailable,
    /// A rational rate requires a nonzero denominator.
    #[error("time rate denominator must be nonzero")]
    ZeroDenominator,
    /// A scheduling/sample frequency requires a nonzero numerator.
    #[error("scheduling rate numerator must be nonzero")]
    ZeroFrequency,
    /// A conversion or discontinuity counter exceeded its representation.
    #[error("time conversion or epoch counter overflowed")]
    Overflow,
    /// A requested coordinate predates the mapping's declared anchor.
    #[error("requested time or sample position precedes the mapping anchor")]
    BeforeAnchor,
    /// A paused mapping cannot choose one unique monotonic time for a show position.
    #[error("a paused show mapping cannot be inverted")]
    PausedMapping,
    /// Clock observations must never move backwards within one authority.
    #[error("monotonic clock regressed from {previous:?} to {observed:?}")]
    ClockRegressed {
        /// Last successfully observed coordinate.
        previous: MonotonicTime,
        /// Invalid new observation.
        observed: MonotonicTime,
    },
    /// Scheduled work belongs to a different authority or discontinuity.
    #[error("activation belongs to a stale time epoch")]
    StaleEpoch,
}
