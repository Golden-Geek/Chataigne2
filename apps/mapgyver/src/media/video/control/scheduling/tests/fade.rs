use super::{frame, loop_target, loop_window, running, window};
use golden_time::{MonotonicTime, ShowPosition};
use std::time::Duration;

const OVERLAP: Duration = Duration::from_millis(200);

#[test]
fn forward_lookahead_crosses_to_consumed_head_at_first_seam() {
    let mut transport = running(true, 1);
    transport.set_loop_policy(true, OVERLAP).unwrap();
    let tail = transport.sample_at(MonotonicTime::from_nanos(999_000_000)).unwrap();
    let tail_window = loop_window(tail, frame(980, None)).unwrap();
    assert_eq!(tail_window.end().unwrap().as_nanos(), 1_000_000_000);
    assert!(tail_window.contains(tail.frame_position().unwrap()));
    let head = loop_target(tail, Some(tail_window)).unwrap().unwrap();
    assert_eq!((head.cycle, head.position), (1, OVERLAP));
    assert!(head.loop_blend().is_none());
    let head_window = loop_window(head, frame(197, Some(211))).unwrap();
    assert_eq!(head_window.start().as_nanos(), 1_000_000_000);
    assert_eq!(head_window.pts().as_nanos(), 197_000_000);
    assert!(!head_window.contains(tail.frame_position().unwrap()));
    assert!(loop_target(tail, Some(head_window)).unwrap().is_none());
    let wrapped = transport.sample_at(MonotonicTime::from_nanos(1_000_000_000)).unwrap();
    assert!(head_window.contains(wrapped.frame_position().unwrap()));
}

#[test]
fn reverse_exact_wrap_and_lookahead_select_lower_half_open_intervals() {
    let mut transport = running(true, -1);
    transport.set_loop_policy(true, OVERLAP).unwrap();
    let wrapped = transport.sample_at(MonotonicTime::from_nanos(1_000_000_000)).unwrap();
    assert_eq!((wrapped.cycle, wrapped.position), (-1, Duration::from_millis(800)));
    assert_eq!(wrapped.frame_position().unwrap().position.as_nanos(), -1);
    let decoded_time = wrapped.channel_position(Duration::from_secs(1));
    assert_eq!(decoded_time.as_nanos(), 799_999_999);
    let current = loop_window(wrapped, frame(795, Some(805))).unwrap();
    assert_eq!(current.start().as_nanos(), -5_000_000);
    assert_eq!(current.end().unwrap().as_nanos(), 0);
    assert!(current.contains(wrapped.frame_position().unwrap()));

    let ahead = loop_target(wrapped, Some(current)).unwrap().unwrap();
    let due = ahead.frame_position().unwrap();
    assert_eq!(due.position, ShowPosition::from_nanos(-5_000_001));
    let previous = loop_window(ahead, frame(780, Some(795))).unwrap();
    assert!(!current.contains(due));
    assert!(previous.contains(due));
    assert!(loop_target(wrapped, Some(previous)).unwrap().is_none());
}

#[test]
fn fade_windows_never_hide_seam_start_and_ignore_an_old_epoch_cursor() {
    let mut transport = running(true, 1);
    transport.set_loop_policy(true, OVERLAP).unwrap();
    let before = transport.sample_at(MonotonicTime::from_nanos(795_000_000)).unwrap();
    let before_window = loop_window(before, frame(790, Some(830))).unwrap();
    assert_eq!(before_window.end().unwrap().as_nanos(), 800_000_000);
    let seam = loop_target(before, Some(before_window)).unwrap().unwrap();
    let seam_window = loop_window(seam, frame(790, Some(830))).unwrap();
    assert_eq!(seam_window.start().as_nanos(), 800_000_000);
    assert_eq!(seam_window.end().unwrap().as_nanos(), 816_666_667);
    assert_eq!(seam_window.pts(), before_window.pts());

    transport.seek(Duration::from_millis(805)).unwrap();
    let sought = transport.sample().unwrap();
    assert_ne!(sought.epoch, seam.epoch);
    assert_eq!(loop_target(sought, Some(seam_window)).unwrap(), Some(sought));

    transport.set_loop_policy(true, Duration::ZERO).unwrap();
    let hard_wrap = transport.sample().unwrap();
    let decoded = frame(799, Some(830));
    assert_eq!(
        loop_window(hard_wrap, decoded).unwrap(),
        window(hard_wrap, decoded).unwrap()
    );
}
