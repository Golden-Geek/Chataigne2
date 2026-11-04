use super::*;

#[test]
fn fps_uses_successful_submission_count_and_elapsed_time() {
    for hz in [2u32, 30, 50, 60] {
        let mut sampler = FrameSampler::default();
        for frame in 0..=hz {
            sampler.record(Duration::from_secs_f64(f64::from(frame) / f64::from(hz)));
        }
        assert_eq!(sampler.fps, Some(f64::from(hz)));
    }
    let mut sampler = FrameSampler::default();
    sampler.record(Duration::ZERO);
    sampler.record(Duration::from_millis(900));
    assert_eq!(sampler.fps, None, "no nominal configured rate before measurement");
}

#[test]
fn local_clock_keeps_midnight_and_millisecond_zero_padding() {
    for (local, expected) in [([0, 0, 0, 0], "00:00:00.000"), ([23, 59, 59, 999], "23:59:59.999")] {
        let text = AsciiLine::clock(FrameText { local, fps: None });
        assert_eq!(&text.bytes[..text.len], expected.as_bytes());
    }
}

#[test]
fn idle_gap_or_clock_replacement_discards_the_old_fps_window() {
    let mut sampler = FrameSampler::default();
    for second in 0..=2 {
        sampler.observe(Duration::from_secs(second));
        sampler.record(Duration::from_secs(second));
    }
    assert_eq!(sampler.fps, Some(1.));
    sampler.observe(Duration::from_secs(10));
    assert_eq!(sampler.fps, None);
    sampler.record(Duration::from_secs(10));
    sampler.observe(Duration::ZERO);
    assert_eq!(sampler.fps, None);
}
