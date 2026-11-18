use super::*;

fn fields(engine: &AppEngine, channel: ChannelId) -> [NodeId; 2] {
    let audio = parameter(engine, channel.0, "video_audio");
    let mut child = engine.nodes.get(audio).unwrap().node_data().first_child;
    let audio_file = loop {
        let id = child.expect("Loop Audio File parameter");
        let node = engine.nodes.get(id).unwrap().node_data();
        if node.meta.decl_id.0 == "parameters/video_audio/video_loop_audio" {
            break id;
        }
        child = node.next_sibling;
    };
    [parameter(engine, channel.0, "video_loop_file"), audio_file]
}

fn values(engine: &AppEngine, channel: ChannelId) -> [ParamValue; 2] {
    fields(engine, channel).map(|id| engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value)
}

#[test]
fn separate_loop_files_are_channel_parameters_with_undo_copy_and_reload() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::Video);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: Some("Other loop".into()),
        },
    ) else {
        panic!("new Video channel")
    };
    let defaults = [ParamValue::File("".into()), ParamValue::File("".into())];
    let authored = [
        ParamValue::File("loops/loop.mp4".into()),
        ParamValue::File("loops/loop.wav".into()),
    ];
    for (node, value) in fields(&engine, first.channel).into_iter().zip(authored.iter().cloned()) {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
    }
    assert_eq!(values(&engine, first.channel), authored);
    assert_eq!(values(&engine, second.channel), defaults);
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(
        values(&engine, first.channel),
        [authored[0].clone(), defaults[1].clone()]
    );
    assert!(engine.redo().unwrap());
    engine.settle_edit_notifications(16).unwrap();

    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    loaded.settle_edit_notifications(16).unwrap();
    assert_eq!(values(&loaded, first.channel), authored);
    assert_eq!(values(&loaded, second.channel), defaults);
    let source = loaded.node_id_by_uuid(first.channel.0).unwrap();
    let parent = loaded.nodes.get(source).unwrap().node_data().parent.unwrap();
    let ack = apply_project_intent(
        &mut loaded,
        UiEditIntent::DuplicateNode {
            source,
            new_parent: parent,
            new_prev_sibling: Some(source),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let copied = crate::media::catalog::channels(
        &loaded,
        MediaChannelsQuery {
            media: first.media,
            start: 0,
            count: 3,
        },
    )
    .unwrap()
    .channels
    .into_iter()
    .map(|row| row.reference.channel)
    .find(|id| *id != first.channel && *id != second.channel)
    .unwrap();
    assert_eq!(values(&loaded, copied), authored);
}
