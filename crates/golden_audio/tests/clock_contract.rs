use golden_audio::{
    ClockBridgeConfig, ClockHandoffPhase, ClockSource, DriftControllerConfig, FrameCount, NullClockDriver,
    PlanarBuffer, RenderClockCoordinator, SampleRate, input_clock_bridge,
};

#[test]
fn external_consumer_can_compose_input_output_and_null_clock_domains() {
    let rate = SampleRate::new(48_000).unwrap();
    let block = FrameCount::new(128).unwrap();
    let bridge_config = ClockBridgeConfig {
        device_sample_rate: SampleRate::new(44_100).unwrap(),
        engine_sample_rate: rate,
        channels: 2,
        engine_block_frames: block,
        ring_capacity_frames: 4_096,
        output_buffer_frames: 128,
        drift: DriftControllerConfig {
            target_fill_frames: 1_024,
            ..DriftControllerConfig::default()
        },
    };
    let (mut input, mut render_input) = input_clock_bridge(bridge_config).unwrap();
    let source = vec![0.25; bridge_config.drift.target_fill_frames * usize::from(bridge_config.channels)];
    input.write_interleaved(&source, Some(0)).unwrap();
    let mut destination = PlanarBuffer::new(2, 128).unwrap();
    assert!(!render_input.read_engine_block(&mut destination).unwrap().underflowed);

    let mut clock = RenderClockCoordinator::new(rate, block).unwrap();
    let mut null_driver = NullClockDriver::new(rate, block, 2).unwrap();
    null_driver.resynchronize(0);
    assert_eq!(null_driver.poll(2_666_665), 0);
    assert_eq!(null_driver.poll(2_666_666), 1);
    assert!(clock.advance(ClockSource::Null, block).unwrap().render);

    clock.prime_output(1).unwrap();
    assert_eq!(clock.authority().phase, ClockHandoffPhase::FadingUp);
    assert!(clock.advance(ClockSource::Output(1), block).unwrap().render);
    assert_eq!(clock.frame(), 256);
}
