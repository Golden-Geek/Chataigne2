use super::*;
mod frames;
mod membership;
#[cfg(feature = "video-files")]
mod metadata;
#[cfg(feature = "video-files")]
mod reader;
use golden_time::{Clock, MonotonicTime, PlaybackRate, TimeError};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

#[derive(Clone, Default)]
struct FakeClock(Arc<AtomicU64>);
impl Clock for FakeClock {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        Ok(MonotonicTime::from_nanos(self.0.load(Ordering::SeqCst)))
    }
}
impl FakeClock {
    fn set(&self, nanos: u64) {
        self.0.store(nanos, Ordering::SeqCst);
    }
}
fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}
fn fixture() -> (FakeClock, MediaTransport<FakeClock>) {
    let clock = FakeClock::default();
    let mut transport = MediaTransport::new(clock.clone()).unwrap();
    transport.set_duration(seconds(10)).unwrap();
    (clock, transport)
}

#[test]
fn defaults_and_unavailable_duration_do_not_start_playback() {
    let mut transport = MediaTransport::new(FakeClock::default()).unwrap();
    assert_eq!(transport.rate(), PlaybackRate::NORMAL);
    assert!(!transport.looping());
    assert_eq!(transport.sample().unwrap().position, Duration::ZERO);
    assert!(matches!(transport.play(), Err(TransportError::NoDuration)));
    assert!(!transport.sample().unwrap().playing);
}

#[test]
fn repeated_consumers_do_not_advance_or_round_the_media_clock() {
    let (clock, mut transport) = fixture();
    transport.set_rate(PlaybackRate::new(1, 3).unwrap()).unwrap();
    transport.play().unwrap();
    for now in 1..=300 {
        clock.set(now);
        let first = transport.sample().unwrap();
        let reference = transport.sample().unwrap();
        assert_eq!(first, reference);
        assert_eq!(first.position.as_nanos(), u128::from(now / 3));
    }
}

#[test]
fn pause_resume_stop_and_end_restart_preserve_rate_and_position() {
    let (clock, mut transport) = fixture();
    transport.play().unwrap();
    clock.set(2_000_000_000);
    transport.pause().unwrap();
    clock.set(5_000_000_000);
    assert_eq!(transport.sample().unwrap().position, seconds(2));
    transport.play().unwrap();
    clock.set(6_000_000_000);
    assert_eq!(transport.sample().unwrap().position, seconds(3));
    transport.stop().unwrap();
    assert!(!transport.sample().unwrap().playing);
    assert_eq!(transport.sample().unwrap().position, Duration::ZERO);
    transport.play().unwrap();
    clock.set(20_000_000_000);
    assert_eq!(transport.sample().unwrap().position, seconds(10));
    assert!(!transport.sample().unwrap().playing);
    transport.play().unwrap();
    assert_eq!(transport.sample().unwrap().position, Duration::ZERO);
}

#[test]
fn longer_media_timeline_holds_shorter_channels_and_loop_fences_change() {
    let (clock, mut transport) = fixture();
    transport.set_looping(true).unwrap();
    transport.play().unwrap();
    clock.set(5_000_000_000);
    let first = transport.sample().unwrap();
    assert_eq!(first.position, seconds(5));
    assert_eq!(first.channel_position(seconds(3)), seconds(3));
    clock.set(15_000_000_000);
    let next = transport.sample().unwrap();
    assert_eq!(next.position, first.position);
    assert_eq!(next.epoch, first.epoch);
    assert_eq!(next.cycle, first.cycle + 1);
    transport.pause().unwrap();
    clock.set(30_000_000_000);
    assert_eq!(transport.sample().unwrap().position, seconds(5));
}

#[test]
fn seek_and_exact_selected_channel_steps_pause_and_fence_old_work() {
    let (clock, mut transport) = fixture();
    transport.play().unwrap();
    let before = transport.sample().unwrap();
    transport.seek(seconds(4)).unwrap();
    let after = transport.sample().unwrap();
    assert_eq!(after.position, seconds(4));
    assert!(after.playing);
    assert_ne!(after.epoch, before.epoch);
    // Variable-rate channel neighbour supplied by the decoder, not a nominal FPS estimate.
    let exact = Duration::from_nanos(4_123_456_789);
    transport.step_to_timestamp(exact).unwrap();
    clock.set(8_000_000_000);
    assert_eq!(transport.sample().unwrap().position, exact);
    assert!(!transport.sample().unwrap().playing);
}

#[test]
fn speed_changes_are_continuous_and_reverse_restarts_from_the_end() {
    let (clock, mut transport) = fixture();
    transport.play().unwrap();
    clock.set(2_000_000_000);
    transport.set_rate(PlaybackRate::new(2, 1).unwrap()).unwrap();
    clock.set(3_000_000_000);
    assert_eq!(transport.sample().unwrap().position, seconds(4));
    transport.stop().unwrap();
    transport.set_rate(PlaybackRate::new(-1, 1).unwrap()).unwrap();
    transport.play().unwrap();
    assert_eq!(transport.sample().unwrap().position, seconds(10));
    clock.set(5_000_000_000);
    assert_eq!(transport.sample().unwrap().position, seconds(8));
}

#[test]
fn rejected_controls_and_clock_regression_leave_accepted_mapping_intact() {
    let (clock, mut transport) = fixture();
    transport.seek(seconds(3)).unwrap();
    let before = transport.sample().unwrap();
    assert!(matches!(
        transport.seek(seconds(11)),
        Err(TransportError::OutsideDuration)
    ));
    assert!(matches!(
        transport.set_duration(Duration::MAX),
        Err(TransportError::Overflow)
    ));
    assert_eq!(transport.sample().unwrap(), before);
    clock.set(100);
    transport.sample().unwrap();
    clock.set(99);
    assert!(matches!(transport.stop(), Err(TransportError::Time(_))));
    clock.set(100);
    assert_eq!(transport.sample().unwrap(), before);
}

#[test]
fn reverse_loop_boundaries_present_the_last_frame_and_fence_the_old_cycle() {
    let (clock, mut transport) = fixture();
    transport.set_rate(PlaybackRate::new(-1, 1).unwrap()).unwrap();
    transport.set_looping(true).unwrap();
    transport.play().unwrap();
    let first = transport.sample().unwrap();
    assert_eq!(first.position, seconds(10));
    assert_eq!(first.cycle, 0);
    clock.set(10_000_000_000);
    let next = transport.sample().unwrap();
    assert_eq!(next.position, seconds(10));
    assert_eq!(next.cycle, -1);
}

#[test]
fn duration_changes_preserve_position_and_reject_unrepresentable_cycles() {
    let (clock, mut transport) = fixture();
    transport.play().unwrap();
    clock.set(5_000_000_000);
    let before = transport.sample().unwrap();
    transport.set_duration(seconds(10)).unwrap();
    assert_eq!(transport.sample().unwrap(), before);
    transport.set_duration(seconds(3)).unwrap();
    assert_eq!(transport.sample().unwrap().position, seconds(3));
    assert!(!transport.sample().unwrap().playing);
    transport.set_duration(seconds(12)).unwrap();
    clock.set(6_000_000_000);
    assert_eq!(transport.sample().unwrap().position, seconds(3));
    assert!(!transport.sample().unwrap().playing);

    let clock = FakeClock::default();
    let mut transport = MediaTransport::new(clock.clone()).unwrap();
    transport.set_duration(Duration::from_nanos(1)).unwrap();
    transport.set_rate(PlaybackRate::new(-1, 1).unwrap()).unwrap();
    transport.set_looping(true).unwrap();
    transport.play().unwrap();
    clock.set(i64::MAX as u64 + 2);
    assert!(matches!(transport.sample(), Err(TransportError::Overflow)));
}
