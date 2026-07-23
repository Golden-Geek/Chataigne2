use crate::{ClockHandoffPhase, ClockSource, FrameCount, NullClockDriver, RenderClockCoordinator, SampleRate};

#[test]
fn output_loss_moves_to_null_without_resetting_sample_time_and_reconnect_resumes() {
    let mut clock = clock(256);
    let block = FrameCount::new(128).unwrap();
    assert_eq!(clock.advance(ClockSource::Null, block).unwrap().end_frame, 128);

    clock.prime_output(1).unwrap();
    assert_eq!(clock.advance(ClockSource::Output(1), block).unwrap().end_frame, 256);
    assert_eq!(clock.advance(ClockSource::Output(1), block).unwrap().end_frame, 384);
    assert_eq!(clock.authority().phase, ClockHandoffPhase::Stable);

    clock.output_lost(1);
    assert_eq!(clock.authority().source, ClockSource::Null);
    assert_eq!(clock.advance(ClockSource::Null, block).unwrap().end_frame, 512);

    clock.prime_output(2).unwrap();
    let reconnect = clock.advance(ClockSource::Output(2), block).unwrap();
    assert_eq!(reconnect.start_frame, 512);
    assert_eq!(reconnect.end_frame, 640);
}

#[test]
fn primed_replacement_fades_old_down_switches_once_and_fades_new_up() {
    let mut clock = clock(256);
    let block = FrameCount::new(128).unwrap();
    clock.prime_output(1).unwrap();
    clock.advance(ClockSource::Output(1), block).unwrap();
    clock.advance(ClockSource::Output(1), block).unwrap();

    clock.prime_output(2).unwrap();
    let early_new = clock.advance(ClockSource::Output(2), block).unwrap();
    assert!(!early_new.render);
    let first = clock.advance(ClockSource::Output(1), block).unwrap();
    let second = clock.advance(ClockSource::Output(1), block).unwrap();
    assert_eq!((first.gain_start, first.gain_end), (1.0, 0.5));
    assert_eq!((second.gain_start, second.gain_end), (0.5, 0.0));
    assert_eq!(clock.authority().source, ClockSource::Output(2));
    assert_eq!(clock.retired_outputs(), 1);

    let up = clock.advance(ClockSource::Output(2), block).unwrap();
    assert_eq!((up.gain_start, up.gain_end), (0.0, 0.5));
    assert!(!clock.advance(ClockSource::Output(1), block).unwrap().render);
}

#[test]
fn null_clock_driver_is_deadline_based_and_catch_up_is_bounded() {
    let mut driver = NullClockDriver::new(SampleRate::new(48_000).unwrap(), FrameCount::new(480).unwrap(), 3).unwrap();
    assert_eq!(driver.poll(0), 0);
    assert_eq!(driver.poll(9_999_999), 0);
    assert_eq!(driver.poll(10_000_000), 1);
    assert_eq!(driver.poll(100_000_000), 3);
    driver.resynchronize(100_000_000);
    assert_eq!(driver.poll(105_000_000), 0);
}

#[test]
fn loss_during_handoff_promotes_the_already_primed_replacement() {
    let mut coordinator =
        RenderClockCoordinator::new(SampleRate::new(48_000).unwrap(), FrameCount::new(128).unwrap()).unwrap();
    coordinator.prime_output(1).unwrap();
    coordinator
        .advance(ClockSource::Output(1), FrameCount::new(128).unwrap())
        .unwrap();
    coordinator.prime_output(2).unwrap();

    coordinator.output_lost(1);

    assert_eq!(coordinator.authority().source, ClockSource::Output(2));
    assert_eq!(coordinator.authority().phase, ClockHandoffPhase::FadingUp);
    assert_eq!(coordinator.authority().pending_output, None);
    assert_eq!(coordinator.retired_outputs(), 1);
    let block = coordinator
        .advance(ClockSource::Output(2), FrameCount::new(128).unwrap())
        .unwrap();
    assert!(block.render);
    assert_eq!((block.start_frame, block.end_frame), (128, 256));
}

#[test]
fn repeated_switching_retires_every_old_output_without_retaining_pending_state() {
    let mut clock = clock(1);
    let frame = FrameCount::new(1).unwrap();
    clock.prime_output(1).unwrap();
    clock.advance(ClockSource::Output(1), frame).unwrap();
    for generation in 2..=10_000 {
        let old = generation - 1;
        clock.prime_output(generation).unwrap();
        clock.advance(ClockSource::Output(old), frame).unwrap();
        clock.advance(ClockSource::Output(generation), frame).unwrap();
    }
    assert_eq!(clock.authority().source, ClockSource::Output(10_000));
    assert_eq!(clock.authority().phase, ClockHandoffPhase::Stable);
    assert_eq!(clock.authority().pending_output, None);
    assert_eq!(clock.retired_outputs(), 9_999);
}

#[test]
fn one_hour_mock_reconnect_soak_has_exact_monotonic_growth() {
    let mut clock = clock(1);
    let block = FrameCount::new(128).unwrap();
    let blocks = 48_000_u64 * 60 * 60 / 128;
    let mut generation = 1_u64;
    for index in 0..blocks {
        if index % 10_000 == 0 {
            if matches!(clock.authority().source, ClockSource::Output(active) if active == generation) {
                clock.output_lost(generation);
                generation += 1;
            } else {
                clock.prime_output(generation).unwrap();
            }
        }
        let source = clock.authority().source;
        let rendered = clock.advance(source, block).unwrap();
        assert!(rendered.render);
    }
    assert_eq!(clock.frame(), blocks * 128);
    assert_eq!(clock.authority().pending_output, None);
}

fn clock(fade_frames: u32) -> RenderClockCoordinator {
    RenderClockCoordinator::new(SampleRate::new(48_000).unwrap(), FrameCount::new(fade_frames).unwrap()).unwrap()
}
