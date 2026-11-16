use super::*;
use crate::media::video::Neighbours;

#[test]
fn paused_boundary_steps_preserve_neighbours_before_position_feedback_is_delivered() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    apply(
        &mut engine,
        MediaEdit::ConfigureVideoFile {
            reference,
            file: "clip.mp4".into(),
        },
    );
    engine.settle_edit_notifications(16).unwrap();
    let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
    let AppNode::Media(node) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let control = node.video_control().unwrap();
    let channel = crate::media::authoring::channel(&engine, reference).unwrap();
    let ChannelSource::Video { file: Some(file) } = channel.definition.source else {
        panic!()
    };
    let decoder = uuid::Uuid::new_v4();
    control.claim_channel(reference.channel, decoder, file.clone()).unwrap();
    control
        .channel_duration(reference.channel, decoder, Duration::from_secs(1))
        .unwrap();
    let position = parameter(&engine, reference.media.0, "video_current_position_ms");
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    // The runtime has reached its last frame; the engine has not consumed its position wake.
    control
        .configure(
            VideoTransportSettings {
                position_ms: 50.0,
                ..Default::default()
            },
            true,
            None,
        )
        .unwrap();
    let sample = control.sample().unwrap();
    let frame = Neighbours {
        position: Duration::from_millis(50),
        previous: Some(Duration::from_millis(25)),
        previous_ready: true,
        next: None,
    };
    control
        .observe_neighbours(reference.channel, decoder, sample, frame)
        .unwrap();
    assert_eq!(
        engine
            .nodes
            .get(position)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(0.0)
    );
    let snapshot = edit(
        &mut engine,
        reference.media,
        VideoEdit::Step {
            channel: reference.channel,
            direction: VideoStepDirection::Next,
        },
    );
    assert_eq!(snapshot.position_ms, 50.0);
    assert_eq!(control.sample().unwrap().epoch, sample.epoch);
    assert!(control.frame(reference, &file).unwrap().is_some());
    assert_eq!(
        edit(
            &mut engine,
            reference.media,
            VideoEdit::Step {
                channel: reference.channel,
                direction: VideoStepDirection::Previous,
            },
        )
        .position_ms,
        25.0,
        "an immediate opposite Step still uses the prepared neighbour"
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}
