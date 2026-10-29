use golden_time::{PlaybackRate, RationalRate, ShowPosition, TimeAuthority, TimeSnapshot};
use uuid::Uuid;

use super::*;

#[derive(Clone)]
struct Fixed(u64);
impl TimeClock for Fixed {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        Ok(MonotonicTime::from_nanos(self.0))
    }
}

fn schedule(observed: TimeSnapshot) -> FrameSchedule {
    FrameSchedule {
        cadence: Uuid::new_v4(),
        anchor: MonotonicTime::from_nanos(0),
        rate: RationalRate::new(60, 1).unwrap(),
        frame_index: 0,
        slot_start: MonotonicTime::from_nanos(0),
        slot_end: MonotonicTime::from_nanos(16_666_667),
        observed,
        coalesced_slots: 0,
        skipped_slots: 0,
    }
}

#[test]
fn exact_counter_inverse_never_chooses_an_early_tick_and_preserves_quantization_bounds() {
    for frequency in [1, 10_000_000, 1_000_000_000, 3_000_000_000] {
        let mapping = Mapping {
            origin: 1000,
            frequency,
        };
        for deadline in [0, 1, 99, 100, 101, 16_666_667, 1_000_000_001] {
            let tick = mapping.counter(deadline).unwrap();
            assert!(mapping.nanos(tick).unwrap() >= deadline);
            if tick > mapping.origin {
                assert!(mapping.nanos(tick - 1).unwrap() < deadline);
            }
        }
    }
    let mut authority = TimeAuthority::new(Fixed(1), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let frame = schedule(authority.sample().unwrap());
    let mapped = Mapping {
        origin: 1000,
        frequency: 3_000_000_000,
    }
    .project(frame)
    .unwrap();
    assert!(matches!(
        mapped,
        ScheduleClock::Qpc {
            observed_qpc_lower: 1003,
            observed_qpc_upper: 1005,
            ..
        }
    ));
    assert_eq!(
        Mapping {
            origin: 1000,
            frequency: 10_000_000
        }
        .project(frame),
        Err(ScheduleClockError::InvalidRange)
    );
}

#[test]
fn negative_ranges_zero_frequency_and_overflow_never_fabricate_counter_coordinates() {
    assert_eq!(
        Mapping {
            origin: 1000,
            frequency: 1
        }
        .nanos(999),
        Err(ScheduleClockError::BeforeOrigin)
    );
    assert_eq!(
        Mapping {
            origin: 0,
            frequency: 0
        }
        .counter(1),
        Err(ScheduleClockError::InvalidFrequency)
    );
    assert_eq!(
        Mapping {
            origin: 0,
            frequency: 0
        }
        .nanos(1),
        Err(ScheduleClockError::InvalidFrequency)
    );
    assert_eq!(
        Mapping {
            origin: u64::MAX,
            frequency: 1_000_000_000
        }
        .counter(1),
        Err(ScheduleClockError::Overflow)
    );
    assert_eq!(
        Mapping {
            origin: 0,
            frequency: 1
        }
        .nanos(u64::MAX),
        Err(ScheduleClockError::Overflow)
    );
}

#[test]
fn replacement_authority_and_read_failure_invalidate_mapping_while_show_discontinuity_does_not() {
    let clock = SchedulerClock {
        counter: Clock {
            frequency: Some(10_000_000),
        },
        mapping: Mapping {
            origin: 1000,
            frequency: 10_000_000,
        },
        failure: Arc::new(AtomicU8::new(0)),
    };
    let source = ScheduleClockSource::Qpc(clock.clone());
    let mut original = TimeAuthority::new(Fixed(100), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let first = original.sample().unwrap();
    assert!(matches!(
        source.project(schedule(first), first.epoch()),
        ScheduleClock::Qpc { .. }
    ));
    let seek = original
        .discontinuity(ShowPosition::from_nanos(900), PlaybackRate::PAUSED)
        .unwrap();
    assert!(matches!(
        source.project(schedule(seek), first.epoch()),
        ScheduleClock::Qpc { .. }
    ));
    let mut replacement = TimeAuthority::new(Fixed(100), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let replacement = replacement.sample().unwrap();
    assert_eq!(first.epoch().generation(), replacement.epoch().generation());
    assert_eq!(
        source.project(schedule(replacement), first.epoch()),
        ScheduleClock::Unavailable {
            reason: ScheduleClockError::AuthorityChanged
        }
    );
    clock.failure.store(1, Ordering::Release);
    assert_eq!(
        source.project(schedule(first), first.epoch()),
        ScheduleClock::Unavailable {
            reason: ScheduleClockError::ClockUnavailable
        }
    );
    assert_eq!(
        ScheduleClockSource::new(false).project(schedule(first), first.epoch()),
        ScheduleClock::Unavailable {
            reason: ScheduleClockError::InitializationUnavailable
        }
    );
}
