use std::{path::PathBuf, time::Duration};

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour, RangeConstraint},
};

use super::{playback_start_offset, SoundCardCommandRequest, SoundCardPlayFileCommand};

#[test]
fn play_file_command_declares_offset_and_restart_controls() {
    let (engine, command) = materialized_play_file_command();
    let snapshot = engine.process_tree_snapshot();

    let start_offset = direct_child(&snapshot, command, "start_offset");
    let start_offset = snapshot.node(start_offset).expect("Start Offset parameter");
    assert_eq!(start_offset.label, "Start Offset");
    assert_eq!(start_offset.param_value, Some(ParamValue::Float(0.0)));
    assert_eq!(
        start_offset
            .param_constraints
            .as_ref()
            .and_then(|constraints| constraints.range.clone()),
        RangeConstraint::uniform(Some(0.0), None),
    );
    assert_eq!(
        engine
            .nodes
            .get(start_offset.id)
            .and_then(|node| node.engine_param_snapshot())
            .and_then(|snapshot| snapshot.ui_hints.widget),
        Some("time".to_string())
    );

    let force_restart = direct_child(&snapshot, command, "force_restart");
    let force_restart = snapshot.node(force_restart).expect("Force Restart parameter");
    assert_eq!(force_restart.label, "Force Restart");
    assert_eq!(force_restart.param_value, Some(ParamValue::Bool(true)));
}

#[test]
fn play_file_command_builds_typed_offset_and_restart_request() {
    let (mut engine, command) = materialized_play_file_command();
    let snapshot = engine.process_tree_snapshot();
    let audio_file = direct_child(&snapshot, command, "audio_file");
    let start_offset = direct_child(&snapshot, command, "start_offset");
    let force_restart = direct_child(&snapshot, command, "force_restart");

    for (node, value) in [
        (audio_file, ParamValue::File("clip.wav".to_string())),
        (start_offset, ParamValue::Float(1.25)),
        (force_restart, ParamValue::Bool(false)),
    ] {
        engine.edits.push(Edit::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
    engine.apply_edits().expect("Play Audio File parameters should update");

    let snapshot = engine.process_tree_snapshot();
    let request = super::preparation::request_from_snapshot(&snapshot, command).expect("Play Audio File request");

    assert_eq!(
        request,
        SoundCardCommandRequest::PlayFile {
            path: PathBuf::from("clip.wav"),
            playback_id: golden_audio::PlaybackId::new("playback").expect("default playback ID"),
            start_offset: Duration::from_millis(1_250),
            force_restart: false,
        }
    );
}

#[test]
fn playback_start_offset_rejects_invalid_seconds() {
    assert_eq!(
        playback_start_offset(0.5).expect("valid offset"),
        Duration::from_millis(500)
    );
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(
            playback_start_offset(invalid).is_err(),
            "{invalid} must not produce a playback offset"
        );
    }
}

#[test]
fn command_preparation_uses_last_frozen_override_without_changing_authored_values() {
    use crate::app::module_command::CapturedInvocation;
    let (engine, command) = materialized_play_file_command();
    let snapshot = engine.process_tree_snapshot();
    let path = direct_child(&snapshot, command, "audio_file");
    let offset = direct_child(&snapshot, command, "start_offset");
    let event = golden_core::events::CustomEvent::new(
        "test",
        None,
        serde_json::json!({
            "command_id": command,
            "param_overrides": [
                {"param_id": path, "value": ParamValue::File("old.wav".into())},
                {"param_id": path, "value": ParamValue::File("captured.wav".into())},
                {"param_id": offset, "value": ParamValue::Float(2.5)}
            ]
        }),
    );
    let execution = event.try_lease_payload(4096).unwrap();
    let invocation = CapturedInvocation {
        snapshot: snapshot.try_lease().unwrap(),
        execution: Some(execution),
        module: engine.root,
        command,
        override_weight: 4096,
    };
    let SoundCardCommandRequest::PlayFile {
        path: prepared_path,
        start_offset,
        ..
    } = super::prepare_command(&invocation).unwrap()
    else {
        panic!("play request");
    };
    assert_eq!(prepared_path, PathBuf::from("captured.wav"));
    assert_eq!(start_offset, Duration::from_millis(2500));
    assert_eq!(snapshot.node(offset).unwrap().param_value, Some(ParamValue::Float(0.0)));
    assert_ne!(
        snapshot.node(path).unwrap().param_value,
        Some(ParamValue::File("captured.wav".into()))
    );
}

#[test]
fn command_preparation_rejects_oversized_paths_and_source_capacity_before_copying() {
    let (engine, command) = materialized_play_file_command();
    let snapshot = engine.process_tree_snapshot();
    let path = direct_child(&snapshot, command, "audio_file");
    for value in ["x".repeat(64 * 1024 + 1), String::with_capacity(256 * 1024 + 1)] {
        let snapshot = snapshot.with_param_values([(path, ParamValue::File(value))]);
        assert!(super::preparation::request_from_snapshot(&snapshot, command).is_err());
    }
}

fn materialized_play_file_command() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SoundCardPlayFileCommand::create().into(), None);
    for _ in 0..8 {
        engine
            .apply_edits()
            .expect("Play Audio File declarations should materialize");
    }
    let command = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Play Audio File command");
    (engine, command)
}

fn direct_child(snapshot: &golden_core::process_ctx::ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> NodeId {
    snapshot
        .child_ids_slice(parent)
        .iter()
        .copied()
        .find(|child| {
            snapshot.node(*child).is_some_and(|node| {
                node.decl_id
                    .rsplit('/')
                    .next()
                    .is_some_and(|candidate| candidate == decl_id)
            })
        })
        .unwrap_or_else(|| panic!("missing command child '{decl_id}'"))
}
