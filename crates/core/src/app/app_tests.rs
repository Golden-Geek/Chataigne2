use std::path::PathBuf;

use crate::app::{
    ProjectLifecycle, ensure_preferences_tree, insert_sparse_preferences_json, preferences_data_folder,
    to_sparse_preferences_json_pretty, to_sparse_project_json_pretty,
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
    assert!(preferences_json.contains("Interface"));
    assert!(preferences_json.contains("Data Folder"));
    assert!(preferences_json.contains(data_folder));
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
}
