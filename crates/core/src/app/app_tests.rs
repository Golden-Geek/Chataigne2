use std::path::PathBuf;

use crate::app::{
    DEFAULT_ENGINE_LOW_FREQUENCY_HZ, DEFAULT_ENGINE_MAX_FREQUENCY_HZ, ProjectLifecycle, ensure_preferences_tree,
    from_sparse_project_json_with_ui_state, insert_sparse_preferences_json, preferences_data_folder,
    preferences_engine_low_frequency_hz, preferences_engine_max_frequency_hz, to_sparse_preferences_json_pretty,
    to_sparse_project_json_pretty, to_sparse_project_json_pretty_with_ui_state,
};
use crate::define_node_enum;
use crate::engine::Engine;
use crate::node::Folder;

define_node_enum!(
    enum PreferencesTestNode {}
);

impl ProjectLifecycle for PreferencesTestNode {}

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
