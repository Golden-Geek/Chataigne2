use super::*;
use golden_time::{PlaybackRate, SystemClock, TimeAuthority};

#[test]
fn forward_hold_uses_the_latest_past_pts_without_crossing_epoch_or_direction() {
    let mut clock = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let epoch = clock.sample().unwrap().epoch();
    let at = |position| FramePosition {
        epoch,
        position: ShowPosition::from_nanos(position),
        reverse: false,
    };
    let window = |start| {
        FrameWindow::new(
            epoch,
            at(start).position,
            Some(at(start + 10).position),
            at(start).position,
        )
        .unwrap()
    };
    let mut frames = FrameBuffer::default();
    frames.publish(window(10), 10, at(0)).unwrap();
    frames.publish(window(30), 30, at(0)).unwrap();
    assert!(frames.sample_or_forward_hold(at(9)).is_none());
    assert!(frames.sample(at(25)).is_none());
    assert_eq!(frames.sample_or_forward_hold(at(25)).unwrap().0, window(10));
    assert_eq!(frames.sample_or_forward_hold(at(31)).unwrap().0, window(30));
    assert_eq!(frames.sample_or_forward_hold(at(45)).unwrap().0, window(30));
    assert!(
        frames
            .sample_or_forward_hold(FramePosition {
                reverse: true,
                ..at(45)
            })
            .is_none()
    );
    let epoch = clock
        .discontinuity(at(0).position, PlaybackRate::PAUSED)
        .unwrap()
        .epoch();
    assert!(
        frames
            .sample_or_forward_hold(FramePosition { epoch, ..at(45) })
            .is_none()
    );
}

#[test]
fn prepared_frames_preserve_every_phase_without_early_selection_and_remain_bounded() {
    let mut authority =
        TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let epoch = authority.sample().unwrap().epoch();
    for phase in 0..17 {
        let mut frames = FrameBuffer::default();
        let at = |position| FramePosition {
            epoch,
            position: ShowPosition::from_nanos(position),
            reverse: false,
        };
        let window = |index| {
            FrameWindow::new(
                epoch,
                ShowPosition::from_nanos(index * 17),
                Some(ShowPosition::from_nanos((index + 1) * 17)),
                ShowPosition::from_nanos(index * 17),
            )
            .unwrap()
        };
        frames.publish(window(0), 0, at(0)).unwrap();
        frames.publish(window(1), 1, at(0)).unwrap();
        assert_eq!(frames.publish(window(2), 2, at(0)), Err(2));
        for index in 0..100 {
            let now = index * 17 + phase;
            assert_eq!(*frames.sample(at(now)).unwrap().1, index);
            assert!(frames.sample(at(-1)).is_none());
            let next = index + 2;
            if index < 99 {
                frames.publish(window(next), next, at((index + 1) * 17)).unwrap();
            }
        }
        let old = frames.clear();
        assert_eq!(old.into_iter().flatten().count(), 2);
        assert!(frames.is_empty());
    }
}

#[test]
fn reverse_epoch_replacement_and_final_hold_do_not_admit_future_or_stale_windows() {
    let mut authority =
        TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let epoch = authority.sample().unwrap().epoch();
    let at = |position| FramePosition {
        epoch,
        position: ShowPosition::from_nanos(position),
        reverse: true,
    };
    let w = |start, end: Option<i64>| {
        FrameWindow::new(
            epoch,
            ShowPosition::from_nanos(start),
            end.map(ShowPosition::from_nanos),
            ShowPosition::from_nanos(start),
        )
        .unwrap()
    };
    let mut frames = FrameBuffer::default();
    frames.publish(w(10, None), 1, at(15)).unwrap();
    frames.publish(w(0, Some(10)), 0, at(15)).unwrap();
    assert_eq!(*frames.sample(at(10)).unwrap().1, 1);
    assert_eq!(*frames.sample(at(9)).unwrap().1, 0);
    let replacement = authority
        .discontinuity(ShowPosition::from_nanos(0), PlaybackRate::PAUSED)
        .unwrap()
        .epoch();
    let now = FramePosition {
        epoch: replacement,
        ..at(0)
    };
    assert!(frames.sample(now).is_none());
    assert_eq!(frames.publish(w(0, Some(10)), 7, now), Err(7));
    let new = FrameWindow::new(
        replacement,
        ShowPosition::from_nanos(0),
        None,
        ShowPosition::from_nanos(0),
    )
    .unwrap();
    assert_eq!(frames.publish(new, 2, now).unwrap().into_iter().flatten().count(), 2);
    assert_eq!(*frames.sample(now).unwrap().1, 2);
}
