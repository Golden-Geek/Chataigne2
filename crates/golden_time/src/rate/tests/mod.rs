use std::time::Duration;

use crate::{PlaybackRate, RationalRate, Rounding, TimeError};

#[test]
fn scheduling_rates_reduce_equivalent_frequencies_and_reject_zero_components() {
    let video = RationalRate::new(120_000, 2_002).unwrap();
    assert_eq!(video, RationalRate::new(60_000, 1_001).unwrap());
    assert_eq!((video.numerator(), video.denominator()), (60_000, 1_001));
    assert_eq!(
        RationalRate::new(u32::MAX, u32::MAX).unwrap(),
        RationalRate::new(1, 1).unwrap()
    );
    assert_eq!(RationalRate::new(0, 1), Err(TimeError::ZeroFrequency));
    assert_eq!(RationalRate::new(1, 0), Err(TimeError::ZeroDenominator));
}

#[test]
fn rational_video_deadlines_use_absolute_tick_counts_without_period_rounding_drift() {
    let rate = RationalRate::new(60_000, 1_001).unwrap();
    let floor_period = rate.duration_for(1, Rounding::Floor).unwrap();
    let ceil_period = rate.duration_for(1, Rounding::Ceil).unwrap();
    assert_eq!(floor_period, Duration::from_nanos(16_683_333));
    assert_eq!(ceil_period, Duration::from_nanos(16_683_334));
    assert_eq!(rate.ticks_for(floor_period, Rounding::Floor), Ok(0));
    assert_eq!(rate.ticks_for(floor_period, Rounding::Ceil), Ok(1));
    assert_eq!(rate.ticks_for(ceil_period, Rounding::Floor), Ok(1));

    let complete_cycle = Duration::from_secs(1_001);
    assert_eq!(rate.duration_for(60_000, Rounding::Floor), Ok(complete_cycle));
    assert_eq!(rate.duration_for(60_000, Rounding::Ceil), Ok(complete_cycle));
    assert_eq!(rate.ticks_for(complete_cycle, Rounding::Floor), Ok(60_000));
    assert_eq!(rate.ticks_for(complete_cycle, Rounding::Ceil), Ok(60_000));
    assert_eq!(complete_cycle - floor_period * 60_000, Duration::from_nanos(20_000));
}

#[test]
fn audio_sample_rate_uses_explicit_quantization_and_exact_whole_seconds() {
    let rate = RationalRate::new(48_000, 1).unwrap();
    assert_eq!(rate.duration_for(1, Rounding::Floor), Ok(Duration::from_nanos(20_833)));
    assert_eq!(rate.duration_for(1, Rounding::Ceil), Ok(Duration::from_nanos(20_834)));
    assert_eq!(rate.ticks_for(Duration::from_secs(2), Rounding::Floor), Ok(96_000));
    assert_eq!(rate.duration_for(96_000, Rounding::Ceil), Ok(Duration::from_secs(2)));
    assert_eq!(rate.duration_for(0, Rounding::Ceil), Ok(Duration::ZERO));
    assert_eq!(rate.ticks_for(Duration::ZERO, Rounding::Ceil), Ok(0));
}

#[test]
fn conversions_use_the_full_duration_range_and_report_unrepresentable_results() {
    let one_hz = RationalRate::new(1, 1).unwrap();
    assert_eq!(
        one_hz.duration_for(u64::MAX, Rounding::Floor),
        Ok(Duration::from_secs(u64::MAX))
    );
    assert_eq!(one_hz.ticks_for(Duration::MAX, Rounding::Floor), Ok(u64::MAX));
    assert_eq!(
        one_hz.ticks_for(Duration::MAX, Rounding::Ceil),
        Err(TimeError::Overflow)
    );

    let very_slow = RationalRate::new(1, u32::MAX).unwrap();
    assert_eq!(
        very_slow.duration_for(u64::MAX, Rounding::Floor),
        Err(TimeError::Overflow)
    );
    let very_fast = RationalRate::new(u32::MAX, 1).unwrap();
    assert_eq!(
        very_fast.ticks_for(Duration::MAX, Rounding::Floor),
        Err(TimeError::Overflow)
    );
    assert_eq!(
        very_fast.ticks_for(Duration::from_secs(1), Rounding::Floor),
        Ok(u64::from(u32::MAX))
    );
}

#[test]
fn playback_rates_preserve_pause_and_reverse_without_minimum_integer_overflow() {
    assert_eq!(PlaybackRate::new(2, 2).unwrap(), PlaybackRate::NORMAL);
    assert_eq!(PlaybackRate::new(0, u32::MAX).unwrap(), PlaybackRate::PAUSED);
    let reverse = PlaybackRate::new(-4, 6).unwrap();
    assert_eq!((reverse.numerator(), reverse.denominator()), (-2, 3));

    let reduced_minimum = PlaybackRate::new(i32::MIN, 1_u32 << 31).unwrap();
    assert_eq!((reduced_minimum.numerator(), reduced_minimum.denominator()), (-1, 1));
    let minimum = PlaybackRate::new(i32::MIN, 1).unwrap();
    assert_eq!((minimum.numerator(), minimum.denominator()), (i32::MIN, 1));
    assert_eq!(PlaybackRate::new(1, 0), Err(TimeError::ZeroDenominator));
    assert_eq!(PlaybackRate::new(0, 0), Err(TimeError::ZeroDenominator));
}
