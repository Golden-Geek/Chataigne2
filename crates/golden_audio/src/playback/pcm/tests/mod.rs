use allocation_counter::measure;

use super::*;

#[test]
fn clocked_pcm_discards_expired_blocks_and_invalid_generations_without_allocating() {
    let frames = FrameCount::new(4).unwrap();
    let (mut writer, mut reader) = pcm_playback_bus(2, frames, 4, &EngineLimits::default()).unwrap();
    let mut destination = PlanarBuffer::new(2, 4).unwrap();
    let old = ConfigGeneration::new(1);
    let current = old.next();

    assert!(writer.try_write(old, 4, &[0.75; 8]).unwrap());
    assert!(writer.try_write(current, 8, &[0.90; 8]).unwrap());
    writer.invalidate();
    assert!(writer.try_write(current, 4, &[0.50; 8]).unwrap());
    assert!(writer.try_write(current, 12, &[0.25; 8]).unwrap());
    assert!(!writer.try_write(current, 16, &[0.25; 8]).unwrap());
    assert!(writer.try_write(current, 12, &[0.25; 8]).is_err());
    assert!(writer.try_write(current, 17, &[0.25; 8]).is_err());

    let allocations = measure(|| {
        reader.mix(current, 8, &mut destination);
        assert_eq!(destination.channel(0), [0.0; 4]);
        reader.mix(current, 12, &mut destination);
    });
    assert_eq!(allocations.count_total, 0);
    assert_eq!(allocations.count_current, 0);
    assert_eq!(destination.channel(0), [0.25; 4]);
    assert_eq!(destination.channel(1), [0.25; 4]);

    let now = Instant::now();
    reader.publish_cursor(current.next(), 16, 480, now);
    assert_eq!(
        writer.cursor(),
        PcmRenderCursor {
            generation: current.next(),
            next_frame: 16,
            observed_at: now,
            queued_output_frames: 480,
        }
    );
}

#[test]
fn changing_the_playback_plan_does_not_reuse_an_old_lanes_samples() {
    let frames = FrameCount::new(4).unwrap();
    let (mut writer, mut reader) = pcm_playback_bus(1, frames, 4, &EngineLimits::default()).unwrap();
    let mut destination = PlanarBuffer::new(1, 4).unwrap();
    let previous = ConfigGeneration::new(1);
    let current = previous.next();
    assert!(writer.try_write(previous, 4, &[0.75; 4]).unwrap());
    assert!(
        writer
            .try_write(current, 8, &[0.5, f32::NAN, f32::INFINITY, 0.25])
            .unwrap()
    );
    reader.mix(current, 4, &mut destination);
    assert_eq!(destination.channel(0), [0.0; 4]);
    reader.mix(current, 8, &mut destination);
    assert_eq!(destination.channel(0), [0.5, 0.0, 0.0, 0.25]);
}
