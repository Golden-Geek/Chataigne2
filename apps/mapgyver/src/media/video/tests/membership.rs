use super::*;
use crate::media::protocol::{AssetLocator, ChannelId};
use golden_core::node::NodeUuid;

#[test]
fn authored_bindings_fence_metadata_and_removed_channels_while_retired_media_keeps_clip_time() {
    let control = VideoControl::default();
    let short = ChannelId(NodeUuid(uuid::Uuid::new_v4()));
    let long = ChannelId(NodeUuid(uuid::Uuid::new_v4()));
    let owner = uuid::Uuid::new_v4();
    let file = AssetLocator::ProjectRelative {
        path: "clip.mp4".to_owned().try_into().unwrap(),
    };
    control.bind_file(short, owner, Some(file.clone())).unwrap();
    control.bind_file(long, owner, Some(file.clone())).unwrap();
    let files = control.metadata_files().unwrap();
    control
        .configure(
            protocol::VideoTransportSettings {
                position_ms: 1500.0,
                ..Default::default()
            },
            true,
            None,
        )
        .unwrap();
    let short_generation = files.iter().find(|(channel, _, _)| *channel == short).unwrap().1;
    let long_generation = files.iter().find(|(channel, _, _)| *channel == long).unwrap().1;
    control.metadata_duration(short, short_generation, seconds(1)).unwrap();
    assert_eq!(
        control.sample().unwrap().position,
        Duration::ZERO,
        "unknown-duration requests are not saved for later metadata"
    );
    control.metadata_duration(long, long_generation, seconds(2)).unwrap();
    assert_eq!(control.sample().unwrap().position, Duration::ZERO);
    control
        .configure(
            protocol::VideoTransportSettings {
                position_ms: 1500.0,
                ..Default::default()
            },
            true,
            None,
        )
        .unwrap();
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(1500));
    let epoch = control.sample().unwrap().epoch;
    let old = files.iter().find(|(channel, _, _)| *channel == long).unwrap().1;
    control.release_channel(long, uuid::Uuid::new_v4());
    assert_eq!(
        control.sample().unwrap().epoch,
        epoch,
        "consumer removal does not own metadata"
    );
    control.bind_file(long, owner, None).unwrap();
    assert_eq!(control.sample().unwrap().position, seconds(1));
    assert!(!control.metadata_duration(long, old, seconds(3)).unwrap());
    control.bind_file(long, owner, Some(file)).unwrap();
    assert!(
        !control.metadata_duration(long, old, seconds(3)).unwrap(),
        "A-to-clear-to-A cannot reuse the old probe"
    );
    let fresh = control
        .metadata_files()
        .unwrap()
        .into_iter()
        .find(|(channel, _, _)| *channel == long)
        .unwrap()
        .1;
    control.metadata_duration(long, fresh, seconds(2)).unwrap();
    control.unbind_file(long, uuid::Uuid::new_v4());
    assert_eq!(
        control.metadata_files().unwrap().len(),
        2,
        "retired channel incarnations cannot remove a replacement"
    );
    control.unbind_file(long, owner);
    control
        .configure(
            protocol::VideoTransportSettings {
                position_ms: 900.0,
                ..Default::default()
            },
            true,
            None,
        )
        .unwrap();
    control.retire_media();
    control.unbind_file(short, owner);
    assert_eq!(
        control.metadata_files().unwrap().len(),
        1,
        "outgoing owned clips retain the retired media's metadata"
    );
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(900));
    control.activate_media();
    control.unbind_file(short, owner);
    assert!(control.metadata_files().unwrap().is_empty());
    assert_eq!(control.sample().unwrap().position, Duration::ZERO);
}
