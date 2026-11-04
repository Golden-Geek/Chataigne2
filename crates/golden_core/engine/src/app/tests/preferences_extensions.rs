use super::*;
use crate::app::{default_preferences_tree, ensure_preferences_defaults, live_preferences_root};
use crate::node::DeclId;

fn defaults(extra: bool) -> NodeTree {
    let mut root = default_preferences_tree("default-folder");
    let mut section = Folder::new("Extension");
    section.node_data_mut().meta.decl_id = DeclId("extension".into());
    let mut tree = NodeTree::new(section);
    for name in if extra { vec!["saved", "new"] } else { vec!["saved"] } {
        let mut parameter =
            crate::parameter::Parameter::new(name, ParamValue::Int(7), crate::parameter::ParameterChangeCheck::None);
        parameter.node_data_mut().meta.decl_id = DeclId(name.into());
        tree.push_child(NodeTree::new(parameter));
    }
    root.push_child(tree);
    root
}

#[test]
fn extending_defaults_preserves_authored_values_and_identity_and_fills_every_missing_branch() {
    let root: PreferencesTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    ensure_preferences_defaults(&mut engine, defaults(false));
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let preferences = live_preferences_root(&engine).unwrap();
    let section = snapshot.find_child_by_decl_id(preferences, "extension").unwrap();
    let saved = snapshot.find_child_by_decl_id(section, "saved").unwrap();
    let uuid = engine.nodes.get(saved).unwrap().node_data().meta.uuid;
    let save_and_load = snapshot.find_child_by_decl_id(preferences, "save_and_load").unwrap();
    let interface = snapshot.find_child_by_decl_id(preferences, "interface").unwrap();
    engine.edits.push(Edit::SetParam {
        node: saved,
        value: ParamValue::Int(42),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::RemoveNode { node: save_and_load });
    engine.edits.push(Edit::RemoveNode { node: interface });
    engine.apply_edits().unwrap();
    ensure_preferences_defaults(&mut engine, defaults(true));
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.find_child_by_decl_id(preferences, "save_and_load").is_some());
    assert!(snapshot.find_child_by_decl_id(preferences, "interface").is_some());
    assert!(snapshot.find_child_by_decl_id(section, "new").is_some());
    let node = engine.nodes.get(saved).unwrap();
    assert_eq!(node.node_data().meta.uuid, uuid);
    assert_eq!(node.engine_param_snapshot().unwrap().value, ParamValue::Int(42));
    assert!(!to_sparse_project_json_pretty(&engine).unwrap().contains("Extension"));
    let before = to_sparse_preferences_json_pretty(&engine).unwrap();
    ensure_preferences_defaults(&mut engine, defaults(true));
    engine.apply_edits().unwrap();
    assert_eq!(to_sparse_preferences_json_pretty(&engine).unwrap(), before);
}
