use std::time::{Duration, Instant};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

/// Machine-wide monotonic coordinates shared with independent qualification processes.
#[derive(Clone, Copy)]
pub(crate) struct Clock {
    pub(crate) frequency: Option<u64>,
}

impl Clock {
    pub(crate) fn new() -> Self {
        let mut value = 0;
        // SAFETY: writable stack output; QPC frequency is fixed at system startup.
        let frequency = unsafe { QueryPerformanceFrequency(&mut value) }
            .ok()
            .and_then(|()| u64::try_from(value).ok())
            .filter(|value| *value != 0);
        Self { frequency }
    }

    pub(crate) fn stamp(self) -> Option<u64> {
        let mut value = 0;
        // SAFETY: writable stack output, no native ownership leaves this call.
        unsafe { QueryPerformanceCounter(&mut value) }
            .ok()
            .and_then(|()| u64::try_from(value).ok())
    }

    /// Conservative QPC lower endpoint for a prior Windows Instant (also QPC-backed).
    /// Read QPC before the Instant anchor. Round elapsed nanoseconds/ticks outward;
    /// preemption only moves the bound earlier, never toward a false low age.
    pub(crate) fn lower_bound(self, published: Instant) -> Option<u64> {
        let anchor = self.stamp()?;
        let elapsed = Instant::now().checked_duration_since(published)?;
        earliest_qpc(anchor, elapsed, self.frequency?)
    }
}

fn earliest_qpc(anchor: u64, elapsed: Duration, frequency: u64) -> Option<u64> {
    if frequency == 0 {
        return None;
    }
    // Instant subtraction truncates to nanoseconds. Include that quantum and one
    // QPC tick rather than treating a rounded duration as an exact coordinate.
    let ticks = elapsed
        .as_nanos()
        .checked_add(1)?
        .checked_mul(u128::from(frequency))?
        .div_ceil(1_000_000_000)
        .checked_add(1)?;
    anchor.checked_sub(u64::try_from(ticks).ok()?)
}

#[cfg(test)]
mod tests;
