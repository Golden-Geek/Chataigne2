use super::*;
use crate::media::video::audio::AudioTimeline;
use golden_video::scheduling::SourceRange;

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
