use super::*;
use crate::media::video::audio::AudioTimeline;
use golden_video::scheduling::SourceRange;

#[test]
fn separate_loop_projects_pcm_into_each_file_and_never_restarts_the_intro() {
    let (clock, mut transport) = fixture();
    transport.set_loop_section(true, seconds(6), seconds(1)).unwrap();
    transport.play().unwrap();
    for (offset_ms, repeat, expected_ms) in [
        (5000, false, 5000),
        (6000, true, 0),
        (9500, true, 3500),
        (10000, true, 1000),
        (13000, true, 1000),
    ] {
        clock.set(offset_ms * 1_000_000);
        let sample = audio_timeline(transport.sample().unwrap());
        let actual = sample.samples(0, seconds(6), Some(seconds(4)))[0].unwrap();
        assert_eq!(
            (actual.repeat, actual.position),
            (repeat, Duration::from_millis(expected_ms))
        );
        if offset_ms == 9500 {
            let pair = sample.samples(0, seconds(6), Some(seconds(4))).map(Option::unwrap);
            assert_eq!(pair[1].position, Duration::from_millis(500));
            assert_eq!(pair.map(|sample| sample.weight), [0.5, 0.5]);
            assert!(
                sample
                    .samples(0, seconds(6), None)
                    .into_iter()
                    .all(|sample| sample.is_none())
            );
        }
    }
}

fn audio_timeline(sample: TransportSample) -> AudioTimeline {
    AudioTimeline::with_repeat(sample, Some(seconds(6))).unwrap()
}

#[test]
fn audio_uses_crop_loop_and_reverse_positions_from_the_video_clock() {
    let (_, mut transport) = fixture();
    transport
        .set_source_range(Some(SourceRange::new(seconds(2), seconds(6)).unwrap()))
        .unwrap();
    transport.set_looping(true).unwrap();
    transport.play().unwrap();
    transport.seek(Duration::from_millis(3900)).unwrap();
    let timeline = AudioTimeline::new(transport.sample().unwrap()).unwrap();
    assert_eq!(
        timeline.positions(200_000_000, seconds(10))[0],
        Some((Duration::from_millis(2100), 1.0))
    );
    assert_eq!(timeline.positions(0, seconds(4)), [None, None]);

    transport.set_loop_policy(true, seconds(1)).unwrap();
    transport.seek(Duration::from_millis(3500)).unwrap();
    let timeline = AudioTimeline::new(transport.sample().unwrap()).unwrap();
    assert_eq!(
        timeline.positions(0, seconds(10)),
        [
            Some((Duration::from_millis(5500), 0.5)),
            Some((Duration::from_millis(2500), 0.5)),
        ]
    );

    transport.set_looping(false).unwrap();
    transport.set_rate(PlaybackRate::new(-1, 1).unwrap()).unwrap();
    transport.seek(seconds(1)).unwrap();
    let timeline = AudioTimeline::new(transport.sample().unwrap()).unwrap();
    assert_eq!(
        timeline.positions(250_000_000, seconds(10))[0],
        Some((Duration::from_nanos(2_749_999_999), 1.0))
    );
    transport.pause().unwrap();
    assert!(AudioTimeline::new(transport.sample().unwrap()).is_none());
}
