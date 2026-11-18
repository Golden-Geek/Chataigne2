use super::*;
use golden_core::node::NodeMetaPatch;

fn subtitles(engine: &mut AppEngine, reference: ChannelReference) -> VideoSubtitleSettings {
    let MediaEditResult::VideoSubtitles {
        reference: actual,
        settings,
        ..
    } = apply(engine, MediaEdit::InspectVideoSubtitles { reference })
    else {
        panic!("expected Video Subtitles");
    };
    assert_eq!(actual, reference);
    settings
}

fn commit(engine: &mut AppEngine, reference: ChannelReference, settings: VideoSubtitleSettings) {
    let MediaEditResult::VideoSubtitles { settings: actual, .. } = apply(
        engine,
        MediaEdit::ConfigureVideoSubtitles {
            reference,
            settings: settings.clone(),
        },
    ) else {
        panic!("expected Video Subtitles");
    };
    assert_eq!(actual, settings);
}

#[test]
fn subtitle_folder_has_typed_settings_and_a_single_history_entry() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let Some(AppNode::MediaChannel(owner)) = engine.nodes.get(engine.node_id_by_uuid(reference.channel.0).unwrap())
    else {
        panic!("Video channel");
    };
    let folder = owner.subtitle_folder(&engine).unwrap();
    let meta = &engine.nodes.get(folder).unwrap().node_data().meta;
    assert_eq!(meta.decl_id.0, "parameters/video_subtitles");
    assert!(meta.can_be_disabled);
    assert!(!meta.enabled);
    let mut current = engine.nodes.get(folder).unwrap().node_data().first_child;
    let mut declared = Vec::new();
    while let Some(id) = current {
        let node = engine.nodes.get(id).unwrap();
        declared.push((
            id,
            node.node_data().meta.label.as_str(),
            node.engine_param_snapshot().unwrap().value,
        ));
        current = node.node_data().next_sibling;
    }
    assert_eq!(
        declared
            .iter()
            .map(|(_, label, value)| (*label, value.clone()))
            .collect::<Vec<_>>(),
        [
            ("Track", ParamValue::Enum("None".into())),
            ("File", ParamValue::File("".into())),
            ("Size", ParamValue::Float(5.0)),
            ("Font", ParamValue::Str("sans-serif".into())),
            ("Positioning", ParamValue::Vec2(0.5, 0.9)),
        ]
    );
    let track = declared[0].0;
    assert_eq!(subtitles(&mut engine, reference), VideoSubtitleSettings::default());
    let history = engine.undo_len();
    let initial = to_sparse_project_json_pretty(&engine).unwrap();
    let configured = VideoSubtitleSettings {
        enabled: true,
        track: VideoSubtitleTrack::File,
        file: "clips/captions.srt".into(),
        size: 7.75,
        font: "Segoe UI".into(),
        position: [0.35, 0.8],
    };
    commit(&mut engine, reference, configured.clone());
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), initial);
    assert!(engine.redo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(subtitles(&mut engine, reference), configured);

    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::PatchMeta {
            node: folder,
            patch: NodeMetaPatch {
                enabled: Some(false),
                ..Default::default()
            }
            .into(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let preserved = VideoSubtitleSettings {
        enabled: false,
        ..configured
    };
    assert_eq!(subtitles(&mut engine, reference), preserved);
    let before_invalid = engine.undo_len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: track,
            value: ParamValue::Enum("Embedded:0003".into()),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(!ack.success, "{ack:?}");
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::ConfigureVideoSubtitles {
                reference,
                settings: VideoSubtitleSettings {
                    enabled: true,
                    size: 200.0,
                    ..preserved.clone()
                },
            },
        )
        .success
    );
    assert_eq!(engine.undo_len(), before_invalid);
    assert_eq!(subtitles(&mut engine, reference), preserved);
}

#[test]
fn video_channels_keep_independent_subtitle_choices_through_undo_reload_and_copy() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::Video);
    let media = first.media;
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media,
            label: Some("Other captions".into()),
        },
    ) else {
        panic!("expected second Video channel");
    };
    let authored = VideoSubtitleSettings {
        enabled: true,
        track: VideoSubtitleTrack::Embedded { index: 9 },
        ..Default::default()
    };
    commit(&mut engine, first, authored.clone());
    assert_eq!(subtitles(&mut engine, second), VideoSubtitleSettings::default());
    let other = VideoSubtitleSettings {
        enabled: true,
        track: VideoSubtitleTrack::File,
        file: "other.vtt".into(),
        size: 10.0,
        ..Default::default()
    };
    commit(&mut engine, second, other.clone());
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(subtitles(&mut engine, first), authored);
    assert_eq!(subtitles(&mut engine, second), VideoSubtitleSettings::default());
    assert!(engine.redo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(subtitles(&mut engine, second), other);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    loaded.settle_edit_notifications(16).unwrap();
    assert_eq!(subtitles(&mut loaded, first), authored);
    assert_eq!(subtitles(&mut loaded, second), other);
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
    let copy = crate::media::catalog::channels(
        &loaded,
        MediaChannelsQuery {
            media,
            start: 0,
            count: 3,
        },
    )
    .unwrap()
    .channels
    .into_iter()
    .map(|row| row.reference)
    .find(|entry| *entry != first && *entry != second)
    .unwrap();
    assert_eq!(subtitles(&mut loaded, copy), authored);
    commit(&mut loaded, copy, VideoSubtitleSettings::default());
    assert_eq!(subtitles(&mut loaded, first), authored);
    assert_eq!(subtitles(&mut loaded, second), other);
    let foreign = create(&mut loaded, MediaKind::Video);
    assert!(
        !invoke(
            &mut loaded,
            MediaEdit::InspectVideoSubtitles {
                reference: ChannelReference {
                    media: foreign.media,
                    channel: first.channel
                },
            }
        )
        .success
    );
}
