use super::*;
use crate::media::video::MediaTransport;
use golden_time::{Clock, MonotonicTime, PlaybackRate, TimeError};
use golden_video::scheduling::FrameBuffer;
mod transitions;

struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        Ok(MonotonicTime::from_nanos(0))
    }
}
fn running(looping: bool, rate: i32) -> MediaTransport<Fixed> {
    let mut transport = MediaTransport::new(Fixed).unwrap();
    transport.set_duration(Duration::from_secs(1)).unwrap();
    transport.set_looping(looping).unwrap();
    transport.set_rate(PlaybackRate::new(rate, 1).unwrap()).unwrap();
    transport.play().unwrap();
    transport
}
fn frame(position: u64, next: Option<u64>) -> Neighbours {
    Neighbours {
        position: Duration::from_millis(position),
        next: next.map(Duration::from_millis),
        previous: None,
        previous_ready: true,
    }
}

#[test]
fn one_future_frame_is_prepared_and_selected_only_when_its_pts_becomes_due() {
    let sample = running(false, 1).sample().unwrap();
    let current = frame(0, Some(17));
    let future = target(sample, Some(sample), Some(current), true, sample.duration)
        .unwrap()
        .unwrap();
    assert_eq!(future.position, Duration::from_millis(17));
    assert!(
        target(sample, Some(future), Some(frame(17, Some(34))), true, sample.duration)
            .unwrap()
            .is_none()
    );
    let mut queue = FrameBuffer::default();
    queue
        .publish(window(sample, current).unwrap(), 0, sample.frame_position().unwrap())
        .unwrap();
    queue
        .publish(
            window(future, frame(17, Some(34))).unwrap(),
            1,
            sample.frame_position().unwrap(),
        )
        .unwrap();
    for position in [0, 1, 8, 16] {
        let now = TransportSample {
            position: Duration::from_millis(position),
            ..sample
        };
        assert_eq!(*queue.sample(now.frame_position().unwrap()).unwrap().1, 0);
    }
    let due = TransportSample {
        position: Duration::from_millis(17),
        ..sample
    };
    assert_eq!(*queue.sample(due.frame_position().unwrap()).unwrap().1, 1);
}

#[test]
fn short_channel_holds_until_the_shared_loop_and_prefetches_the_next_cycle_once() {
    let sample = TransportSample {
        position: Duration::from_millis(200),
        ..running(true, 1).sample().unwrap()
    };
    let last = frame(200, None);
    let future = target(sample, Some(sample), Some(last), true, Duration::from_millis(250))
        .unwrap()
        .unwrap();
    assert_eq!((future.cycle, future.position), (1, Duration::ZERO));
    assert!(
        target(
            sample,
            Some(future),
            Some(frame(0, Some(17))),
            true,
            Duration::from_millis(250)
        )
        .unwrap()
        .is_none()
    );
    let mut queue = FrameBuffer::default();
    queue
        .publish(window(sample, last).unwrap(), 0, sample.frame_position().unwrap())
        .unwrap();
    queue
        .publish(
            window(future, frame(0, Some(17))).unwrap(),
            1,
            sample.frame_position().unwrap(),
        )
        .unwrap();
    let held = TransportSample {
        position: Duration::from_millis(999),
        ..sample
    };
    assert_eq!(*queue.sample(held.frame_position().unwrap()).unwrap().1, 0);
    assert_eq!(*queue.sample(future.frame_position().unwrap()).unwrap().1, 1);
}

#[test]
fn seeks_rates_pause_reverse_and_endpoints_keep_explicit_transport_semantics() {
    let mut transport = running(false, 1);
    let sample = transport.sample().unwrap();
    transport.seek(Duration::from_millis(100)).unwrap();
    let seek = transport.sample().unwrap();
    assert_ne!(sample.epoch, seek.epoch);
    assert_eq!(
        target(seek, Some(sample), Some(frame(0, Some(17))), true, seek.duration)
            .unwrap()
            .unwrap(),
        seek
    );
    transport.pause().unwrap();
    let pause = transport.sample().unwrap();
    assert_eq!(
        target(pause, Some(pause), Some(frame(100, Some(117))), true, pause.duration)
            .unwrap()
            .unwrap(),
        pause
    );
    transport.set_rate(PlaybackRate::new(-1, 1).unwrap()).unwrap();
    transport.play().unwrap();
    let reverse = transport.sample().unwrap();
    assert!(reverse.frame_position().unwrap().reverse);
    assert_eq!(
        target(
            reverse,
            Some(reverse),
            Some(frame(100, Some(117))),
            true,
            reverse.duration
        )
        .unwrap()
        .unwrap(),
        reverse
    );
    let last = TransportSample {
        position: Duration::from_millis(900),
        ..sample
    };
    assert!(
        target(last, Some(last), Some(frame(900, None)), true, last.duration)
            .unwrap()
            .is_none()
    );
    assert!(window(last, frame(900, None)).unwrap().end().is_none());
    let seam = running(true, -1).sample().unwrap();
    let last = window(seam, frame(900, None)).unwrap();
    assert!(
        last.contains(seam.frame_position().unwrap()),
        "reverse loop seam selects the previous cycle's end"
    );
}
