use golden_core::node::{Folder, Node, NodeId};
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
use golden_core::ui_sync::UiEditIntent;

use super::{
    ConditionManager, ConsequencesManager, FilterChainManager, InputsManager, OutputGroup,
    OutputsManager,
};

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

#[test]
fn output_containers_use_periodic_updates_for_delayed_outputs() {
    assert_eq!(OutputsManager::new().execution_rule().update_rate, Some(60));
    assert_eq!(OutputGroup::new().execution_rule().update_rate, Some(60));
}

#[test]
fn output_controls_live_in_collapsed_advanced_folder() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(OutputsManager::new().into(), None);
    engine.apply_edits().expect("outputs manager should attach");
    let manager = node_by_type(&engine, OutputsManager::NODE_TYPE);
    let snapshot = engine.process_tree_snapshot();
    let advanced = snapshot
        .find_child_by_decl_id(manager, "advanced")
        .expect("advanced folder should exist");

    assert!(
        snapshot
            .node(advanced)
            .expect("advanced folder should be in the snapshot")
            .presentation
            .collapsed,
        "output advanced folder should start collapsed"
    );
    assert!(snapshot.find_child_by_decl_id(manager, "delay").is_none());
    assert!(snapshot.find_child_by_decl_id(manager, "stagger").is_none());
    assert!(
        snapshot
            .find_child_by_decl_id(manager, "cancel_on_trigger")
            .is_none()
    );
    assert_output_control_visibility(&engine, manager, "delay", true);
    assert_output_control_visibility(&engine, manager, "stagger", false);
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", false);
}

#[test]
fn output_stagger_visibility_tracks_output_count() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(OutputsManager::new().into(), None);
    engine.apply_edits().expect("outputs manager should attach");
    let manager = node_by_type(&engine, OutputsManager::NODE_TYPE);

    assert_output_control_visibility(&engine, manager, "stagger", false);

    create_generic_log_output(&mut engine, manager);
    assert_output_control_visibility(&engine, manager, "stagger", false);

    create_generic_log_output(&mut engine, manager);
    assert_output_control_visibility(&engine, manager, "stagger", true);
}

#[test]
fn output_cancel_visibility_tracks_active_timing() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(OutputsManager::new().into(), None);
    engine.apply_edits().expect("outputs manager should attach");
    let manager = node_by_type(&engine, OutputsManager::NODE_TYPE);

    create_generic_log_output(&mut engine, manager);
    create_generic_log_output(&mut engine, manager);
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", false);

    let delay = output_control(&engine, manager, "delay");
    set_param(&mut engine, delay, ParamValue::Float(0.25));
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", true);

    set_param(&mut engine, delay, ParamValue::Float(0.0));
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", false);

    let stagger = output_control(&engine, manager, "stagger");
    set_param(&mut engine, stagger, ParamValue::Float(0.25));
    assert_output_control_visibility(&engine, manager, "cancel_on_trigger", true);
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

fn create_generic_log_output(engine: &mut crate::app::AppEngine, parent: NodeId) {
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: crate::app::systems_alchemist_generic_commands::GenericLogCommand::NODE_TYPE
            .to_owned(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        ack.success,
        "generic log command should attach to outputs manager: {ack:?}"
    );
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "parameter change should apply: {ack:?}");
}

fn node_by_type(engine: &crate::app::AppEngine, node_type: &str) -> NodeId {
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == node_type)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("{node_type} should exist"))
}

fn output_control(engine: &crate::app::AppEngine, manager: NodeId, decl_id: &str) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    let advanced = snapshot
        .find_child_by_decl_id(manager, "advanced")
        .expect("advanced folder should exist");
    snapshot
        .find_child_by_decl_id(advanced, decl_id)
        .unwrap_or_else(|| panic!("{decl_id} output control should exist"))
}

fn assert_output_control_visibility(
    engine: &crate::app::AppEngine,
    manager: NodeId,
    decl_id: &str,
    expected: bool,
) {
    let snapshot = engine.process_tree_snapshot();
    let control = output_control(engine, manager, decl_id);
    assert_eq!(
        snapshot
            .node(control)
            .unwrap_or_else(|| panic!("{decl_id} output control should be in the snapshot"))
            .presentation
            .show_in_inspector_content,
        expected,
        "{decl_id} visibility should be {expected}"
    );
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
