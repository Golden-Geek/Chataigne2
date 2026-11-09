use super::*;
use crate::{app::AppNode, media::video::protocol::*};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::time::Duration;

fn state(engine: &mut AppEngine, media: MediaId) -> VideoTransportSnapshot {
    let MediaEditResult::Video { snapshot } = apply(engine, MediaEdit::InspectVideo { media }) else {
        panic!()
    };
    snapshot
}
fn edit(engine: &mut AppEngine, media: MediaId, edit: VideoEdit) -> VideoTransportSnapshot {
    let MediaEditResult::Video { snapshot } = apply(engine, MediaEdit::Video { media, edit }) else {
        panic!()
    };
    snapshot
}
fn parameter(engine: &AppEngine, uuid: golden_core::node::NodeUuid, name: &str) -> NodeId {
    let owner = engine.node_id_by_uuid(uuid).unwrap();
    let mut folder = engine.nodes.get(owner).unwrap().node_data().first_child;
    while let Some(id) = folder {
        let node = engine.nodes.get(id).unwrap();
        if crate::parameters::is_parameter_folder(node) {
            let mut child = node.node_data().first_child;
            while let Some(id) = child {
                let node = engine.nodes.get(id).unwrap();
                if node.node_data().meta.decl_id.0 == format!("parameters/{name}") {
                    return id;
                }
                child = node.node_data().next_sibling;
            }
        }
        folder = node.node_data().next_sibling;
    }
    panic!("missing {name}")
}

#[test]
fn video_creation_has_one_transport_real_file_variants_and_independent_project_controls() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    assert_eq!(
        state(&mut engine, reference.media).settings,
        VideoTransportSettings::default()
    );
    assert!(!state(&mut engine, reference.media).ready);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: reference.media,
            label: None,
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::ConfigureVideoFile {
            reference,
            file: "clips/a.mov".into(),
        },
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureVideoFile {
            reference: second,
            file: "clips/b.mp4".into(),
        },
    );
    assert_ne!(
        parameter(&engine, reference.channel.0, "file"),
        parameter(&engine, second.channel.0, "file")
    );
    let id = engine.node_id_by_uuid(reference.media.0).unwrap();
    let Some(AppNode::Media(node)) = engine.nodes.get(id) else {
        panic!()
    };
    let control = node.video_control().unwrap();
    assert!(std::sync::Arc::ptr_eq(&control, &node.video_control().unwrap()));
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(
        crate::media::channel(&loaded, second).unwrap(),
        crate::media::channel(&engine, second).unwrap()
    );
    let id = loaded.node_id_by_uuid(reference.media.0).unwrap();
    let Some(AppNode::Media(node)) = loaded.nodes.get(id) else {
        panic!()
    };
    assert!(
        !std::sync::Arc::ptr_eq(&control, &node.video_control().unwrap()),
        "project replacement creates a new runtime owner"
    );
}

#[test]
fn runtime_playing_and_length_are_read_only_and_never_authored_history() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
    let AppNode::Media(node) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let control = node.video_control().unwrap();
    let length = parameter(&engine, reference.media.0, "video_length_ms");
    let playing = parameter(&engine, reference.media.0, "video_playing");
    let value = |engine: &AppEngine, id| engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value;
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    control.prepare(Duration::from_secs(60)).unwrap();
    control.publish_feedback().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(value(&engine, length), ParamValue::Float(60000.0));
    assert_eq!(value(&engine, playing), ParamValue::Bool(false));
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(engine.undo_len(), history);
    for (node, value) in [(length, ParamValue::Float(123.0)), (playing, ParamValue::Bool(true))] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(!ack.success, "{ack:?}");
    }
    assert_eq!(value(&engine, length), ParamValue::Float(60000.0));
    assert_eq!(engine.undo_len(), history);
    edit(&mut engine, reference.media, VideoEdit::Play);
    let after_play = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(value(&engine, playing), ParamValue::Bool(true));
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after_play);
    assert_eq!(engine.undo_len(), history);
    edit(&mut engine, reference.media, VideoEdit::Pause);
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(value(&engine, playing), ParamValue::Bool(false));
    assert_eq!(value(&engine, length), ParamValue::Float(60000.0));
}

#[test]
fn restored_project_does_not_inherit_old_runtime_feedback() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
    let AppNode::Media(node) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let control = node.video_control().unwrap();
    control.prepare(Duration::from_secs(60)).unwrap();
    control.publish_feedback().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    let loaded_before = to_sparse_project_json_pretty(&loaded).unwrap();
    control.prepare(Duration::from_secs(120)).unwrap();
    control.publish_feedback().unwrap();
    loaded.run_tick(Duration::from_millis(1)).unwrap();
    let length = parameter(&loaded, reference.media.0, "video_length_ms");
    assert_eq!(
        loaded.nodes.get(length).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Float(0.0)
    );
    assert_eq!(to_sparse_project_json_pretty(&loaded).unwrap(), loaded_before);
}

#[test]
fn fresh_seeks_use_current_duration_before_queued_range_feedback_is_applied() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let AppNode::Media(node) = engine
        .nodes
        .get(engine.node_id_by_uuid(reference.media.0).unwrap())
        .unwrap()
    else {
        panic!()
    };
    let control = node.video_control().unwrap();
    let position = parameter(&engine, reference.media.0, "video_current_position_ms");
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for duration in [500, 0, 250] {
        control.prepare(Duration::from_millis(duration)).unwrap();
        // The IO owner has new metadata; no engine tick has delivered the range wake yet.
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: position,
                value: ParamValue::Float(125.0),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
        engine.settle_edit_notifications(16).unwrap();
        assert_eq!(
            control.sample().unwrap().position,
            Duration::from_millis(125.min(duration)),
            "generic seeks use current availability before its queued projection"
        );
        let sought = edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 200.0 });
        assert_eq!(sought.position_ms, 200.min(duration) as f64, "typed atomic seeks agree");
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

#[test]
fn current_position_is_bounded_transient_and_shared_by_generic_and_script_seeks() {
    use golden_core::parameter::RangeConstraint;
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
    let AppNode::Media(node) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let control = node.video_control().unwrap();
    let position = parameter(&engine, reference.media.0, "video_current_position_ms");
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let snapshot = engine.nodes.get(position).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(snapshot.ui_hints.widget.as_deref(), Some("time_slider"));
    assert_eq!(
        snapshot.constraints.range,
        RangeConstraint::uniform(Some(0.0), Some(0.0))
    );
    assert_eq!(
        engine.nodes.get(position).unwrap().node_data().meta.label,
        "Current Position"
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert_eq!(
        edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 999.0 }).position_ms,
        0.0
    );
    control.prepare(Duration::from_millis(500)).unwrap();
    control.publish_feedback().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    for (requested, expected) in [(-10.0, 0.0), (125.0, 125.0), (999.0, 500.0)] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: position,
                value: ParamValue::Float(requested),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
        engine.settle_edit_notifications(16).unwrap();
        assert_eq!(
            control.sample().unwrap().position,
            Duration::from_secs_f64(expected / 1000.0)
        );
        assert_eq!(
            engine
                .nodes
                .get(position)
                .unwrap()
                .engine_param_snapshot()
                .unwrap()
                .value,
            ParamValue::Float(expected)
        );
    }
    let script = crate::media::source_api::operation(
        "seekVideo",
        &[
            ParamValue::Str(reference.media.0.0.to_string()),
            ParamValue::Float(125.0),
        ],
    )
    .unwrap()
    .unwrap();
    apply(&mut engine, script);
    let before = control.sample().unwrap().epoch;
    edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 125.0 });
    assert_ne!(
        control.sample().unwrap().epoch,
        before,
        "an identical seek remains a real intent"
    );
    control.prepare(Duration::from_millis(100)).unwrap();
    control.publish_feedback().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(100));
    let snapshot = engine.nodes.get(position).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(snapshot.value, ParamValue::Float(100.0));
    assert_eq!(
        snapshot.constraints.range,
        RangeConstraint::uniform(Some(0.0), Some(100.0))
    );
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert_eq!(engine.undo_len(), history);
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(state(&mut loaded, reference.media).position_ms, 0.0);
    let loaded_position = parameter(&loaded, reference.media.0, "video_current_position_ms");
    assert_eq!(
        loaded
            .nodes
            .get(loaded_position)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(0.0)
    );
}

#[test]
fn seek_command_keeps_its_authored_argument_and_clamps_runtime_execution() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let AppNode::Media(node) = engine
        .nodes
        .get(engine.node_id_by_uuid(reference.media.0).unwrap())
        .unwrap()
    else {
        panic!()
    };
    let control = node.video_control().unwrap();
    control.prepare(Duration::from_millis(500)).unwrap();
    let MediaEditResult::CommandCreated { command } = apply(
        &mut engine,
        MediaEdit::CreateCommand {
            command_kind: crate::media::commands::MediaCommandKind::VideoSeek,
            label: None,
        },
    ) else {
        panic!()
    };
    for (field, value) in [
        (
            "media",
            ParamValue::Reference(golden_core::node::NodeReference::new(reference.media.0)),
        ),
        ("video_position_ms", ParamValue::Float(999.0)),
    ] {
        let node = parameter(&engine, command, field);
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
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    apply(&mut engine, MediaEdit::ExecuteCommand { command });
    assert_eq!(control.sample().unwrap().position, Duration::from_millis(500));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let argument = parameter(&engine, command, "video_position_ms");
    assert_eq!(
        engine
            .nodes
            .get(argument)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(999.0)
    );
}

#[test]
fn current_position_follows_play_pause_and_stop_without_render_demand_or_authored_changes() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let AppNode::Media(node) = engine
        .nodes
        .get(engine.node_id_by_uuid(reference.media.0).unwrap())
        .unwrap()
    else {
        panic!()
    };
    let control = node.video_control().unwrap();
    let id = parameter(&engine, reference.media.0, "video_current_position_ms");
    let visible = |engine: &AppEngine| match engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value {
        ParamValue::Float(value) => value,
        _ => panic!(),
    };
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    control.prepare(Duration::from_secs(60)).unwrap();
    edit(&mut engine, reference.media, VideoEdit::Play);
    std::thread::sleep(Duration::from_millis(25));
    control.publish_feedback().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let first = visible(&engine);
    assert!(first > 0.0);
    std::thread::sleep(Duration::from_millis(25));
    control.publish_feedback().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert!(visible(&engine) > first);
    edit(&mut engine, reference.media, VideoEdit::Pause);
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let paused = visible(&engine);
    assert!((paused - state(&mut engine, reference.media).position_ms).abs() < 0.001);
    std::thread::sleep(Duration::from_millis(25));
    control.publish_feedback().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(visible(&engine), paused);
    edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 125.0 });
    edit(&mut engine, reference.media, VideoEdit::Play);
    std::thread::sleep(Duration::from_millis(25));
    control.publish_feedback().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert!(
        visible(&engine) > 125.0,
        "feedback acknowledges the latest manual input token"
    );
    edit(&mut engine, reference.media, VideoEdit::Stop);
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(visible(&engine), 0.0);
    assert_eq!(
        state(&mut engine, reference.media).settings.mode,
        VideoPlaybackMode::Stopped
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

#[test]
fn inspector_transport_triggers_do_not_author_or_replay_playback_mode() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (name, expected) in [
        ("video_play", VideoPlaybackMode::Playing),
        ("video_pause", VideoPlaybackMode::Paused),
        ("video_stop", VideoPlaybackMode::Stopped),
        ("video_play", VideoPlaybackMode::Playing),
        ("video_play", VideoPlaybackMode::Playing),
    ] {
        let target = parameter(&engine, reference.media.0, name);
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: target,
                value: ParamValue::Trigger(),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(state(&mut engine, reference.media).settings.mode, expected);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    assert!(!before.contains("video_mode"));
    let mut loaded = from_sparse_project_json::<AppNode>(&before).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(
        state(&mut loaded, reference.media).settings.mode,
        VideoPlaybackMode::Paused
    );
}

#[test]
fn load_policy_roundtrips_but_old_pause_does_not_override_a_new_media_lifetime() {
    for enabled in [false, true] {
        let mut engine = fixture();
        let reference = create(&mut engine, MediaKind::Video);
        assert!(!state(&mut engine, reference.media).settings.play_at_load);
        edit(&mut engine, reference.media, VideoEdit::PlayAtLoad { enabled });
        edit(&mut engine, reference.media, VideoEdit::Pause);
        assert_eq!(
            state(&mut engine, reference.media).settings.mode,
            VideoPlaybackMode::Paused
        );
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        assert_eq!(state(&mut loaded, reference.media).settings.play_at_load, enabled);
        golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
        let owner = loaded.node_id_by_uuid(reference.media.0).unwrap();
        let AppNode::Media(node) = loaded.nodes.get(owner).unwrap() else {
            panic!()
        };
        let control = node.video_control().unwrap();
        assert!(!control.sample().unwrap().playing);
        control.prepare(Duration::from_secs(60)).unwrap();
        assert_eq!(control.sample().unwrap().playing, enabled);
        edit(&mut loaded, reference.media, VideoEdit::Pause);
        assert!(!control.sample().unwrap().playing);
    }
}

#[test]
fn inspector_speed_uses_the_same_transport_and_rejects_invalid_values_atomically() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let speed = parameter(&engine, reference.media.0, "video_speed");
    for value in [-16.0, 0.0, 16.0, -0.5] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: speed,
                value: ParamValue::Float(value),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(state(&mut engine, reference.media).settings.speed, value);
    }
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for value in [-16.01, 16.01, f64::NAN, f64::INFINITY] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: speed,
                value: ParamValue::Float(value),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(!ack.success, "{ack:?}");
        assert_eq!(state(&mut engine, reference.media).settings.speed, -0.5);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    let mut loaded = from_sparse_project_json::<AppNode>(&before).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(state(&mut loaded, reference.media).settings.speed, -0.5);
}

#[test]
fn typed_transport_intents_share_parameters_history_and_truthful_runtime_readiness() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    edit(&mut engine, reference.media, VideoEdit::Speed { speed: 0.5 });
    edit(&mut engine, reference.media, VideoEdit::Loop { looping: true });
    let unknown = edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 1250.25 });
    assert_eq!(
        unknown.position_ms, 0.0,
        "unknown duration has a zero-length seek range"
    );
    let playing = edit(&mut engine, reference.media, VideoEdit::Play);
    assert_eq!(playing.settings.mode, VideoPlaybackMode::Playing);
    assert!(
        !playing.ready && !playing.playing,
        "accepted intent is not a decoder readiness claim"
    );
    let id = engine.node_id_by_uuid(reference.media.0).unwrap();
    let Some(AppNode::Media(node)) = engine.nodes.get(id) else {
        panic!()
    };
    let control = node.video_control().unwrap();
    control.prepare(Duration::from_secs(3600)).unwrap();
    let ready = state(&mut engine, reference.media);
    assert!(ready.ready && ready.playing);
    let sought = edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 1250.25 });
    assert!(sought.position_ms >= 1250.25);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let stopped = edit(&mut engine, reference.media, VideoEdit::Stop);
    assert_eq!(stopped.settings.position_ms, 0.0);
    assert_eq!(stopped.position_ms, 0.0);
    assert!(!stopped.playing);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(
        engine.undo_len(),
        history,
        "Stop is a runtime trigger, not an authored edit"
    );
    assert_eq!(
        state(&mut engine, reference.media).settings.mode,
        VideoPlaybackMode::Stopped
    );
    let sought = edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 500.5 });
    assert_eq!(sought.settings.mode, VideoPlaybackMode::Paused);
    assert!((sought.position_ms - 500.5).abs() < 0.00001);
}

#[test]
fn generic_stop_trigger_and_invalid_writes_use_the_same_backend() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 5000.0 });
    let stop = parameter(&engine, reference.media.0, "video_stop");
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: stop,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(state(&mut engine, reference.media).settings.position_ms, 0.0);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Speed { speed: 17.0 }
            }
        )
        .success
    );
    let clamped = edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: -1.0 });
    assert_eq!(clamped.position_ms, 0.0);
    let file = parameter(&engine, reference.channel.0, "file");
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: file,
            value: ParamValue::File("bad\0movie.mp4".into()),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn script_projection_and_unprepared_steps_use_the_exact_owner_without_partial_history() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let operation = crate::media::source_api::operation(
        "seekVideo",
        &[
            ParamValue::Str(reference.media.0.0.to_string()),
            ParamValue::Float(750.25),
        ],
    )
    .unwrap()
    .unwrap();
    apply(&mut engine, operation);
    assert_eq!(
        state(&mut engine, reference.media).settings.position_ms,
        0.0,
        "script seeks use the same bounded unknown-duration policy"
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for direction in [VideoStepDirection::Next, VideoStepDirection::Previous] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Video {
                    media: reference.media,
                    edit: VideoEdit::Step {
                        channel: reference.channel,
                        direction
                    }
                }
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
        assert_eq!(engine.undo_len(), history);
    }
    let wrong = create(&mut engine, MediaKind::Picture);
    let before = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media: wrong.media,
                edit: VideoEdit::Play
            }
        )
        .success
    );
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Step {
                    channel: wrong.channel,
                    direction: VideoStepDirection::Next
                }
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), before);
}
