use super::*;
use crate::media::protocol::{AssetLocator, ChannelId, MediaId};
use crate::media::video::frames::Frames;
use golden_core::node::NodeUuid;

#[test]
fn stationary_and_advancing_clocks_use_distinct_publication_semantics() {
    let mut playback = running(false, 1);
    assert!(timed(playback.sample().unwrap()));
    playback.pause().unwrap();
    assert!(!timed(playback.sample().unwrap()));
    playback.set_rate(PlaybackRate::PAUSED).unwrap();
    playback.play().unwrap();
    assert!(!timed(playback.sample().unwrap()));
    let reverse = running(false, -1).sample().unwrap();
    assert!(timed(reverse));
    let endpoint = TransportSample {
        playing: false,
        ..reverse
    };
    assert!(!timed(endpoint));
}

#[test]
fn paused_seek_replaces_future_neighbours_with_exact_static_metadata() {
    let mut playback = running(false, 1);
    let moving = playback.sample().unwrap();
    let channel = ChannelId(NodeUuid(uuid::Uuid::new_v4()));
    let media = MediaId(NodeUuid(uuid::Uuid::new_v4()));
    let owner = uuid::Uuid::new_v4();
    let file = AssetLocator::ProjectRelative {
        path: "fixture.mov".to_owned().try_into().unwrap(),
    };
    let mut observed = Frames::default();
    observed.claim(channel, owner, file.clone());
    observed
        .observe_scheduled(
            channel,
            owner,
            window(moving, frame(0, Some(17))).unwrap(),
            frame(0, Some(17)),
            moving.frame_position().unwrap(),
        )
        .unwrap();
    observed
        .observe_scheduled(
            channel,
            owner,
            window(moving, frame(17, Some(34))).unwrap(),
            frame(17, Some(34)),
            moving.frame_position().unwrap(),
        )
        .unwrap();
    assert_eq!(
        observed.inspect(media, channel, &file, moving).unwrap().position_ms,
        0.0
    );
    playback.pause().unwrap();
    playback.seek(Duration::from_millis(100)).unwrap();
    let paused = playback.sample().unwrap();
    assert!(observed.inspect(media, channel, &file, paused).is_none());
    observed.observe(channel, owner, paused, frame(100, Some(117)));
    let current = observed.inspect(media, channel, &file, paused).unwrap();
    assert_eq!(current.position_ms, 100.0);
    assert!(current.previous_ready);
    playback.play().unwrap();
    let resumed = playback.sample().unwrap();
    observed
        .observe_scheduled(
            channel,
            owner,
            window(resumed, frame(100, Some(117))).unwrap(),
            frame(100, Some(117)),
            resumed.frame_position().unwrap(),
        )
        .unwrap();
    assert_eq!(
        observed.inspect(media, channel, &file, resumed).unwrap().position_ms,
        100.0
    );
}
