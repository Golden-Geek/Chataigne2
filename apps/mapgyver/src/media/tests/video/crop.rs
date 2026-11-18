use super::*;

fn crop(start_ms: f64, end_ms: Option<f64>) -> VideoEdit {
    VideoEdit::Crop { start_ms, end_ms }
}

fn range(settings: &VideoTransportSettings) -> (f64, Option<f64>) {
    (settings.start_ms, settings.end_ms)
}

#[test]
fn video_crop_parameters_are_atomic_and_persist_with_an_independent_copy() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let media = reference.media;
    let start = parameter(&engine, media.0, "video_start_ms");
    let end = parameter(&engine, media.0, "video_end_ms");
    let read = |engine: &AppEngine, node| engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value;
    assert_eq!(read(&engine, start), ParamValue::Float(0.0));
    assert_eq!(read(&engine, end), ParamValue::Float(0.0));
    assert_eq!(range(&state(&mut engine, media).settings), (0.0, None));

    edit(&mut engine, media, crop(100.25, Some(200.5)));
    let previous = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let change = |engine: &mut AppEngine, node, value| {
        apply_project_intent(
            engine,
            UiEditIntent::SetParam {
                node,
                value: ParamValue::Float(value),
                behaviour: ParameterEventBehaviour::Append,
            },
        )
    };
    assert!(!change(&mut engine, start, 300.75).success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), previous);
    let authored = edit(&mut engine, media, crop(300.75, Some(400.25))).settings;
    assert_eq!(range(&authored), (300.75, Some(400.25)));
    assert_eq!(read(&engine, start), ParamValue::Float(300.75));
    assert_eq!(read(&engine, end), ParamValue::Float(400.25));
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(range(&state(&mut engine, media).settings), (100.25, Some(200.5)));
    assert!(engine.redo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(range(&state(&mut engine, media).settings), range(&authored));

    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    for invalid in [
        crop(100.0, Some(0.0)),
        crop(400.0, Some(400.0)),
        crop(-1.0, None),
        crop(0.0, Some(9_000_000_000_001.0)),
    ] {
        assert!(!invoke(&mut engine, MediaEdit::Video { media, edit: invalid }).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }

    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    loaded.settle_edit_notifications(16).unwrap();
    assert_eq!(range(&state(&mut loaded, media).settings), range(&authored));
    let before = crate::media::catalog::inspect(&loaded).unwrap();
    let source = loaded.node_id_by_uuid(media.0).unwrap();
    let parent = loaded.nodes.get(source).unwrap().node_data().parent.unwrap();
    assert!(
        apply_project_intent(
            &mut loaded,
            UiEditIntent::DuplicateNode {
                source,
                new_parent: parent,
                new_prev_sibling: Some(source),
                initial_params: vec![],
            },
        )
        .success
    );
    let copy = crate::media::catalog::inspect(&loaded)
        .unwrap()
        .media
        .into_iter()
        .find(|media| !before.media.iter().any(|old| old.id == media.id))
        .unwrap()
        .id;
    assert_eq!(range(&state(&mut loaded, copy).settings), range(&authored));
    let copy_end = parameter(&loaded, copy.0, "video_end_ms");
    assert!(change(&mut loaded, copy_end, 0.0).success);
    assert_eq!(range(&state(&mut loaded, copy).settings), (300.75, None));
    assert_eq!(range(&state(&mut loaded, media).settings), range(&authored));
}

#[test]
fn video_crop_rejects_out_of_source_start_and_preserves_end_beyond_the_source() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Video).media;
    let Some(AppNode::Media(owner)) = engine.nodes.get(engine.node_id_by_uuid(media.0).unwrap()) else {
        panic!("Video media owner");
    };
    let control = owner.video_control().unwrap();
    control.prepare(Duration::from_millis(1000)).unwrap();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media,
                edit: crop(1000.0, None)
            }
        )
        .success
    );
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let authored = edit(&mut engine, media, crop(200.0, Some(1200.0)));
    assert_eq!(range(&authored.settings), (200.0, Some(1200.0)));
    control.prepare(Duration::from_millis(600)).unwrap();
    assert_eq!(range(&state(&mut engine, media).settings), range(&authored.settings));

    let start = parameter(&engine, media.0, "video_start_ms");
    let end = parameter(&engine, media.0, "video_end_ms");
    for (node, value, accepted) in [(start, 600.0, false), (end, 700.0, true), (start, 300.0, true)] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node,
                value: ParamValue::Float(value),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert_eq!(ack.success, accepted, "{ack:?}");
    }
    assert_eq!(range(&state(&mut engine, media).settings), (300.0, Some(700.0)));
}
