use std::path::PathBuf;

use crate::app::{
    CapturedProjectNode, DEFAULT_ENGINE_LOW_FREQUENCY_HZ, DEFAULT_ENGINE_MAX_FREQUENCY_HZ, PREFERENCES_DECL_ID,
    PREFERENCES_ENGINE_DECL_ID, PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID, ProjectGraphCapture, ProjectLifecycle,
    apply_preferences_runtime_limits, capture_sparse_project_file_with_ui_state, ensure_preferences_tree,
    from_sparse_project_json_with_ui_state, insert_sparse_preferences_json, load_sparse_project_file_with_ui_state,
    load_sparse_project_file_with_ui_state_recovering, preferences_data_folder, preferences_engine_low_frequency_hz,
    preferences_engine_max_frequency_hz, sparse_project_file_from_capture, to_sparse_preferences_json_pretty,
    to_sparse_project_json_pretty, to_sparse_project_json_pretty_with_ui_state,
};
use crate::define_node_enum;
use crate::edit::{Edit, NodeTree};
use crate::engine::{Engine, ProjectLoadRecoveryStage};
use crate::node::{Folder, Node};
use crate::parameter::{ParamValue, ParameterEventBehaviour};

define_node_enum!(
    enum PreferencesTestNode {}
);

impl ProjectLifecycle for PreferencesTestNode {}

#[test]
fn immutable_project_capture_matches_the_live_sparse_codec() {
    let root: PreferencesTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    engine.add_node(
        crate::parameter::Parameter::new(
            "Project Value",
            ParamValue::Int(42),
            crate::parameter::ParameterChangeCheck::None,
        )
        .into(),
        None,
    );
    ensure_preferences_tree(&mut engine, "C:/NotInProject");
    engine.apply_edits().expect("test graph should attach");
    let ui_state = serde_json::json!({ "selected_node_ids": [1, 2] });

    let live = capture_sparse_project_file_with_ui_state(&engine, Some(ui_state.clone())).expect("live sparse capture");
    let graph = ProjectGraphCapture::from_engine(&engine).expect("immutable project graph capture");
    let captured = sparse_project_file_from_capture::<PreferencesTestNode>(&graph, Some(ui_state))
        .expect("captured sparse materialization");

    assert_eq!(
        serde_json::to_value(live).expect("live JSON value"),
        serde_json::to_value(captured).expect("captured JSON value")
    );
}

#[test]
fn updating_one_captured_node_copies_only_its_shard() {
    let root: PreferencesTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    engine.add_node(
        crate::parameter::Parameter::new(
            "Project Value",
            ParamValue::Int(1),
            crate::parameter::ParameterChangeCheck::None,
        )
        .into(),
        None,
    );
    engine.apply_edits().expect("project parameter should attach");
    let parameter = engine
        .nodes
        .iter()
        .find_map(|(node_id, node)| (node.node_data().meta.label == "Project Value").then_some(node_id))
        .expect("project parameter");
    let original = ProjectGraphCapture::from_engine(&engine).expect("original capture");
    let mut updated = original.clone();

    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::Int(2),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("parameter update");
    updated.insert(CapturedProjectNode::from_engine(&engine, parameter).expect("updated node capture"));

    assert_eq!(
        original.shared_shards_with(&updated),
        ProjectGraphCapture::shard_count() - 1
    );
}

#[test]
#[ignore = "manual immutable project capture/materialization measurement"]
fn measure_immutable_project_capture_at_scale() {
    use std::time::Instant;

    for node_count in [1_000, 10_000, 100_000] {
        let root: PreferencesTestNode = Folder::new("root").into();
        let mut engine = Engine::new(root);
        let mut tree = NodeTree::new(Folder::new("scale root"));
        for index in 0..node_count {
            tree.push_child(NodeTree::new(Folder::new(format!("node {index}"))));
        }
        engine.edits.push(Edit::AddNodeTree {
            tree,
            parent: engine.root,
            prev_sibling: None,
        });
        engine.apply_edits().expect("scale tree should attach");

        let publish_started = Instant::now();
        let graph = ProjectGraphCapture::from_engine(&engine).expect("scale graph capture");
        let publish = publish_started.elapsed();
        let actor_capture_started = Instant::now();
        let captured = graph.clone();
        let actor_capture = actor_capture_started.elapsed();
        let materialize_started = Instant::now();
        let project = sparse_project_file_from_capture::<PreferencesTestNode>(&captured, None)
            .expect("scale project materialization");
        let materialize = materialize_started.elapsed();
        let encode_started = Instant::now();
        let encoded = serde_json::to_vec_pretty(&project).expect("scale project encoding");
        let encode = encode_started.elapsed();

        eprintln!(
            "project_capture nodes={} publish_ms={} actor_capture_us={} materialize_ms={} encode_ms={} bytes={}",
            graph.len(),
            publish.as_millis(),
            actor_capture.as_micros(),
            materialize.as_millis(),
            encode.as_millis(),
            encoded.len()
        );
    }
}

#[test]
fn preferences_tree_is_saved_separately_from_project_json() {
    let data_folder = "C:/ChataigneData";
    let root: PreferencesTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);

    ensure_preferences_tree(&mut engine, data_folder);
    engine.apply_edits().expect("preferences tree should attach");

    let project_json = to_sparse_project_json_pretty(&engine).expect("project json should encode");
    assert!(
        !project_json.contains("\"preferences\""),
        "project persistence should not include app-data preferences: {project_json}"
    );

    let preferences_json = to_sparse_preferences_json_pretty(&engine)
        .expect("preferences json should encode")
        .expect("preferences tree should be present");
    assert!(preferences_json.contains("Startup and Update"));
    assert!(preferences_json.contains("Save and Load"));
    assert!(preferences_json.contains("Engine"));
    assert!(preferences_json.contains("Interface"));
    assert!(preferences_json.contains("Data Folder"));
    assert!(preferences_json.contains("Engine Max Frequency"));
    assert!(preferences_json.contains("Engine Low Frequency"));
    assert!(preferences_json.contains(data_folder));
}

#[test]
fn preferences_tree_exposes_engine_frequency_defaults() {
    let root: PreferencesTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);

    ensure_preferences_tree(&mut engine, "C:/ChataigneData");
    engine.apply_edits().expect("preferences tree should attach");

    assert_eq!(
        preferences_engine_max_frequency_hz(&engine),
        DEFAULT_ENGINE_MAX_FREQUENCY_HZ
    );
    assert_eq!(
        preferences_engine_low_frequency_hz(&engine),
        DEFAULT_ENGINE_LOW_FREQUENCY_HZ
    );
}

#[test]
fn runtime_limits_follow_live_max_frequency_preference_changes() {
    let root: PreferencesTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    ensure_preferences_tree(&mut engine, "C:/ChataigneData");
    engine.apply_edits().expect("preferences tree should attach");

    let snapshot = engine.process_tree_snapshot();
    let preferences = snapshot
        .find_child_by_decl_id(snapshot.root(), PREFERENCES_DECL_ID)
        .expect("preferences root");
    let engine_preferences = snapshot
        .find_child_by_decl_id(preferences, PREFERENCES_ENGINE_DECL_ID)
        .expect("engine preferences");
    let max_frequency = snapshot
        .find_child_by_decl_id(engine_preferences, PREFERENCES_ENGINE_MAX_FREQUENCY_DECL_ID)
        .expect("max frequency preference");

    engine.edits.push(Edit::SetParam {
        node: max_frequency,
        value: ParamValue::Int(125),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("max frequency preference should change");
    apply_preferences_runtime_limits(&mut engine);

    assert_eq!(preferences_engine_max_frequency_hz(&engine), 125);
    assert_eq!(engine.runtime_limits().max_loop_frequency_hz, 125);
}

#[test]
fn preferences_roundtrip_keeps_data_folder_value() {
    let data_folder = "D:/ReusableData";
    let root: PreferencesTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    ensure_preferences_tree(&mut engine, data_folder);
    engine.apply_edits().expect("preferences tree should attach");

    let preferences_json = to_sparse_preferences_json_pretty(&engine)
        .expect("preferences json should encode")
        .expect("preferences tree should be present");

    let next_root: PreferencesTestNode = Folder::new("root").into();
    let mut next_engine = Engine::new(next_root);
    insert_sparse_preferences_json(&mut next_engine, &preferences_json).expect("preferences json should insert");
    ensure_preferences_tree(&mut next_engine, "fallback");
    next_engine
        .apply_edits()
        .expect("loaded preferences tree should attach");

    assert_eq!(preferences_data_folder(&next_engine), Some(PathBuf::from(data_folder)));
    assert_eq!(
        preferences_engine_max_frequency_hz(&next_engine),
        DEFAULT_ENGINE_MAX_FREQUENCY_HZ
    );
    assert_eq!(
        preferences_engine_low_frequency_hz(&next_engine),
        DEFAULT_ENGINE_LOW_FREQUENCY_HZ
    );
}

#[test]
fn sparse_project_roundtrip_preserves_project_ui_state() {
    let root: PreferencesTestNode = Folder::new("root").into();
    let engine = Engine::new(root);
    let ui_state = serde_json::json!({
        "dock_layout": {
            "panels": {
                "state-machine": {
                    "params": {
                        "__gc_panel_state": {
                            "camera": { "x": 120.0, "y": -42.0, "zoom": 0.8 }
                        }
                    }
                }
            }
        },
        "selected_node_ids": [1, 2, 3]
    });

    let project_json = to_sparse_project_json_pretty_with_ui_state(&engine, Some(ui_state.clone()))
        .expect("project json should encode with UI state");
    let (loaded_engine, loaded_ui_state) = from_sparse_project_json_with_ui_state::<PreferencesTestNode>(&project_json)
        .expect("project json should load with UI state");

    assert_eq!(loaded_ui_state, Some(ui_state.clone()));

    let saved_again = to_sparse_project_json_pretty_with_ui_state(&loaded_engine, loaded_ui_state)
        .expect("loaded project should re-encode with UI state");
    let first: serde_json::Value = serde_json::from_str(&project_json).expect("first project json should parse");
    let second: serde_json::Value = serde_json::from_str(&saved_again).expect("second project json should parse");
    assert_eq!(first, second);
}

#[test]
fn recovering_sparse_file_load_uses_last_complete_atomic_backup() {
    let directory = tempfile::tempdir().expect("temporary project directory should be created");
    let path = directory.path().join("recovery.noisette");
    let root: PreferencesTestNode = Folder::new("root").into();
    let engine = Engine::new(root);
    let ui_state = serde_json::json!({ "selected_node_ids": [7] });
    let valid = to_sparse_project_json_pretty_with_ui_state(&engine, Some(ui_state.clone()))
        .expect("valid sparse project should encode");

    golden_persistence::write_file_atomically_with_recovery(&path, valid.as_bytes())
        .expect("initial save should succeed");
    golden_persistence::write_file_atomically_with_recovery(&path, b"{")
        .expect("atomic replacement should preserve the valid backup");

    assert!(
        load_sparse_project_file_with_ui_state::<PreferencesTestNode, _>(&path).is_err(),
        "strict load must reject the corrupt primary"
    );
    let (_loaded, loaded_ui_state, recovery) =
        load_sparse_project_file_with_ui_state_recovering::<PreferencesTestNode, _>(&path)
            .expect("recovery load should use the last complete backup");
    assert_eq!(loaded_ui_state, Some(ui_state));
    assert!(recovery.problems.iter().any(|problem| {
        problem.stage == ProjectLoadRecoveryStage::ProjectFile
            && problem.message.contains("loaded last complete backup")
    }));
    load_sparse_project_file_with_ui_state::<PreferencesTestNode, _>(&path)
        .expect("approved recovery should atomically repair the primary file");
}

#[test]
fn large_sparse_project_roundtrip_preserves_ten_thousand_node_subtree() {
    let root: PreferencesTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    let mut tree = NodeTree::new(Folder::new("large subtree"));
    for index in 0..10_000 {
        tree.push_child(NodeTree::new(Folder::new(format!("node {index}"))));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine
        .apply_edits()
        .expect("large detached subtree should attach in one edit");

    let json = to_sparse_project_json_pretty(&engine).expect("large project should encode");
    let (loaded, _) =
        from_sparse_project_json_with_ui_state::<PreferencesTestNode>(&json).expect("large project should decode");
    assert_eq!(loaded.nodes.iter().count(), 10_002);
}
