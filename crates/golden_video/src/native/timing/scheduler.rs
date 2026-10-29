use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use golden_time::{Clock as TimeClock, MonotonicTime, SystemClock, TimeEpoch, TimeError};

use super::{Clock, ScheduleClock, ScheduleClockError};
use crate::runtime::FrameSchedule;

const NANOS: u128 = 1_000_000_000;

#[derive(Clone, Copy)]
struct Mapping {
    origin: u64,
    frequency: u64,
}

impl Mapping {
    fn nanos(self, tick: u64) -> Result<u64, ScheduleClockError> {
        if self.frequency == 0 {
            return Err(ScheduleClockError::InvalidFrequency);
        }
        let elapsed = tick.checked_sub(self.origin).ok_or(ScheduleClockError::BeforeOrigin)?;
        u64::try_from(u128::from(elapsed) * NANOS / u128::from(self.frequency))
            .map_err(|_| ScheduleClockError::Overflow)
    }
    fn counter(self, nanos: u64) -> Result<u64, ScheduleClockError> {
        if self.frequency == 0 {
            return Err(ScheduleClockError::InvalidFrequency);
        }
        let ticks = (u128::from(nanos) * u128::from(self.frequency)).div_ceil(NANOS);
        let ticks = u64::try_from(ticks).map_err(|_| ScheduleClockError::Overflow)?;
        self.origin.checked_add(ticks).ok_or(ScheduleClockError::Overflow)
    }
    fn project(self, schedule: FrameSchedule) -> Result<ScheduleClock, ScheduleClockError> {
        let observed = schedule.observed.monotonic().as_nanos();
        let lower = self.counter(observed)?;
        let upper = self
            .counter(observed.checked_add(1).ok_or(ScheduleClockError::Overflow)?)?
            .checked_sub(1)
            .ok_or(ScheduleClockError::InvalidRange)?;
        if lower > upper {
            return Err(ScheduleClockError::InvalidRange);
        }
        Ok(ScheduleClock::Qpc {
            origin_qpc: self.origin,
            frequency: self.frequency,
            anchor_qpc: self.counter(schedule.anchor.as_nanos())?,
            slot_start_qpc: self.counter(schedule.slot_start.as_nanos())?,
            slot_end_qpc: self.counter(schedule.slot_end.as_nanos())?,
            observed_qpc_lower: lower,
            observed_qpc_upper: upper,
        })
    }
}

#[derive(Clone)]
pub(crate) struct SchedulerClock {
    counter: Clock,
    mapping: Mapping,
    failure: Arc<AtomicU8>,
}

impl TimeClock for SchedulerClock {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        let result = self
            .counter
            .stamp()
            .ok_or(ScheduleClockError::ClockUnavailable)
            .and_then(|tick| self.mapping.nanos(tick));
        result.map(MonotonicTime::from_nanos).map_err(|error| {
            self.failure.store(
                match error {
                    ScheduleClockError::BeforeOrigin => 2,
                    ScheduleClockError::Overflow => 3,
                    _ => 1,
                },
                Ordering::Release,
            );
            TimeError::ClockUnavailable
        })
    }
}

/// Opt-in explicit clock source. Default/off never initializes or reads QPC for scheduling.
#[derive(Clone)]
pub(crate) enum ScheduleClockSource {
    Qpc(SchedulerClock),
    Unavailable(ScheduleClockError),
}

impl ScheduleClockSource {
    pub(crate) fn new(enabled: bool) -> Self {
        if !enabled {
            return Self::Unavailable(ScheduleClockError::InitializationUnavailable);
        }
        let counter = Clock::new();
        let Some(frequency) = counter.frequency else {
            return Self::Unavailable(ScheduleClockError::InitializationUnavailable);
        };
        let Some(origin) = counter.stamp() else {
            return Self::Unavailable(ScheduleClockError::ClockUnavailable);
        };
        Self::Qpc(SchedulerClock {
            counter,
            mapping: Mapping { origin, frequency },
            failure: Arc::new(AtomicU8::new(0)),
        })
    }
    pub(crate) fn clock(&self) -> Arc<dyn TimeClock> {
        match self {
            Self::Qpc(clock) => Arc::new(clock.clone()),
            Self::Unavailable(_) => Arc::new(SystemClock::new()),
        }
    }
    pub(crate) fn project(&self, schedule: FrameSchedule, original: TimeEpoch) -> ScheduleClock {
        let result = match self {
            Self::Unavailable(error) => Err(*error),
            Self::Qpc(clock) => {
                if !schedule.observed.epoch().same_authority(original) {
                    Err(ScheduleClockError::AuthorityChanged)
                } else {
                    match clock.failure.load(Ordering::Acquire) {
                        0 => clock.mapping.project(schedule),
                        2 => Err(ScheduleClockError::BeforeOrigin),
                        3 => Err(ScheduleClockError::Overflow),
                        _ => Err(ScheduleClockError::ClockUnavailable),
                    }
                }
            }
        };
        result.unwrap_or_else(|reason| ScheduleClock::Unavailable { reason })
    }
}

#[cfg(test)]
mod tests;
