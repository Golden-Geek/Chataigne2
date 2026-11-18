use super::{fixture, seconds};
use golden_time::PlaybackRate;
use std::time::Duration;

#[test]
fn shared_transport_repeats_from_the_consumed_head_and_reanchors_on_seek() {
    let (clock, mut transport) = fixture();
    transport.set_loop_policy(true, seconds(2)).unwrap();
    transport.play().unwrap();

    clock.set(9_000_000_000);
    let half = transport.sample().unwrap();
    assert_eq!(half, transport.sample().unwrap());
    assert_eq!(half.position, seconds(9));
    // Both consumers share the same wrap and overlap. The shorter outgoing channel holds.
    assert_eq!(half.channel_position(seconds(3)), seconds(3));
    let blend = half.loop_blend().unwrap();
    assert_eq!((blend.secondary_position, blend.secondary_weight), (seconds(1), 0.5));
    assert_eq!(blend.primary_weight(), 0.5);

    clock.set(10_000_000_000);
    let wrapped = transport.sample().unwrap();
    assert_eq!((wrapped.cycle, wrapped.position), (1, seconds(2)));
    assert_eq!(wrapped.epoch, half.epoch);
    assert!(wrapped.loop_blend().is_none());
    assert_eq!(wrapped.frame_position().unwrap().position.as_nanos(), 10_000_000_000);

    clock.set(50_000_000_000);
    let wrapped = transport.sample().unwrap();
    assert_eq!((wrapped.cycle, wrapped.position), (6, seconds(2)));
    transport.seek(seconds(1)).unwrap();
    let sought = transport.sample().unwrap();
    assert_ne!(sought.epoch, wrapped.epoch);
    assert_eq!((sought.cycle, sought.position), (0, seconds(1)));
    assert!(sought.loop_blend().is_none());
}

#[test]
fn loop_toggle_pause_and_speed_changes_use_one_clock_and_keep_exact_file_time() {
    let (clock, mut transport) = fixture();
    transport.set_loop_policy(false, seconds(2)).unwrap();
    transport.play().unwrap();
    clock.set(9_000_000_000);
    let without_loop = transport.sample().unwrap();
    assert_eq!(without_loop.position, seconds(9));
    assert!(without_loop.loop_blend().is_none());

    transport.set_loop_policy(true, seconds(2)).unwrap();
    let with_loop = transport.sample().unwrap();
    assert_eq!(with_loop.position, without_loop.position);
    assert_ne!(with_loop.epoch, without_loop.epoch);
    assert_eq!(with_loop.loop_blend().unwrap().secondary_weight, 0.5);
    transport.set_rate(PlaybackRate::new(2, 1).unwrap()).unwrap();
    let speed = transport.sample().unwrap();
    assert_eq!(speed.position, with_loop.position);
    assert_ne!(speed.epoch, with_loop.epoch);
    clock.set(9_250_000_000);
    let before_pause = transport.sample().unwrap();
    assert_eq!(before_pause.position, Duration::from_millis(9500));
    assert_eq!(before_pause.loop_blend().unwrap().secondary_weight, 0.84375);
    transport.pause().unwrap();
    clock.set(20_000_000_000);
    let paused = transport.sample().unwrap();
    assert!(!paused.playing);
    assert_eq!(paused.position, before_pause.position);
    assert_eq!(paused.loop_blend(), before_pause.loop_blend());

    transport.set_loop_policy(false, seconds(2)).unwrap();
    let disabled = transport.sample().unwrap();
    assert_eq!(disabled.position, paused.position);
    assert!(disabled.loop_blend().is_none());
    transport.stop().unwrap();
    assert_eq!(transport.sample().unwrap().position, Duration::ZERO);
}

#[test]
fn zero_duration_policy_keeps_the_hard_wrap_path() {
    let (clock, mut transport) = fixture();
    transport.set_loop_policy(true, Duration::ZERO).unwrap();
    transport.play().unwrap();
    clock.set(10_000_000_000);
    let wrapped = transport.sample().unwrap();
    assert_eq!((wrapped.cycle, wrapped.position), (1, Duration::ZERO));
    assert!(wrapped.timeline.is_none());
    assert!(wrapped.loop_blend().is_none());
    assert_eq!(wrapped.frame_position().unwrap().position.as_nanos(), 10_000_000_000);
}

#[test]
fn reverse_stop_restores_the_original_first_frame_without_a_fade() {
    let (_, mut transport) = fixture();
    transport.set_loop_policy(true, seconds(2)).unwrap();
    transport.set_rate(PlaybackRate::new(-1, 1).unwrap()).unwrap();
    transport.seek(seconds(1)).unwrap();
    assert_eq!(transport.sample().unwrap().loop_blend().unwrap().secondary_weight, 0.5);
    transport.stop().unwrap();
    let stopped = transport.sample().unwrap();
    assert_eq!(stopped.position, Duration::ZERO);
    assert!(stopped.loop_blend().is_none());
}
