use allocation_counter::measure;

use crate::{
    ClockBridgeConfig, DriftControllerConfig, FrameCount, InputReadError, InputWriteError, PlanarBuffer, SampleRate,
    input_clock_bridge,
};

#[test]
fn preallocated_rubato_bridge_processes_without_callback_allocation() {
    let config = config(48_000, 48_000);
    let (mut writer, mut reader) = input_clock_bridge(config).unwrap();
    let prefill = vec![0.0_f32; config.drift.target_fill_frames * usize::from(config.channels)];
    writer.write_interleaved(&prefill, Some(0)).unwrap();
    let block = vec![0.25_f32; config.engine_block_frames.get() as usize * usize::from(config.channels)];
    let mut destination =
        PlanarBuffer::new(usize::from(config.channels), config.engine_block_frames.get() as usize).unwrap();

    writer.write_interleaved(&block, Some(42_666_666)).unwrap();
    let warm = reader.read_engine_block(&mut destination).unwrap();
    assert_eq!(warm.rendered_frames, 128);
    assert!(!warm.underflowed);

    let allocation = measure(|| {
        for index in 0..100 {
            writer
                .write_interleaved(&block, Some(45_333_332 + index * 2_666_666))
                .unwrap();
            reader.read_engine_block(&mut destination).unwrap();
        }
    });
    assert_eq!(allocation.count_total, 0);
    assert_eq!(allocation.bytes_total, 0);
    assert!(reader.observation().estimated_latency_ms.is_finite());
}

#[test]
fn underflow_overflow_timestamp_loss_and_discontinuity_are_bounded_and_observable() {
    let mut overflow_config = config(48_000, 48_000);
    overflow_config.ring_capacity_frames = 512;
    overflow_config.drift.target_fill_frames = 256;
    let (mut writer, mut reader) = input_clock_bridge(overflow_config).unwrap();
    let mut destination = PlanarBuffer::new(2, 128).unwrap();

    let underflow = reader.read_engine_block(&mut destination).unwrap();
    assert!(underflow.underflowed);
    assert_eq!(reader.observation().underflow_count, 1);

    let full = vec![0.0_f32; overflow_config.ring_capacity_frames * 2];
    writer.write_interleaved(&full, Some(0)).unwrap();
    assert_eq!(
        writer.write_interleaved(&[0.0; 256], Some(2_666_666)),
        Err(InputWriteError::Overflow)
    );
    assert_eq!(writer.observation().overflow_count, 1);

    let mut discontinuity_config = config(48_000, 48_000);
    discontinuity_config.ring_capacity_frames = 1_024;
    discontinuity_config.drift.target_fill_frames = 512;
    let (mut writer, _reader) = input_clock_bridge(discontinuity_config).unwrap();
    writer.write_interleaved(&[0.0; 256], Some(0)).unwrap();
    writer.write_interleaved(&[0.0; 256], Some(100_000_000)).unwrap();
    writer.write_interleaved(&[0.0; 256], None).unwrap();
    let observation = writer.observation();
    assert_eq!(observation.discontinuity_count, 1);
    assert_eq!(observation.timestamp_loss_count, 1);
}

#[test]
fn abrupt_device_rate_change_flushes_old_domain_and_keeps_engine_rate_stable() {
    let config = config(48_000, 48_000);
    let (mut writer, mut reader) = input_clock_bridge(config).unwrap();
    let prefill = vec![0.0_f32; config.drift.target_fill_frames * 2];
    writer.write_interleaved(&prefill, Some(0)).unwrap();

    reader
        .reconfigure_device_rate(SampleRate::new(44_100).unwrap())
        .unwrap();

    assert_eq!(reader.config().engine_sample_rate.get(), 48_000);
    assert_eq!(reader.config().device_sample_rate.get(), 44_100);
    assert_eq!(reader.observation().fill_frames, 0);
    assert_eq!(reader.observation().discontinuity_count, 1);
}

#[test]
fn independent_input_domains_support_different_devices_and_hardware_rates() {
    for device_rate in [44_100, 48_000, 96_000] {
        let config = config(device_rate, 48_000);
        let (mut writer, mut reader) = input_clock_bridge(config).unwrap();
        let prefill = vec![0.0_f32; config.drift.target_fill_frames * 2];
        writer.write_interleaved(&prefill, Some(0)).unwrap();
        let mut destination = PlanarBuffer::new(2, 128).unwrap();
        let result = reader.read_engine_block(&mut destination).unwrap();
        assert_eq!(result.rendered_frames, 128);
        assert!(!result.underflowed);
        assert_eq!(reader.config().engine_sample_rate.get(), 48_000);
    }
}

#[test]
fn malformed_destination_and_input_are_rejected_without_partial_mutation() {
    let config = config(48_000, 48_000);
    let (mut writer, mut reader) = input_clock_bridge(config).unwrap();
    assert_eq!(
        writer.write_interleaved(&[0.0; 3], Some(0)),
        Err(InputWriteError::InvalidShape)
    );
    let mut destination = PlanarBuffer::new(1, 128).unwrap();
    assert_eq!(
        reader.read_engine_block(&mut destination),
        Err(InputReadError::InvalidDestination)
    );
}

fn config(device_rate: u32, engine_rate: u32) -> ClockBridgeConfig {
    ClockBridgeConfig {
        device_sample_rate: SampleRate::new(device_rate).unwrap(),
        engine_sample_rate: SampleRate::new(engine_rate).unwrap(),
        channels: 2,
        engine_block_frames: FrameCount::new(128).unwrap(),
        ring_capacity_frames: 4_096,
        output_buffer_frames: 128,
        drift: DriftControllerConfig {
            target_fill_frames: 1_024,
            ..DriftControllerConfig::default()
        },
    }
}
