use super::*;
use golden_time::ShowPosition;

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn cropped(looping: bool, rate: i32) -> TransportSample {
    let range = SourceRange::new(ms(200), ms(600)).unwrap();
    let sample = running(looping, rate).sample().unwrap();
    TransportSample {
        source_range: Some(range),
        duration: range.duration(),
        position: Duration::ZERO,
        ..sample
    }
}

#[test]
fn hard_loop_targets_exact_source_pts_and_skips_the_frame_at_crop_end() {
    let sample = cropped(true, 1);
    let head = frame(200, Some(233));
    let next = target(sample, Some(sample), Some(head), true, ms(1000))
        .unwrap()
        .unwrap();
    assert_eq!(next.position, ms(33));
    assert_eq!(next.channel_position(ms(1000)), ms(233));

    let sample = TransportSample {
        position: ms(367),
        ..sample
    };
    let tail = frame(567, Some(600));
    let tail_window = window(sample, tail).unwrap();
    assert_eq!(tail_window.pts(), ShowPosition::from_nanos(567_000_000));
    assert_eq!(tail_window.end(), Some(ShowPosition::from_nanos(400_000_000)));
    let restarted = target(sample, Some(sample), Some(tail), true, ms(1000))
        .unwrap()
        .unwrap();
    assert_eq!((restarted.cycle, restarted.position), (1, Duration::ZERO));
    assert_eq!(restarted.channel_position(ms(1000)), ms(200));
    let restarted_window = window(restarted, head).unwrap();
    assert_eq!(restarted_window.start(), tail_window.end().unwrap());
    assert!(!tail_window.contains(restarted.frame_position().unwrap()));
    assert!(restarted_window.contains(restarted.frame_position().unwrap()));
}

#[test]
fn reverse_boundary_and_short_channel_hold_stay_inside_the_cropped_cycle() {
    let sample = TransportSample {
        cycle: -1,
        position: ms(400),
        ..cropped(true, -1)
    };
    assert_eq!(sample.frame_position().unwrap().position.as_nanos(), -1);
    assert_eq!(sample.channel_position(ms(1000)).as_nanos(), 599_999_999);
    let tail = window(sample, frame(580, Some(600))).unwrap();
    assert_eq!(tail.start().as_nanos(), -20_000_000);
    assert_eq!(tail.end().unwrap().as_nanos(), 0);
    assert!(tail.contains(sample.frame_position().unwrap()));

    let held = window(sample, frame(120, Some(150))).unwrap();
    assert_eq!(held.pts().as_nanos(), 120_000_000);
    assert_eq!(held.start().as_nanos(), -400_000_000);
    assert_eq!(held.end().unwrap().as_nanos(), 0);
    assert!(held.contains(sample.frame_position().unwrap()));
}

#[test]
fn smooth_loop_schedules_the_cropped_tail_and_consumes_the_cropped_head() {
    use golden_video::scheduling::LoopTimeline;

    let sample = TransportSample {
        timeline: LoopTimeline::new(ms(400), ms(80)),
        position: ms(399),
        ..cropped(true, 1)
    };
    let tail = loop_window(sample, frame(570, Some(600))).unwrap();
    assert_eq!(tail.end().unwrap().as_nanos(), 400_000_000);
    assert_eq!(tail.pts().as_nanos(), 570_000_000);
    let restarted = loop_target(sample, Some(tail)).unwrap().unwrap();
    assert_eq!((restarted.cycle, restarted.position), (1, ms(80)));
    assert_eq!(restarted.channel_position(ms(1000)), ms(280));
    let head = loop_window(restarted, frame(270, Some(300))).unwrap();
    assert_eq!(head.start(), tail.end().unwrap());
    assert_eq!(head.pts().as_nanos(), 270_000_000);
}
