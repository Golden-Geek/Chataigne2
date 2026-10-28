use std::time::Duration;

use super::*;

fn at(nanos: u64) -> MonotonicTime {
    MonotonicTime::from_nanos(nanos)
}

#[test]
fn the_anchor_is_immediately_due_and_repeated_observation_cannot_advance_it_twice() {
    let mut cadence = Cadence::new(RationalRate::new(60, 1).unwrap(), at(100));
    assert_eq!(cadence.deadline(), Ok(at(100)));
    assert_eq!(
        cadence.consume_due(at(100)),
        Ok(Some(CadenceTick {
            index: 0,
            scheduled: at(100),
            missed: 0
        }))
    );
    assert_eq!(cadence.consume_due(at(100)), Ok(None));
    assert_eq!(cadence.consume_due(at(101)), Ok(None));
    assert_eq!(cadence.deadline(), Ok(at(16_666_767)));
}

#[test]
fn rational_deadlines_keep_absolute_phase_over_multiple_hours_and_never_fire_early() {
    const INDEX: u64 = 2_160_000;
    const EXACT_NANOS: u64 = 36_036_000_000_000;
    let anchor = at(123_456_789);
    let mut cadence = Cadence::new(RationalRate::new(60_000, 1_001).unwrap(), anchor);
    cadence.consume_due(anchor).unwrap().unwrap();
    let now = anchor.checked_add(Duration::from_nanos(EXACT_NANOS)).unwrap();
    assert_eq!(
        cadence.consume_due(now),
        Ok(Some(CadenceTick {
            index: INDEX,
            scheduled: now,
            missed: INDEX - 1
        }))
    );
    let exact_next = anchor
        .checked_add(Duration::from_nanos(EXACT_NANOS + 16_683_334))
        .unwrap();
    assert_eq!(cadence.deadline(), Ok(exact_next));
    assert_eq!(cadence.consume_due(at(exact_next.as_nanos() - 1)), Ok(None));
    assert_eq!(
        cadence.consume_due(exact_next),
        Ok(Some(CadenceTick {
            index: INDEX + 1,
            scheduled: exact_next,
            missed: 0
        }))
    );
    assert_eq!(cadence.consume_due(exact_next), Ok(None));
}

#[test]
fn a_large_time_jump_yields_one_latest_tick_and_an_exact_missed_count() {
    let mut cadence = Cadence::new(RationalRate::new(60, 1).unwrap(), at(0));
    let now = at(1_000_000_000);
    assert_eq!(
        cadence.consume_due(now),
        Ok(Some(CadenceTick {
            index: 60,
            scheduled: now,
            missed: 60
        }))
    );
    assert_eq!(cadence.consume_due(now), Ok(None));
    assert_eq!(cadence.deadline(), Ok(at(1_016_666_667)));
}

#[test]
fn a_coordinate_before_the_anchor_is_rejected_without_consuming_a_tick() {
    let mut cadence = Cadence::new(RationalRate::new(60, 1).unwrap(), at(100));
    assert_eq!(cadence.consume_due(at(99)), Err(TimeError::BeforeAnchor));
    assert_eq!(cadence.deadline(), Ok(at(100)));
    assert_eq!(cadence.consume_due(at(100)).unwrap().unwrap().index, 0);
}

#[test]
fn frame_counter_exhaustion_returns_an_error_without_wrapping_or_committing() {
    let mut cadence = Cadence::new(RationalRate::new(1_000_000_000, 1).unwrap(), at(0));
    assert_eq!(cadence.consume_due(at(u64::MAX)), Err(TimeError::Overflow));
    assert_eq!(cadence.deadline(), Ok(at(0)));
    assert_eq!(cadence.next_index, 0);
}

#[test]
fn the_last_representable_deadline_does_not_wrap_into_an_early_future_tick() {
    let mut cadence = Cadence::new(RationalRate::new(1_000_000_000, 1).unwrap(), at(u64::MAX - 1));
    assert_eq!(cadence.consume_due(at(u64::MAX - 1)).unwrap().unwrap().index, 0);
    assert_eq!(cadence.deadline(), Ok(at(u64::MAX)));
    assert_eq!(cadence.consume_due(at(u64::MAX)).unwrap().unwrap().index, 1);
    assert_eq!(cadence.deadline(), Err(TimeError::Overflow));
    let prior = cadence.next_index;
    assert_eq!(cadence.consume_due(at(u64::MAX)), Err(TimeError::Overflow));
    assert_eq!(cadence.next_index, prior);
}

#[test]
fn subnanosecond_rates_coalesce_ordinals_that_share_a_representable_deadline() {
    let mut cadence = Cadence::new(RationalRate::new(4_000_000_000, 1).unwrap(), at(0));
    cadence.consume_due(at(0)).unwrap().unwrap();
    assert_eq!(cadence.deadline(), Ok(at(1)));
    assert_eq!(
        cadence.consume_due(at(1)),
        Ok(Some(CadenceTick {
            index: 4,
            scheduled: at(1),
            missed: 3
        }))
    );
    assert_eq!(cadence.consume_due(at(1)), Ok(None));
    assert_eq!(cadence.deadline(), Ok(at(2)));
}
