use super::*;

#[test]
fn intro_and_smooth_loop_controls_survive_disabled_history_copy_and_reload() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let overlap = parameter(&engine, reference.media.0, "video_auto_smooth_loop_ms");
    let loop_start = parameter(&engine, reference.media.0, "video_loop_start_ms");
    let looping = parameter(&engine, reference.media.0, "video_looping");
    let overlap_enabled = |engine: &AppEngine, node| engine.nodes.get(node).unwrap().node_data().meta.enabled;
    assert!(!overlap_enabled(&engine, overlap));
    assert!(!overlap_enabled(&engine, loop_start));
    assert!(!engine.nodes.get(overlap).unwrap().node_data().meta.can_be_disabled);
    assert_eq!(state(&mut engine, reference.media).settings.auto_smooth_loop_ms, 0.0);

    // Typed operations may preconfigure the duration while the Inspector field is disabled.
    let authored = edit(
        &mut engine,
        reference.media,
        VideoEdit::AutoSmoothLoop { duration_ms: 765.25 },
    );
    assert_eq!(authored.settings.auto_smooth_loop_ms, 765.25);
    let authored = edit(&mut engine, reference.media, VideoEdit::LoopStart { start_ms: 4250.0 });
    assert_eq!(authored.settings.loop_start_ms, 4250.0);
    assert!(!authored.settings.looping);
    assert!(!overlap_enabled(&engine, overlap));
    let history = engine.undo_len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: looping,
            value: ParamValue::Bool(true),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(engine.undo_len(), history + 1);
    assert!(overlap_enabled(&engine, overlap));
    assert!(overlap_enabled(&engine, loop_start));
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert!(!overlap_enabled(&engine, overlap));
    assert!(!overlap_enabled(&engine, loop_start));
    assert_eq!(state(&mut engine, reference.media).settings.auto_smooth_loop_ms, 765.25);
    assert!(engine.redo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert!(overlap_enabled(&engine, overlap));
    assert!(overlap_enabled(&engine, loop_start));

    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: overlap,
            value: ParamValue::Float(550.75),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    engine.settle_edit_notifications(16).unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    edit(
        &mut engine,
        reference.media,
        VideoEdit::Crop {
            start_ms: 1000.0,
            end_ms: Some(7500.0),
        },
    );
    let invalid = invoke(
        &mut engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::LoopStart { start_ms: 7500.0 },
        },
    );
    assert!(!invalid.success);
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    for duration_ms in [-1.0, 9_000_000_000_001.0] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Video {
                    media: reference.media,
                    edit: VideoEdit::AutoSmoothLoop { duration_ms }
                },
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }

    edit(&mut engine, reference.media, VideoEdit::Loop { looping: false });
    assert_eq!(
        parameter(&engine, reference.media.0, "video_auto_smooth_loop_ms"),
        overlap
    );
    assert!(!overlap_enabled(&engine, overlap));
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    loaded.settle_edit_notifications(16).unwrap();
    let restored = state(&mut loaded, reference.media);
    assert!(!restored.settings.looping);
    assert_eq!(restored.settings.auto_smooth_loop_ms, 550.75);
    assert_eq!(restored.settings.loop_start_ms, 4250.0);
    assert!(!overlap_enabled(
        &loaded,
        parameter(&loaded, reference.media.0, "video_auto_smooth_loop_ms")
    ));
    let before = crate::media::catalog::inspect(&loaded).unwrap();
    let source = loaded.node_id_by_uuid(reference.media.0).unwrap();
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
    let copy = crate::media::catalog::inspect(&loaded)
        .unwrap()
        .media
        .into_iter()
        .find(|media| !before.media.iter().any(|old| old.id == media.id))
        .unwrap()
        .id;
    assert_eq!(state(&mut loaded, copy).settings.auto_smooth_loop_ms, 550.75);
    assert_eq!(state(&mut loaded, copy).settings.loop_start_ms, 4250.0);
    edit(&mut loaded, copy, VideoEdit::Loop { looping: true });
    edit(&mut loaded, copy, VideoEdit::AutoSmoothLoop { duration_ms: 200.0 });
    edit(&mut loaded, copy, VideoEdit::LoopStart { start_ms: 1000.0 });
    assert!(overlap_enabled(
        &loaded,
        parameter(&loaded, copy.0, "video_auto_smooth_loop_ms")
    ));
    assert_eq!(state(&mut loaded, reference.media).settings, restored.settings);
}
