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
    control
        .bind_file(reference.channel, uuid::Uuid::new_v4(), Some(file.clone()))
        .unwrap();
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
    let mut settings = control.settings().unwrap();
    settings.speed = 0.5;
    control.configure(settings, false, None).unwrap();
    assert_eq!(control.sample().unwrap().epoch, sample.epoch);
    assert_eq!(
        control
            .step_position(reference.channel, &file, VideoStepDirection::Previous)
            .unwrap(),
        None,
        "paused BOF is a no-op with the next timestamp still immediately available"
    );
    assert_eq!(
        control
            .step_position(reference.channel, &file, VideoStepDirection::Next)
            .unwrap(),
        Some(super::super::frames::millis(frame.next.unwrap()))
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
            None,
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

#[test]
fn frame_position_and_steps_are_relative_to_the_crop_and_exclude_the_end() {
    use super::super::frames::Frames;
    use golden_video::scheduling::SourceRange;

    let ms = Duration::from_millis;
    let control = VideoControl::default();
    let channel = ChannelId(NodeUuid(uuid::Uuid::new_v4()));
    let media = MediaId(NodeUuid(uuid::Uuid::new_v4()));
    let owner = uuid::Uuid::new_v4();
    let file = AssetLocator::ProjectRelative {
        path: "crop.mp4".to_owned().try_into().unwrap(),
    };
    let range = SourceRange::new(ms(125), ms(400)).unwrap();
    let mut sample = crate::media::video::TransportSample {
        source_range: Some(range),
        duration: range.duration(),
        position: range.duration(),
        ..control.sample().unwrap()
    };
    let mut frames = Frames::default();
    frames.claim(channel, owner, file.clone());
    frames.observe(
        channel,
        owner,
        sample,
        Neighbours {
            position: ms(300),
            previous: Some(ms(200)),
            previous_ready: true,
            next: Some(ms(400)),
        },
    );
    let state = frames.inspect(media, channel, &file, sample).unwrap();
    assert_eq!(state.position_ms, 175.0);
    assert_eq!(state.previous_ms, Some(75.0));
    assert_eq!(state.next_ms, None);
    assert_eq!(
        frames.step(channel, &file, sample, VideoStepDirection::Previous),
        Ok(ms(75))
    );
    assert_eq!(
        frames.step(channel, &file, sample, VideoStepDirection::Next),
        Ok(ms(275))
    );

    sample.position = ms(75);
    frames.observe(
        channel,
        owner,
        sample,
        Neighbours {
            position: ms(200),
            previous: Some(ms(100)),
            previous_ready: true,
            next: Some(ms(300)),
        },
    );
    assert_eq!(
        frames.step(channel, &file, sample, VideoStepDirection::Previous),
        Ok(Duration::ZERO)
    );

    sample.position = Duration::ZERO;
    frames.observe(
        channel,
        owner,
        sample,
        Neighbours {
            position: ms(100),
            previous: None,
            previous_ready: false,
            next: Some(ms(200)),
        },
    );
    let state = frames.inspect(media, channel, &file, sample).unwrap();
    assert_eq!(state.position_ms, 0.0);
    assert!(state.previous_ready);
    assert_eq!(
        frames.step(channel, &file, sample, VideoStepDirection::Next),
        Ok(ms(75))
    );
}
