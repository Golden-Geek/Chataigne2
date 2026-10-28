use crate::tests::ManualClock;
use crate::{
    EngineSamplePosition, MonotonicTime, PlaybackRate, RationalRate, Rounding, SampleMapping, ShowPosition,
    TimeAuthority, TimeError, TimeMapping,
};

fn mapping(monotonic: u64, show: i64, numerator: i32, denominator: u32) -> TimeMapping {
    let clock = ManualClock::new(monotonic);
    TimeAuthority::new(
        clock,
        ShowPosition::from_nanos(show),
        PlaybackRate::new(numerator, denominator).unwrap(),
    )
    .unwrap()
    .sample()
    .unwrap()
    .mapping()
}

fn time(nanos: u64) -> MonotonicTime {
    MonotonicTime::from_nanos(nanos)
}

fn show(nanos: i64) -> ShowPosition {
    ShowPosition::from_nanos(nanos)
}

fn sample(samples: u64) -> EngineSamplePosition {
    EngineSamplePosition::from_samples(samples)
}

#[test]
fn show_mapping_uses_its_original_anchor_and_signed_floor_for_fractional_reverse_rates() {
    let forward = mapping(10, -20, 1, 3);
    for (elapsed, position) in [(0, -20), (1, -20), (2, -20), (3, -19), (4, -19), (1_000_000, 333_313)] {
        assert_eq!(forward.show_at(time(10 + elapsed)), Ok(show(position)));
    }
    assert_eq!(forward.monotonic_origin(), time(10));
    assert_eq!(forward.show_origin(), show(-20));

    let reverse = mapping(10, 0, -1, 3);
    for (elapsed, position) in [(0, 0), (1, -1), (2, -1), (3, -1), (4, -2), (1_000_000, -333_334)] {
        assert_eq!(reverse.show_at(time(10 + elapsed)), Ok(show(position)));
    }
}

#[test]
fn inverse_show_mapping_quantizes_the_monotonic_deadline_in_both_directions() {
    let forward = mapping(100, 10, 3, 2);
    assert_eq!(forward.monotonic_at(show(11), Rounding::Floor), Ok(time(100)));
    assert_eq!(forward.monotonic_at(show(11), Rounding::Ceil), Ok(time(101)));
    assert_eq!(forward.monotonic_at(show(12), Rounding::Floor), Ok(time(101)));
    assert_eq!(forward.monotonic_at(show(12), Rounding::Ceil), Ok(time(102)));

    let reverse = mapping(100, 10, -3, 2);
    assert_eq!(reverse.monotonic_at(show(9), Rounding::Floor), Ok(time(100)));
    assert_eq!(reverse.monotonic_at(show(9), Rounding::Ceil), Ok(time(101)));
    assert_eq!(reverse.monotonic_at(show(8), Rounding::Floor), Ok(time(101)));
    assert_eq!(reverse.monotonic_at(show(8), Rounding::Ceil), Ok(time(102)));
}

#[test]
fn show_positions_before_the_epoch_anchor_reject_before_any_deadline_rounding() {
    let forward = mapping(100, 10, 2, 1);
    let reverse = mapping(100, 10, -2, 1);
    for rounding in [Rounding::Floor, Rounding::Ceil] {
        assert_eq!(forward.monotonic_at(show(9), rounding), Err(TimeError::BeforeAnchor));
        assert_eq!(reverse.monotonic_at(show(11), rounding), Err(TimeError::BeforeAnchor));
    }
    assert_eq!(forward.show_at(time(99)), Err(TimeError::BeforeAnchor));
    assert_eq!(reverse.show_at(time(99)), Err(TimeError::BeforeAnchor));
}

#[test]
fn paused_show_positions_remain_constant_and_cannot_choose_an_inverse_deadline() {
    let paused = mapping(15, -500, 0, u32::MAX);
    assert_eq!(paused.rate(), PlaybackRate::PAUSED);
    for now in [15, 16, 1_000_000_000, u64::MAX] {
        assert_eq!(paused.show_at(time(now)), Ok(show(-500)));
    }
    for position in [-501, -500, -499] {
        assert_eq!(
            paused.monotonic_at(show(position), Rounding::Ceil),
            Err(TimeError::PausedMapping)
        );
    }
}

#[test]
fn show_and_monotonic_conversion_overflow_is_explicit() {
    assert_eq!(mapping(0, i64::MAX, 1, 1).show_at(time(1)), Err(TimeError::Overflow));
    assert_eq!(mapping(0, i64::MIN, -1, 1).show_at(time(1)), Err(TimeError::Overflow));
    assert_eq!(
        mapping(0, 0, i32::MAX, 1).show_at(time(u64::MAX)),
        Err(TimeError::Overflow)
    );
    assert_eq!(
        mapping(u64::MAX, 0, 1, 1).monotonic_at(show(1), Rounding::Ceil),
        Err(TimeError::Overflow)
    );
    assert_eq!(
        mapping(0, 0, 1, u32::MAX).monotonic_at(show(i64::MAX), Rounding::Floor),
        Err(TimeError::Overflow)
    );
    assert_eq!(
        mapping(0, 0, i32::MIN, u32::MAX).show_at(time(u64::from(u32::MAX))),
        Ok(show(i64::from(i32::MIN)))
    );
}

#[test]
fn engine_sample_mapping_keeps_both_anchors_and_explicit_quantization() {
    let mapping = SampleMapping::new(time(1_000), sample(123), RationalRate::new(48_000, 1).unwrap());
    assert_eq!(mapping.sample_at(time(1_000), Rounding::Floor), Ok(sample(123)));
    assert_eq!(mapping.sample_at(time(1_001), Rounding::Floor), Ok(sample(123)));
    assert_eq!(mapping.sample_at(time(1_001), Rounding::Ceil), Ok(sample(124)));
    assert_eq!(
        mapping.sample_at(time(1_000_001_000), Rounding::Floor),
        Ok(sample(48_123))
    );
    assert_eq!(
        mapping.monotonic_at(sample(48_123), Rounding::Floor),
        Ok(time(1_000_001_000))
    );
    assert_eq!(mapping.monotonic_at(sample(124), Rounding::Floor), Ok(time(21_833)));
    assert_eq!(mapping.monotonic_at(sample(124), Rounding::Ceil), Ok(time(21_834)));
    assert_eq!(
        mapping.sample_at(time(999), Rounding::Floor),
        Err(TimeError::BeforeAnchor)
    );
    assert_eq!(
        mapping.monotonic_at(sample(122), Rounding::Ceil),
        Err(TimeError::BeforeAnchor)
    );
}

#[test]
fn engine_sample_and_monotonic_overflow_never_saturate_into_a_valid_activation() {
    let one_hz = RationalRate::new(1, 1).unwrap();
    assert_eq!(
        SampleMapping::new(time(0), sample(u64::MAX), one_hz).sample_at(time(1_000_000_000), Rounding::Floor),
        Err(TimeError::Overflow)
    );
    assert_eq!(
        SampleMapping::new(time(u64::MAX), sample(0), one_hz).monotonic_at(sample(1), Rounding::Floor),
        Err(TimeError::Overflow)
    );
    let very_slow = RationalRate::new(1, u32::MAX).unwrap();
    assert_eq!(
        SampleMapping::new(time(0), sample(0), very_slow).monotonic_at(sample(u64::MAX), Rounding::Ceil),
        Err(TimeError::Overflow)
    );
}

#[test]
fn show_seek_and_pause_do_not_reset_the_independent_engine_sample_timeline() {
    let clock = ManualClock::new(0);
    let mut authority = TimeAuthority::new(clock.clone(), show(0), PlaybackRate::NORMAL).unwrap();
    let samples = SampleMapping::new(time(0), sample(1_000), RationalRate::new(48_000, 1).unwrap());
    clock.set(1_000_000_000);
    let before = authority.sample().unwrap();
    assert_eq!(before.show_position(), show(1_000_000_000));
    assert_eq!(
        samples.sample_at(before.monotonic(), Rounding::Floor),
        Ok(sample(49_000))
    );

    let seek = authority
        .discontinuity(show(-5_000_000_000), PlaybackRate::PAUSED)
        .unwrap();
    clock.set(2_000_000_000);
    let after = authority.sample().unwrap();
    assert_ne!(before.epoch(), seek.epoch());
    assert_eq!(seek.epoch(), after.epoch());
    assert_eq!(after.show_position(), show(-5_000_000_000));
    assert_eq!(
        samples.sample_at(after.monotonic(), Rounding::Floor),
        Ok(sample(97_000))
    );
    assert_eq!(
        samples.monotonic_at(sample(97_000), Rounding::Ceil),
        Ok(time(2_000_000_000))
    );
}
