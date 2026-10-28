use std::time::Duration;

use crate::tests::ManualClock;
use crate::{ActivationStamp, ActivationState, MonotonicTime, PlaybackRate, ShowPosition, TimeAuthority, TimeError};

#[test]
fn current_epoch_reports_early_exact_and_late_activation_without_selecting_policy() {
    let clock = ManualClock::new(10);
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let initial = authority.sample().unwrap();
    let stamp = ActivationStamp::new(initial.epoch(), MonotonicTime::from_nanos(20));
    assert_eq!(stamp.epoch(), initial.epoch());
    assert_eq!(stamp.at().as_nanos(), 20);
    assert_eq!(
        stamp.check(initial),
        Ok(ActivationState::Pending {
            remaining: Duration::from_nanos(10)
        })
    );
    clock.set(20);
    assert_eq!(
        stamp.check(authority.sample().unwrap()),
        Ok(ActivationState::Due {
            lateness: Duration::ZERO
        })
    );
    clock.set(25);
    assert_eq!(
        stamp.check(authority.sample().unwrap()),
        Ok(ActivationState::Due {
            lateness: Duration::from_nanos(5)
        })
    );
}

#[test]
fn discontinuity_rejects_old_work_before_considering_its_deadline() {
    let clock = ManualClock::new(0);
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let old = ActivationStamp::new(authority.sample().unwrap().epoch(), MonotonicTime::from_nanos(100));
    clock.set(10);
    let changed = authority
        .discontinuity(ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap();
    assert_eq!(old.check(changed), Err(TimeError::StaleEpoch));
}

#[test]
fn authority_replacement_rejects_old_work_even_if_both_generations_are_zero() {
    let clock = ManualClock::new(0);
    let mut first = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let old = ActivationStamp::new(first.sample().unwrap().epoch(), MonotonicTime::from_nanos(0));
    let mut replacement = TimeAuthority::new(clock, ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    assert_eq!(old.check(replacement.sample().unwrap()), Err(TimeError::StaleEpoch));
}

#[test]
fn paused_show_position_does_not_pause_monotonic_activation_observation() {
    let clock = ManualClock::new(0);
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(50), PlaybackRate::PAUSED).unwrap();
    let stamp = ActivationStamp::new(authority.sample().unwrap().epoch(), MonotonicTime::from_nanos(10));
    clock.set(12);
    let now = authority.sample().unwrap();
    assert_eq!(now.show_position().as_nanos(), 50);
    assert_eq!(
        stamp.check(now),
        Ok(ActivationState::Due {
            lateness: Duration::from_nanos(2)
        })
    );
}

#[test]
fn activation_differences_cover_the_full_monotonic_representation_without_overflow() {
    let clock = ManualClock::new(0);
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::PAUSED).unwrap();
    let start = authority.sample().unwrap();
    let latest = ActivationStamp::new(start.epoch(), MonotonicTime::from_nanos(u64::MAX));
    assert_eq!(
        latest.check(start),
        Ok(ActivationState::Pending {
            remaining: Duration::from_nanos(u64::MAX)
        })
    );
    let earliest = ActivationStamp::new(start.epoch(), MonotonicTime::from_nanos(0));
    clock.set(u64::MAX);
    assert_eq!(
        earliest.check(authority.sample().unwrap()),
        Ok(ActivationState::Due {
            lateness: Duration::from_nanos(u64::MAX)
        })
    );
}
