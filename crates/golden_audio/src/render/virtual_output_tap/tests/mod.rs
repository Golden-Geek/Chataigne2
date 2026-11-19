use super::*;

#[test]
fn virtual_output_tap_exports_whole_planar_blocks_only_while_enabled() {
    let config = AudioEngineConfig::default();
    let limits = EngineLimits::default();
    let (mut writer, mut reader) = virtual_output_tap(&config, &limits, 2).unwrap();
    let frames = config.internal_block_frames.get() as usize;
    let mut buffer = PlanarBuffer::new(2, frames).unwrap();
    buffer.channel_mut(0).fill(0.25);
    buffer.channel_mut(1).fill(-0.5);
    let before = Instant::now();
    writer.capture(ConfigGeneration::INITIAL, &buffer, 0);
    reader.drain(2, |_| panic!("inactive tap exported PCM"));
    reader.set_enabled(true);
    writer.capture(ConfigGeneration::INITIAL.next(), &buffer, 0);
    let mut blocks = 0;
    reader.drain(2, |block| {
        blocks += 1;
        assert_eq!(block.generation, ConfigGeneration::INITIAL.next().get());
        assert_eq!(block.frames, frames);
        assert_eq!(block.sample_rate, config.sample_rate.get());
        assert!(block.starts_at >= before);
        assert!(block.samples[..frames].iter().all(|sample| *sample == 0.25));
        assert!(block.samples[frames..].iter().all(|sample| *sample == -0.5));
    });
    assert_eq!(blocks, 1);
}
