use super::*;

#[test]
fn matrix_video_policies_roundtrip_all_combinations_without_changing_transport() {
    for on_activate in [
        VideoOnActivate::PlayFromStart,
        VideoOnActivate::Resume,
        VideoOnActivate::Nothing,
    ] {
        for on_deactivate in [
            VideoOnDeactivate::Stop,
            VideoOnDeactivate::Pause,
            VideoOnDeactivate::Nothing,
        ] {
            let mut engine = fixture();
            let reference = create(&mut engine, MediaKind::Video);
            let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
            let AppNode::Media(node) = engine.nodes.get(owner).unwrap() else {
                panic!()
            };
            let control = node.video_control().unwrap();
            control.prepare(Duration::from_secs(10)).unwrap();
            edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 2500.0 });
            let before = control.sample().unwrap();
            edit(
                &mut engine,
                reference.media,
                VideoEdit::OnActivate { policy: on_activate },
            );
            edit(
                &mut engine,
                reference.media,
                VideoEdit::OnDeactivate { policy: on_deactivate },
            );
            let after = control.sample().unwrap();
            assert_eq!(after.epoch, before.epoch, "editing policy must not seek");
            assert_eq!(after.position, before.position);
            assert!(!after.playing, "editing policy must not launch playback");
            let settings = state(&mut engine, reference.media).settings;
            assert_eq!(settings.on_activate, on_activate);
            assert_eq!(settings.on_deactivate, on_deactivate);
            let json = to_sparse_project_json_pretty(&engine).unwrap();
            let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
            loaded.apply_edits().unwrap();
            loaded.settle_edit_notifications(16).unwrap();
            crate::media::validate_project(&loaded).unwrap();
            let restored = state(&mut loaded, reference.media).settings;
            assert_eq!(restored.on_activate, on_activate);
            assert_eq!(restored.on_deactivate, on_deactivate);
            assert_eq!(restored.mode, VideoPlaybackMode::Paused);
            assert_eq!(restored.position_ms, 0.0, "runtime position is not authored");
        }
    }
}

#[test]
fn generic_policy_parameters_have_exact_choices_history_and_atomic_rejection() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let activate = parameter(&engine, reference.media.0, "video_on_activate");
    let deactivate = parameter(&engine, reference.media.0, "video_on_deactivate");
    for (parameter, expected) in [(activate, "PlayFromStart"), (deactivate, "Stop")] {
        let snapshot = engine.nodes.get(parameter).unwrap().engine_param_snapshot().unwrap();
        assert_eq!(snapshot.value, ParamValue::Enum(expected.into()));
        assert!(!snapshot.read_only);
    }
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: activate,
            value: ParamValue::Enum("Resume".into()),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(
        state(&mut engine, reference.media).settings.on_activate,
        VideoOnActivate::Resume
    );
    assert!(engine.undo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(
        state(&mut engine, reference.media).settings.on_activate,
        VideoOnActivate::PlayFromStart
    );
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    engine.settle_edit_notifications(16).unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    for (node, value) in [
        (activate, ParamValue::Enum("Stop".into())),
        (deactivate, ParamValue::Enum("Resume".into())),
        (activate, ParamValue::Bool(true)),
    ] {
        assert!(
            !apply_project_intent(
                &mut engine,
                UiEditIntent::SetParam {
                    node,
                    value,
                    behaviour: ParameterEventBehaviour::Append
                }
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
    for method in ["setVideoOnActivate", "setVideoOnDeactivate"] {
        assert!(
            crate::media::source_api::operation(
                method,
                &[
                    ParamValue::Str(reference.media.0.0.to_string()),
                    ParamValue::Str("invalid".into())
                ]
            )
            .is_err()
        );
    }
    let solid = create(&mut engine, MediaKind::SolidColor);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media: solid.media,
                edit: VideoEdit::OnDeactivate {
                    policy: VideoOnDeactivate::Pause
                }
            }
        )
        .success
    );
}
