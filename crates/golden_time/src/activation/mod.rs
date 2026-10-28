use std::time::Duration;

use crate::{MonotonicTime, TimeEpoch, TimeError, TimeSnapshot};

/// Scheduled activation in one authority/discontinuity epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivationStamp {
    epoch: TimeEpoch,
    at: MonotonicTime,
}

impl ActivationStamp {
    /// Declares a target coordinate and its required authority epoch.
    pub const fn new(epoch: TimeEpoch, at: MonotonicTime) -> Self {
        Self { epoch, at }
    }

    /// Epoch that must still be current when this work activates.
    pub const fn epoch(self) -> TimeEpoch {
        self.epoch
    }

    /// Monotonic target coordinate within that authority.
    pub const fn at(self) -> MonotonicTime {
        self.at
    }

    /// Rejects stale work and reports timing without selecting late-admission policy.
    pub fn check(self, now: TimeSnapshot) -> Result<ActivationState, TimeError> {
        if self.epoch != now.epoch() {
            return Err(TimeError::StaleEpoch);
        }
        if now.monotonic() < self.at {
            Ok(ActivationState::Pending {
                remaining: self.at.duration_since(now.monotonic())?,
            })
        } else {
            Ok(ActivationState::Due {
                lateness: now.monotonic().duration_since(self.at)?,
            })
        }
    }
}

/// Timing observation for work whose epoch is current.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationState {
    /// The target has not arrived.
    Pending {
        /// Time remaining until the target.
        remaining: Duration,
    },
    /// The target has arrived; the runtime owns ready/late activation policy.
    Due {
        /// Elapsed time after the target, zero when observed exactly at it.
        lateness: Duration,
    },
}

#[cfg(test)]
mod tests;
