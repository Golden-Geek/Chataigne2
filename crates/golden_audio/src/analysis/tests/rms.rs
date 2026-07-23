use uuid::Uuid;

use crate::{AudioChannelId, MeterAccumulator, PlanarBuffer, linear_to_dbfs};

fn channel(index: u128) -> AudioChannelId {
    AudioChannelId::from_uuid(Uuid::from_u128(index))
}

#[test]
fn linear_dbfs_has_a_deterministic_floor_and_preserves_clip_levels() {
    assert_eq!(linear_to_dbfs(0.0), -120.0);
    assert_eq!(linear_to_dbfs(f32::NAN), -120.0);
    assert!((linear_to_dbfs(0.5) + 6.020_6).abs() < 0.000_1);
    assert!((linear_to_dbfs(2.0) - 6.020_6).abs() < 0.000_1);
}

#[test]
fn meter_window_is_exact_and_independent_of_callback_partitioning() {
    let mut source = PlanarBuffer::new(4, 16).unwrap();
    source.channel_mut(0).fill(0.5);
    source.channel_mut(1)[..8].fill(1.0);
    source.channel_mut(1)[8..].fill(-1.0);
    for frame in 0..16 {
        source.set_sample(2, frame, (std::f32::consts::TAU * frame as f32 / 16.0).sin());
    }
    source.channel_mut(3).fill(0.0);

    let ids = vec![channel(1), channel(2), channel(3), channel(4)];
    let mut whole = MeterAccumulator::new(ids.clone(), 16).unwrap();
    let mut partitioned = MeterAccumulator::new(ids, 16).unwrap();
    let mut whole_result = Vec::new();
    let mut partitioned_result = Vec::new();
    whole
        .accumulate(&source, 16, |observations| whole_result = observations.to_vec())
        .unwrap();

    let mut first = PlanarBuffer::new(4, 7).unwrap();
    let mut second = PlanarBuffer::new(4, 9).unwrap();
    for channel in 0..4 {
        first
            .channel_mut(channel)
            .copy_from_slice(&source.channel(channel)[..7]);
        second
            .channel_mut(channel)
            .copy_from_slice(&source.channel(channel)[7..]);
    }
    partitioned
        .accumulate(&first, 7, |observations| {
            partitioned_result = observations.to_vec();
        })
        .unwrap();
    partitioned
        .accumulate(&second, 9, |observations| {
            partitioned_result = observations.to_vec();
        })
        .unwrap();

    assert_eq!(whole_result, partitioned_result);
    assert!((whole_result[0].rms_linear - 0.5).abs() < 1.0e-6);
    assert!((whole_result[1].rms_linear - 1.0).abs() < 1.0e-6);
    assert!((whole_result[2].rms_linear - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    assert_eq!(whole_result[3].rms_linear, 0.0);
    assert!(whole_result[1].clipped);
    assert!(!whole_result[0].clipped);
}
