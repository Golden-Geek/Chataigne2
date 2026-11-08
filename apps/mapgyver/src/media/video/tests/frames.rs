use super::*;
use crate::media::protocol::{AssetLocator, ChannelId, ChannelReference, MediaId};
use crate::media::video::protocol::*;
use golden_core::node::NodeUuid;

#[test]
fn prepared_steps_are_fenced_by_file_epoch_and_source_incarnation() {
    let control = VideoControl::default();
    let reference = ChannelReference {
        media: MediaId(NodeUuid(uuid::Uuid::new_v4())),
        channel: ChannelId(NodeUuid(uuid::Uuid::new_v4())),
    };
    let first = uuid::Uuid::new_v4();
    let second = uuid::Uuid::new_v4();
    let file = AssetLocator::ProjectRelative {
        path: "clip.mp4".to_owned().try_into().unwrap(),
    };
    let other = AssetLocator::ProjectRelative {
        path: "other.mp4".to_owned().try_into().unwrap(),
    };
    control.claim_channel(reference.channel, first, file.clone()).unwrap();
    control
        .channel_duration(reference.channel, first, Duration::from_secs(1))
        .unwrap();
    let sample = control.sample().unwrap();
    let frame = Neighbours {
        position: Duration::ZERO,
        previous: None,
        previous_ready: true,
        next: Some(Duration::from_nanos(33_366_667)),
    };
    control
        .observe_neighbours(reference.channel, first, sample, frame)
        .unwrap();
    assert_eq!(
        control
            .step_position(reference.channel, &file, VideoStepDirection::Next)
            .unwrap(),
        super::super::frames::millis(frame.next.unwrap())
    );
    assert!(
        control
            .step_position(reference.channel, &other, VideoStepDirection::Next)
            .is_err()
    );
    control
        .configure(
            VideoTransportSettings {
                position_ms: 100.0,
                ..Default::default()
            },
            true,
        )
        .unwrap();
    control
        .observe_neighbours(reference.channel, first, sample, frame)
        .unwrap();
    assert!(
        control.frame(reference, &file).unwrap().is_none(),
        "old seek epoch cannot advertise ready neighbours"
    );
    control.claim_channel(reference.channel, second, file.clone()).unwrap();
    let fresh = control.sample().unwrap();
    control
        .observe_neighbours(reference.channel, first, fresh, frame)
        .unwrap();
    assert!(
        control.frame(reference, &file).unwrap().is_none(),
        "retired decoder cannot populate its replacement"
    );
    control
        .observe_neighbours(reference.channel, second, fresh, frame)
        .unwrap();
    control.release_channel(reference.channel, first);
    assert!(control.frame(reference, &file).unwrap().is_some());
    control.release_channel(reference.channel, second);
    assert!(control.frame(reference, &file).unwrap().is_none());
}

#[test]
fn timestamp_parameter_projection_never_rounds_before_the_target_frame() {
    for nanos in [0, 33_366_667, 41_666_667, 9_000_000_000_000_000_000] {
        let position = Duration::from_nanos(nanos);
        let millis = super::super::frames::millis(position);
        assert!(Duration::from_secs_f64(millis / 1000.0) >= position);
    }
}
