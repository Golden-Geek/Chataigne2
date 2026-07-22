use std::time::{Duration, Instant};

/// Deterministic capped exponential backoff for reconnecting IO workers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconnectBackoff {
    initial_delay: Duration,
    maximum_delay: Duration,
    next_delay: Duration,
}

impl ReconnectBackoff {
    pub fn new(initial_delay: Duration, maximum_delay: Duration) -> Self {
        assert!(!initial_delay.is_zero(), "reconnect delay must be non-zero");
        assert!(
            initial_delay <= maximum_delay,
            "reconnect maximum must not be shorter than its initial delay"
        );
        Self {
            initial_delay,
            maximum_delay,
            next_delay: initial_delay,
        }
    }

    /// Schedules the next attempt and advances the delay for a subsequent failure.
    pub fn schedule(&mut self, now: Instant) -> Instant {
        let retry_at = now + self.next_delay;
        self.next_delay = self.next_delay.saturating_mul(2).min(self.maximum_delay);
        retry_at
    }

    /// Restores the initial delay after a successful connection.
    pub fn reset(&mut self) {
        self.next_delay = self.initial_delay;
    }

    pub fn next_delay(&self) -> Duration {
        self.next_delay
    }
}
