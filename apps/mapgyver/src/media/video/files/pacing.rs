//! File-owner polling cadence. Decode/conversion consumes the turn's time budget.
use std::time::{Duration, Instant};

pub(super) fn deadline(turn_started: Instant, playing: bool, pending: bool) -> Instant {
    let interval = if pending {
        Duration::from_millis(1)
    } else if playing {
        Duration::from_millis(8)
    } else {
        Duration::from_secs(3600)
    };
    // An overrun has no extra sleep; WorkerContext still checks control/cancellation first.
    turn_started + interval
}

#[cfg(test)]
mod tests;
