use super::*;
use crate::media::{assets, protocol::ChannelId, video::bindings::LoopFiles};
use golden_core::node::NodeUuid;

#[test]
fn separate_loop_file_extends_the_crop_and_ignores_the_single_file_loop_start() {
    let control = VideoControl::default();
    let channel = ChannelId(NodeUuid(uuid::Uuid::new_v4()));
    let owner = uuid::Uuid::new_v4();
    let mut settings = VideoTransportSettings {
        start_ms: 2000.0,
        end_ms: Some(6000.0),
        looping: true,
        loop_start_ms: 4000.0,
        auto_smooth_loop_ms: 1000.0,
        ..Default::default()
    };
    control.initialize(settings.clone()).unwrap();
    let bind = |picture| {
        control
            .bind_file_with_settings(
                channel,
                owner,
                assets::parse_file("intro.mp4").unwrap(),
                Arc::default(),
                Arc::default(),
                LoopFiles {
                    picture: assets::parse_file(picture).unwrap(),
                    audio: None,
                },
            )
            .unwrap();
    };
    bind("repeat.mp4");
    let files = control.metadata_files().unwrap();
    assert_eq!(files.len(), 2);
    control.metadata_duration(channel, files[0].1, seconds(8)).unwrap();
    let waiting = control.sample().unwrap();
    assert_eq!(waiting.duration, seconds(4));
    assert!(waiting.has_finite_end());
    control.metadata_duration(channel, files[1].1, seconds(3)).unwrap();
    let sample = control.sample().unwrap();
    assert_eq!(sample.duration, seconds(7));
    let timeline = sample.timeline.unwrap();
    assert_eq!(timeline.loop_start(), seconds(4));
    assert_eq!(timeline.overlap(), seconds(1));

    settings.position_ms = 5000.0;
    control.configure(settings.clone(), true, None).unwrap();
    assert_eq!(control.sample().unwrap().position, seconds(5));
    settings.looping = false;
    control.configure(settings, false, None).unwrap();
    assert_eq!(control.sample().unwrap().duration, seconds(4));

    bind("new-repeat.mp4");
    assert!(!control.metadata_duration(channel, files[1].1, seconds(50)).unwrap());
    assert_eq!(control.metadata_files().unwrap()[0].1, files[0].1);
}
