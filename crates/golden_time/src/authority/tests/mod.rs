use std::sync::Arc;

use super::*;
use crate::tests::ManualClock;

#[test]
fn repeated_consumers_observe_one_anchor_without_duplicate_advancement() {
    let clock = ManualClock::new(100);
    let mut authority =
        TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(-500), PlaybackRate::NORMAL).unwrap();
    clock.set(150);
    let first = authority.sample().unwrap();
    assert_eq!(first.monotonic().as_nanos(), 150);
    assert_eq!(first.show_position().as_nanos(), -450);
    assert_eq!(authority.sample().unwrap(), first);
    assert_eq!(authority.sample().unwrap(), first);
}

#[test]
fn fractional_observations_preserve_original_phase_instead_of_accumulating_truncation() {
    let clock = ManualClock::new(0);
    let mut authority = TimeAuthority::new(
        clock.clone(),
        ShowPosition::from_nanos(0),
        PlaybackRate::new(1, 3).unwrap(),
    )
    .unwrap();
    for nanos in 1..=30 {
        clock.set(nanos);
        let snapshot = authority.sample().unwrap();
        assert_eq!(snapshot.show_position().as_nanos(), (nanos / 3) as i64);
        assert_eq!(snapshot.mapping().monotonic_origin().as_nanos(), 0);
    }
}

#[test]
fn regression_in_sampling_or_discontinuity_leaves_accepted_state_intact() {
    let clock = ManualClock::new(10);
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    clock.set(20);
    let prior = authority.sample().unwrap();
    clock.set(19);
    let error = TimeError::ClockRegressed {
        previous: MonotonicTime::from_nanos(20),
        observed: MonotonicTime::from_nanos(19),
    };
    assert_eq!(authority.sample(), Err(error));
    assert_eq!(
        authority.discontinuity(ShowPosition::from_nanos(99), PlaybackRate::PAUSED),
        Err(error)
    );
    assert_eq!(authority.last, prior.monotonic());
    assert_eq!(authority.mapping, prior.mapping());
    clock.set(21);
    assert_eq!(authority.sample().unwrap().show_position().as_nanos(), 11);
}

#[test]
fn failed_clock_observation_does_not_publish_a_new_anchor_or_epoch() {
    let clock = ManualClock::new(10);
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(5), PlaybackRate::NORMAL).unwrap();
    let prior = authority.sample().unwrap();
    clock.fail(true);
    assert_eq!(authority.sample(), Err(TimeError::Overflow));
    assert_eq!(
        authority.discontinuity(ShowPosition::from_nanos(80), PlaybackRate::PAUSED),
        Err(TimeError::Overflow)
    );
    assert_eq!(authority.last, prior.monotonic());
    assert_eq!(authority.mapping, prior.mapping());
    clock.fail(false);
    clock.set(12);
    assert_eq!(authority.sample().unwrap().show_position().as_nanos(), 7);
}

#[test]
fn show_overflow_does_not_commit_and_an_explicit_discontinuity_can_recover() {
    let clock = ManualClock::new(0);
    let mut authority =
        TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(i64::MAX), PlaybackRate::NORMAL).unwrap();
    let prior = authority.sample().unwrap();
    clock.set(1);
    assert_eq!(authority.sample(), Err(TimeError::Overflow));
    assert_eq!(authority.last, prior.monotonic());
    assert_eq!(authority.mapping, prior.mapping());
    let reset = authority
        .discontinuity(ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap();
    assert_eq!(reset.epoch().generation(), 1);
    clock.set(2);
    assert_eq!(authority.sample().unwrap().show_position().as_nanos(), 1);
}

#[test]
fn explicit_transport_discontinuities_fence_pause_seek_and_reverse_without_resetting_monotonic_time() {
    let clock = ManualClock::new(0);
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let initial = authority.sample().unwrap();
    clock.set(10);
    let paused = authority
        .discontinuity(ShowPosition::from_nanos(100), PlaybackRate::PAUSED)
        .unwrap();
    assert_ne!(paused.epoch(), initial.epoch());
    assert!(paused.epoch().same_authority(initial.epoch()));
    assert_eq!(paused.epoch().generation(), 1);
    clock.set(12);
    assert_eq!(authority.sample().unwrap().show_position().as_nanos(), 100);
    let reversed = authority
        .discontinuity(ShowPosition::from_nanos(100), PlaybackRate::new(-1, 1).unwrap())
        .unwrap();
    clock.set(14);
    let now = authority.sample().unwrap();
    assert_eq!(now.monotonic().as_nanos(), 14);
    assert_eq!(now.show_position().as_nanos(), 98);
    assert_eq!(reversed.epoch().generation(), 2);
}

#[test]
fn replacement_authorities_do_not_reuse_an_epoch_even_with_identical_generation_and_coordinates() {
    let clock = ManualClock::new(0);
    let mut first = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let mut second = TimeAuthority::new(clock, ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let first = first.sample().unwrap();
    let second = second.sample().unwrap();
    assert_eq!(first.epoch().generation(), second.epoch().generation());
    assert!(!first.epoch().same_authority(second.epoch()));
    assert_ne!(first.epoch(), second.epoch());
}

#[test]
fn epoch_exhaustion_does_not_wrap_or_partially_commit_a_new_mapping() {
    let clock = ManualClock::new(0);
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let exhausted = TimeEpoch {
        authority: authority.mapping.epoch().authority,
        generation: u64::MAX,
    };
    authority.mapping = TimeMapping::new(
        exhausted,
        MonotonicTime::from_nanos(0),
        ShowPosition::from_nanos(0),
        PlaybackRate::NORMAL,
    );
    let prior = authority.sample().unwrap();
    clock.set(10);
    assert_eq!(
        authority.discontinuity(ShowPosition::from_nanos(80), PlaybackRate::PAUSED),
        Err(TimeError::Overflow)
    );
    assert_eq!(authority.last, prior.monotonic());
    assert_eq!(authority.mapping, prior.mapping());
    assert_eq!(authority.sample().unwrap().show_position().as_nanos(), 10);
}

#[test]
fn production_clock_and_shared_trait_object_use_the_same_authority_contract() {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::default());
    let mut authority = TimeAuthority::new(clock, ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let first = authority.sample().unwrap();
    let second = authority.sample().unwrap();
    assert!(second.monotonic() >= first.monotonic());
    assert!(second.show_position() >= first.show_position());
    assert_eq!(second.epoch(), first.epoch());
}

#[test]
fn reader_projection_preserves_epoch_original_phase_and_authority_state() {
    let clock = ManualClock::new(100);
    let mut authority = TimeAuthority::new(
        clock.clone(),
        ShowPosition::from_nanos(-20),
        PlaybackRate::new(1, 3).unwrap(),
    )
    .unwrap();
    clock.set(102);
    let snapshot = authority.sample().unwrap();
    let projected = snapshot.at_monotonic(MonotonicTime::from_nanos(103)).unwrap();
    assert_eq!(snapshot.show_position().as_nanos(), -20);
    assert_eq!(projected.show_position().as_nanos(), -19);
    assert_eq!(projected.epoch(), snapshot.epoch());
    assert_eq!(projected.mapping(), snapshot.mapping());
    assert_eq!(projected.mapping().monotonic_origin().as_nanos(), 100);
    assert_eq!(authority.sample().unwrap(), snapshot);
}

#[test]
fn reader_regression_or_overflow_leaves_the_snapshot_unchanged() {
    let clock = ManualClock::new(100);
    let mut authority =
        TimeAuthority::new(clock, ShowPosition::from_nanos(i64::MAX - 1), PlaybackRate::NORMAL).unwrap();
    let snapshot = authority.sample().unwrap();
    assert_eq!(
        snapshot.at_monotonic(MonotonicTime::from_nanos(99)),
        Err(TimeError::ClockRegressed {
            previous: snapshot.monotonic(),
            observed: MonotonicTime::from_nanos(99)
        })
    );
    assert_eq!(
        snapshot.at_monotonic(MonotonicTime::from_nanos(102)),
        Err(TimeError::Overflow)
    );
    assert_eq!(snapshot, authority.sample().unwrap());
}
