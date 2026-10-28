//! Shared time coordinates and activation contracts for Golden runtimes.
//!
//! One authority maps a monotonic clock into show positions. Applications choose transport
//! policy; audio/video retain scheduling and device policy. No consumer advances show time by
//! counting its callbacks. Discontinuity epochs fence scheduled work independently from engine
//! sample positions, whose continuous mapping is explicit.

#![warn(missing_docs)]

mod activation;
mod authority;
mod error;
mod mapping;
mod rate;
mod value;

#[cfg(test)]
mod tests;

pub use activation::{ActivationStamp, ActivationState};
pub use authority::{Clock, SystemClock, TimeAuthority, TimeEpoch, TimeSnapshot};
pub use error::TimeError;
pub use mapping::{SampleMapping, TimeMapping};
pub use rate::{PlaybackRate, RationalRate, Rounding};
pub use value::{EngineSamplePosition, MonotonicTime, ShowPosition};
