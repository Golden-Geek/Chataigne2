use super::*;
use crate::app::{
    PROJECT_FOLDER_DECL_ID, PROJECT_SETTINGS_DECL_ID, configure_loaded_engine, create_engine,
    create_new_project_engine, default_project_settings_tree, from_sparse_project_json,
};
use crate::node::NodeId;

define_node_enum!(
    enum SettingsTestNode {}
);

impl ProjectLifecycle for SettingsTestNode {
    fn project_settings_tree() -> Option<NodeTree> {
        Some(default_project_settings_tree())
    }
}

fn project_folder(engine: &Engine<SettingsTestNode>) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    let settings = snapshot
        .find_child_by_decl_id(engine.root, PROJECT_SETTINGS_DECL_ID)
        .unwrap();
    snapshot
        .find_child_by_decl_id(settings, PROJECT_FOLDER_DECL_ID)
        .unwrap()
}

#[test]
fn project_defaults_fill_missing_declarations_and_round_trip_typed_values_without_preferences() {
    let legacy = create_engine::<SettingsTestNode>().unwrap();
    let legacy = to_sparse_project_json_pretty(&legacy).unwrap();
    let mut loaded = from_sparse_project_json::<SettingsTestNode>(&legacy).unwrap();
    configure_loaded_engine(&mut loaded).unwrap();
    loaded.apply_edits().unwrap();
    let folder = project_folder(&loaded);
    let parameter = loaded.nodes.get(folder).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(parameter.value, ParamValue::File(String::new()));
    assert_eq!(parameter.ui_hints.widget.as_deref(), Some("directory"));

    let mut engine = create_new_project_engine::<SettingsTestNode>().unwrap();
    ensure_preferences_tree(&mut engine, "external application folder");
    engine.apply_edits().unwrap();
    let folder = project_folder(&engine);
    let identity = engine.nodes.get(folder).unwrap().node_data().meta.uuid;
    let value = ParamValue::File("custom project folder".into());
    engine.edits.push(Edit::SetParam {
        node: folder,
        value: value.clone(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let preferences = to_sparse_preferences_json_pretty(&engine).unwrap().unwrap();
    assert!(!preferences.contains(PROJECT_SETTINGS_DECL_ID));
    assert!(!json.contains("external application folder"));

    let mut restored = from_sparse_project_json::<SettingsTestNode>(&json).unwrap();
    configure_loaded_engine(&mut restored).unwrap();
    restored.apply_edits().unwrap();
    let restored_folder = restored.nodes.get(project_folder(&restored)).unwrap();
    assert_eq!(restored_folder.node_data().meta.uuid, identity);
    assert_eq!(restored_folder.engine_param_snapshot().unwrap().value, value);
}
