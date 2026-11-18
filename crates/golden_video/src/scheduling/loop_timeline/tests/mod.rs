use super::{LoopSample, LoopTimeline};
use golden_time::ShowPosition;
use std::time::Duration;

fn at(timeline: LoopTimeline, raw: i64, reverse: bool) -> LoopSample {
    let sample = timeline.sample(ShowPosition::from_nanos(raw), reverse).unwrap();
    assert_eq!(
        timeline.origin_nanos(sample.cycle) + sample.position.as_nanos() as i128,
        i128::from(raw)
    );
    sample
}

#[test]
fn hard_loop_retains_existing_signed_cycles_and_reverse_endpoints() {
    let timeline = LoopTimeline::new(Duration::from_nanos(10), Duration::ZERO).unwrap();
    for (raw, reverse, cycle, position) in [
        (9, false, 0, 9),
        (10, false, 1, 0),
        (-1, false, -1, 9),
        (10, true, 0, 10),
        (1, true, 0, 1),
        (0, true, -1, 10),
        (-10, true, -2, 10),
    ] {
        let sample = at(timeline, raw, reverse);
        assert_eq!(
            (sample.cycle, sample.position, sample.blend),
            (cycle, Duration::from_nanos(position), None)
        );
    }
}

#[test]
fn forward_seam_meets_the_already_consumed_head_without_replay() {
    let timeline = LoopTimeline::new(Duration::from_millis(1000), Duration::from_millis(200)).unwrap();
    assert_eq!(timeline.cycle_duration(), Duration::from_millis(800));
    assert_eq!(timeline.restart_position(false), Duration::from_millis(200));
    assert!(at(timeline, 799_999_999, false).blend.is_none());
    for repeat in [0, 1, 10, 100_000] {
        let origin = repeat * 800_000_000;
        let mut previous_weight = 0.0;
        for (millis, expected) in [(800, 0.0), (850, 0.15625), (900, 0.5), (950, 0.84375)] {
            let sample = at(timeline, origin + millis * 1_000_000, false);
            let blend = sample.blend.unwrap();
            assert_eq!(sample.cycle, repeat);
            assert_eq!(sample.position, Duration::from_millis(millis as u64));
            assert_eq!(blend.secondary_position, Duration::from_millis((millis - 800) as u64));
            assert_eq!(blend.secondary_weight, expected);
            assert!(blend.secondary_weight >= previous_weight);
            assert_eq!(blend.primary_weight() + blend.secondary_weight, 1.0);
            previous_weight = blend.secondary_weight;
        }
        let last = at(timeline, origin + 999_999_999, false).blend.unwrap();
        let next = at(timeline, origin + 1_000_000_000, false);
        assert_eq!(last.secondary_position + Duration::from_nanos(1), next.position);
        assert_eq!((next.cycle, next.blend), (repeat + 1, None));
    }
}

#[test]
fn reverse_seam_meets_the_tail_after_its_overlap_and_continues_backward() {
    let timeline = LoopTimeline::new(Duration::from_nanos(24), Duration::from_nanos(8)).unwrap();
    assert_eq!(timeline.restart_position(true), Duration::from_nanos(16));
    for offset in [0, -16, -32, -1600] {
        let sample = at(timeline, offset + 2, true);
        let blend = sample.blend.unwrap();
        assert_eq!(sample.position, Duration::from_nanos(2));
        assert_eq!(blend.secondary_position, Duration::from_nanos(18));
        assert_eq!(blend.secondary_weight, 0.84375);
        let before = at(timeline, offset + 1, true);
        let seam = at(timeline, offset, true);
        assert_eq!(
            before.blend.unwrap().secondary_position,
            seam.position + Duration::from_nanos(1)
        );
        assert_eq!(
            (seam.cycle, seam.position, seam.blend),
            (offset / 16 - 1, Duration::from_nanos(16), None)
        );
        let after = at(timeline, offset - 1, true);
        assert_eq!((after.cycle, after.position), (seam.cycle, Duration::from_nanos(15)));
    }
}

#[test]
fn reanchoring_at_a_file_coordinate_keeps_that_timestamp_in_cycle_zero() {
    let timeline = LoopTimeline::new(Duration::from_nanos(24), Duration::from_nanos(8)).unwrap();
    // These coordinates lie in the interval that would have been consumed by a prior seam.
    for (file_time, reverse) in [(3, false), (22, true), (12, false), (12, true)] {
        let sample = at(timeline, file_time, reverse);
        assert_eq!(sample.cycle, 0);
        assert_eq!(sample.position, Duration::from_nanos(file_time as u64));
    }
}

#[test]
fn short_and_unknown_sources_do_not_produce_a_zero_repeat_or_overflow() {
    assert!(LoopTimeline::new(Duration::ZERO, Duration::MAX).is_none());
    assert!(LoopTimeline::new(Duration::MAX, Duration::MAX).is_none());
    let smallest = LoopTimeline::new(Duration::from_nanos(1), Duration::MAX).unwrap();
    assert_eq!(smallest.overlap(), Duration::ZERO);
    assert_eq!(smallest.cycle_duration(), Duration::from_nanos(1));
    assert!(smallest.sample(ShowPosition::from_nanos(i64::MIN), true).is_none());
    let short = LoopTimeline::new(Duration::from_nanos(5), Duration::MAX).unwrap();
    assert_eq!(short.overlap(), Duration::from_nanos(2));
    assert_eq!(short.cycle_duration(), Duration::from_nanos(3));
    assert_eq!(at(short, 5, false).position, Duration::from_nanos(2));
    assert_eq!(at(short, 0, true).position, Duration::from_nanos(3));
    for (time, reverse) in [(i64::MIN, true), (i64::MAX, false)] {
        at(short, time, reverse);
    }
}

#[test]
fn source_bounds_apply_only_to_repeats_and_explicit_blends_include_exact_endpoints() {
    let ns = Duration::from_nanos;
    let timeline = LoopTimeline::new(ns(20), ns(6)).unwrap();
    assert_eq!(timeline.source_bounds(0, false), (ns(0), ns(20)));
    assert_eq!(timeline.source_bounds(0, true), (ns(0), ns(20)));
    assert_eq!(timeline.source_bounds(1, false), (ns(6), ns(20)));
    assert_eq!(timeline.source_bounds(-1, true), (ns(0), ns(14)));
    for (position, reverse, expected) in [(20, false, 6), (0, true, 14)] {
        let blend = timeline.blend_at(ns(position), reverse).unwrap();
        assert_eq!(blend.secondary_position, ns(expected));
        assert_eq!(blend.secondary_weight, 1.0);
        assert_eq!(blend.primary_weight(), 0.0);
    }
    assert!(timeline.blend_at(ns(21), false).is_none());
    assert!(timeline.blend_at(Duration::MAX, true).is_none());
}
