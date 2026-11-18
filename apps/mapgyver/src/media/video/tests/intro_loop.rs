use super::*;
use golden_video::scheduling::SourceRange;

#[test]
fn the_cropped_intro_plays_once_then_picture_and_audio_enter_the_tail() {
    let (clock, mut transport) = fixture();
    transport
        .set_source_range(SourceRange::new(seconds(2), seconds(10)))
        .unwrap();
    transport.set_loop_section(true, seconds(5), Duration::ZERO).unwrap();
    transport.play().unwrap();
    for (at, cycle, source) in [(0, 0, 2), (4, 0, 6), (7, 0, 9), (8, 1, 7), (11, 2, 7), (12, 2, 8)] {
        clock.set(at * 1_000_000_000);
        let sample = transport.sample().unwrap();
        assert_eq!(
            (sample.cycle, sample.channel_position(seconds(10))),
            (cycle, seconds(source))
        );
        #[cfg(feature = "video-files")]
        {
            let audio = crate::media::video::audio::AudioTimeline::new(sample).unwrap();
            assert_eq!(audio.positions(0, seconds(10)), [Some((seconds(source), 1.0)), None]);
        }
    }
    transport.pause().unwrap();
    clock.set(21_000_000_000);
    assert_eq!(transport.sample().unwrap().channel_position(seconds(10)), seconds(8));
    transport.stop().unwrap();
    assert_eq!(transport.sample().unwrap().channel_position(seconds(10)), seconds(2));
    transport.seek(seconds(8)).unwrap();
    assert_eq!(
        transport.sample().unwrap().channel_position(seconds(10)),
        seconds(10) - Duration::from_nanos(1)
    );
    transport.play().unwrap();
    assert_eq!(transport.sample().unwrap().channel_position(seconds(10)), seconds(2));
}

#[test]
fn reverse_pause_and_speed_changes_preserve_the_established_tail_pass() {
    let (clock, mut transport) = fixture();
    transport.set_loop_section(true, seconds(6), seconds(1)).unwrap();
    transport.set_rate(PlaybackRate::new(-1, 1).unwrap()).unwrap();
    transport.play().unwrap();
    clock.set(10_000_000_000);
    let first_repeat = transport.sample().unwrap();
    assert_eq!((first_repeat.cycle, first_repeat.position), (-1, seconds(9)));
    transport.pause().unwrap();
    assert_eq!(transport.sample().unwrap().position, first_repeat.position);
    transport.set_rate(PlaybackRate::new(-2, 1).unwrap()).unwrap();
    transport.play().unwrap();
    clock.set(11_500_000_000);
    let second_repeat = transport.sample().unwrap();
    assert_eq!((second_repeat.cycle, second_repeat.position), (-2, seconds(9)));
    assert!(transport.sample().unwrap().cycle_start() >= seconds(6));
}
