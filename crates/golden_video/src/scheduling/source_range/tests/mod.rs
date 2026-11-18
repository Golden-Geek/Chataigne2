use super::*;
use crate::scheduling::FramePosition;
use golden_time::{PlaybackRate, SystemClock, TimeAuthority};

fn ns(nanos: u64) -> Duration {
    Duration::from_nanos(nanos)
}

fn epoch() -> TimeEpoch {
    TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap()
        .sample()
        .unwrap()
        .epoch()
}

#[test]
fn inclusive_control_endpoint_and_exclusive_source_end_are_distinct() {
    let range = SourceRange::new(ns(100), ns(198)).unwrap();
    assert_eq!(range.duration(), ns(98));
    assert_eq!(range.source_position(Duration::ZERO), ns(100));
    assert_eq!(range.source_position(ns(98)), ns(198));
    assert_eq!(range.decode_position(ns(98)), ns(197));
    assert_eq!(range.decode_position(Duration::MAX), ns(197));
    assert_eq!(range.relative_position(ns(99)), Duration::ZERO);
    assert_eq!(range.relative_position(ns(198)), range.duration());
    assert!(range.contains(ns(100)));
    assert!(!range.contains(ns(198)));
    assert!(SourceRange::new(ns(11), ns(11)).is_none());
    assert!(SourceRange::new(ns(12), ns(11)).is_none());
    assert!(SourceRange::new(ns(0), Duration::MAX).is_none());
    assert_eq!(SourceRange::new(ns(11), ns(12)).unwrap().decode_position(ns(1)), ns(11));
}

#[test]
fn clipped_frame_windows_keep_pts_and_do_not_duplicate_the_boundary() {
    let range = SourceRange::new(ns(104), ns(198)).unwrap();
    let epoch = epoch();
    let first = range
        .frame_window(epoch, 0, ns(0)..ns(94), ns(100), Some(ns(120)), false)
        .unwrap();
    assert_eq!(first.start().as_nanos(), 0);
    assert_eq!(first.pts().as_nanos(), 100);
    assert_eq!(first.end().unwrap().as_nanos(), 16);
    let held_tail = range
        .frame_window(epoch, 0, ns(0)..ns(94), ns(187), Some(ns(198)), true)
        .unwrap();
    assert!(held_tail.end().is_none());
    assert!(
        range
            .frame_window(epoch, 0, ns(0)..ns(94), ns(198), None, false)
            .is_none()
    );

    for cycle in 0..1000 {
        let seam = (cycle + 1) * 94;
        let tail = range
            .frame_window(
                epoch,
                i128::from(seam - 94),
                ns(0)..ns(94),
                ns(187),
                Some(ns(198)),
                false,
            )
            .unwrap();
        let head = range
            .frame_window(epoch, i128::from(seam), ns(0)..ns(94), ns(100), Some(ns(120)), false)
            .unwrap();
        let at = |position, reverse| FramePosition {
            epoch,
            position: ShowPosition::from_nanos(position),
            reverse,
        };
        assert!(tail.contains(at(seam - 1, true)));
        assert!(!tail.contains(at(seam, false)));
        assert!(head.contains(at(seam, false)));
        assert!(!head.contains(at(seam - 1, true)));
    }
}

#[test]
fn cycle_specific_overlap_trims_the_first_and_last_frame_intervals() {
    let range = SourceRange::new(ns(100), ns(200)).unwrap();
    let epoch = epoch();
    let forward = range
        .frame_window(epoch, 80, ns(20)..ns(100), ns(110), Some(ns(135)), false)
        .unwrap();
    assert_eq!(forward.start().as_nanos(), 100);
    assert_eq!(forward.end().unwrap().as_nanos(), 115);
    assert_eq!(forward.pts().as_nanos(), 110);
    let reverse = range
        .frame_window(epoch, -80, ns(0)..ns(80), ns(175), Some(ns(190)), false)
        .unwrap();
    assert_eq!(reverse.start().as_nanos(), -5);
    assert_eq!(reverse.end().unwrap().as_nanos(), 0);
    assert_eq!(reverse.pts().as_nanos(), 175);
}
