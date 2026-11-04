use super::*;
use golden_engine::{app::default_preferences_tree, edit::NodeTree, node::DeclId};

define_node_enum!(
    enum PreferencesExtensionNode {}
);
impl ProjectLifecycle for PreferencesExtensionNode {
    fn preferences_tree(default_data_folder: String) -> NodeTree {
        let mut root = default_preferences_tree(default_data_folder);
        let mut section = Folder::new("App options");
        section.node_data_mut().meta.decl_id = DeclId("app_options".into());
        root.push_child(NodeTree::new(section));
        root
    }
}

#[test]
fn shared_host_merges_app_registered_preferences_without_changing_the_default_core_contract() {
    let mut engine = golden_engine::app::create_engine::<PreferencesExtensionNode>().unwrap();
    super::super::load_preferences_into_engine(&mut engine, None).unwrap();
    prepare_engine_for_runtime(&mut engine).unwrap();
    let snapshot = engine.process_tree_snapshot();
    let root = golden_engine::app::live_preferences_root(&engine).unwrap();
    assert!(snapshot.find_child_by_decl_id(root, "app_options").is_some());
    assert!(snapshot.find_child_by_decl_id(root, "engine").is_some());
    assert!(snapshot.find_child_by_decl_id(root, "save_and_load").is_some());
    let uuid = engine.nodes.get(root).unwrap().node_data().meta.uuid;
    super::super::load_preferences_into_engine(&mut engine, None).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(engine.nodes.get(root).unwrap().node_data().meta.uuid, uuid);
}
