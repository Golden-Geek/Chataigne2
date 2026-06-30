use golden_core::node::{Folder, Node, NodeId};
use golden_core::parameter::ParamValue;
use golden_core::ui_sync::UiEditIntent;

use super::{ConditionManager, ConsequencesManager, FilterChainManager, InputsManager, OutputsManager};

#[test]
fn managed_nodes_have_correct_type_ids() {
    assert_eq!(ConditionManager::NODE_TYPE, "sm_condition_manager");
    assert_eq!(ConsequencesManager::NODE_TYPE, "sm_consequences_manager");
    assert_eq!(InputsManager::NODE_TYPE, "sm_inputs_manager");
    assert_eq!(FilterChainManager::NODE_TYPE, "sm_filter_chain_manager");
    assert_eq!(OutputsManager::NODE_TYPE, "sm_outputs_manager");
}

#[test]
fn managed_nodes_are_non_removable_by_default() {
    let cm = ConditionManager::new();
    let csq = ConsequencesManager::new();
    let inp = InputsManager::new();
    let fc = FilterChainManager::new();
    let out = OutputsManager::new();

    for (label, perm) in [
        ("ConditionManager", &cm.node_data().meta.user_permissions),
        ("ConsequencesManager", &csq.node_data().meta.user_permissions),
        ("InputsManager", &inp.node_data().meta.user_permissions),
        ("FilterChainManager", &fc.node_data().meta.user_permissions),
        ("OutputsManager", &out.node_data().meta.user_permissions),
    ] {
        assert!(!perm.can_remove_and_duplicate, "{label} should not be removable");
        assert!(!perm.can_edit_name, "{label} should not have editable name");
    }
}

#[test]
fn condition_manager_operator_defaults_to_all() {
    let cm = ConditionManager::new();
    assert_eq!(cm.operator.get_ref().as_str(), "all");
    assert_eq!(*cm.operator_count.get_ref(), 1.0);
    assert!(!*cm.valid.get_ref());
}

#[test]
fn condition_manager_accepts_all_operators() {
    let mut cm = ConditionManager::new();
    for op in ["all", "any", "none", "at_least", "exactly"] {
        cm.operator.apply_runtime_value(&ParamValue::Str(op.to_string()));
        assert_eq!(cm.operator.get_ref().as_str(), op);
    }
}

#[test]
fn condition_manager_operator_visibility_tracks_condition_items() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ConditionManager::new().into(), None);
    engine
        .apply_edits()
        .expect("condition manager should attach");
    let manager = node_by_type(&engine, ConditionManager::NODE_TYPE);

    assert_operator_visibility(&engine, manager, false);

    create_input_value_condition(&mut engine, manager);
    assert_operator_visibility(&engine, manager, false);

    create_input_value_condition(&mut engine, manager);
    assert_operator_visibility(&engine, manager, true);
}

fn create_input_value_condition(engine: &mut crate::app::AppEngine, parent: NodeId) {
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: crate::app::InputValueCondition::NODE_TYPE.to_owned(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        ack.success,
        "input value condition should attach to condition manager: {ack:?}"
    );
}

fn node_by_type(engine: &crate::app::AppEngine, node_type: &str) -> NodeId {
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == node_type)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("{node_type} should exist"))
}

fn assert_operator_visibility(
    engine: &crate::app::AppEngine,
    manager: NodeId,
    expected: bool,
) {
    let snapshot = engine.process_tree_snapshot();
    let operator = snapshot
        .find_child_by_decl_id(manager, "operator")
        .expect("operator parameter should exist");
    assert_eq!(
        snapshot
            .node(operator)
            .expect("operator parameter should be in the snapshot")
            .presentation
            .show_in_inspector_content,
        expected
    );
}
