use golden_core::node::{Folder, Node, NodeId};
use golden_core::parameter::ParamValue;
use golden_core::ui_sync::UiEditIntent;
use std::sync::Arc;

use super::{ConditionGroup, InputNodeCondition, InputValueCondition, ScriptCondition};
use super::compiler::compile_manager_condition;

// ─── InputValueCondition ─────────────────────────────────────────────────────

#[test]
fn input_value_condition_type_id() {
    assert_eq!(InputValueCondition::NODE_TYPE, "sm_input_value_condition");
}

#[test]
fn input_value_condition_defaults() {
    let c = InputValueCondition::new();
    assert_eq!(c.node_data().meta.label, "Input Value");
    assert!(!*c.toggle_mode.get_ref());
    assert_eq!(*c.validation_delay_s.get_ref(), 0.0);
    assert_eq!(*c.invalidation_delay_s.get_ref(), 0.0);
    assert_eq!(c.source_projection.get_ref().as_str(), "none");
    assert_eq!(c.comparator.get_ref().as_str(), "equal");
    assert_eq!(*c.reference.get_ref(), 0.0);
    assert_eq!(*c.reference_max.get_ref(), 1.0);
    assert!(*c.reference_bool.get_ref());
    assert!(c.reference_string.get_ref().is_empty());
    assert!(matches!(c.reference_vec2.get_ref(), ParamValue::Vec2(0.0, 0.0)));
    assert!(matches!(
        c.reference_vec3.get_ref(),
        ParamValue::Vec3(0.0, 0.0, 0.0)
    ));
    assert!(matches!(
        c.reference_color.get_ref(),
        ParamValue::Color(0.0, 0.0, 0.0, 1.0)
    ));
}

#[test]
fn input_value_condition_has_value_changed_comparator() {
    let mut c = InputValueCondition::new();
    c.comparator.apply_runtime_value(&ParamValue::Str("value_changed".into()));
    assert_eq!(c.comparator.get_ref().as_str(), "value_changed");
}

#[test]
fn input_value_condition_accepts_vector_and_color_comparators() {
    let mut c = InputValueCondition::new();
    for op in [
        "magnitude_greater_than",
        "magnitude_less_than",
        "speed_greater_than",
        "speed_less_than",
        "abs_speed_greater_than",
        "abs_speed_less_than",
        "luminance_greater_than",
        "luminance_less_than",
        "alpha_greater_than",
        "alpha_less_than",
    ] {
        c.comparator.apply_runtime_value(&ParamValue::Str(op.to_owned()));
        assert_eq!(c.comparator.get_ref().as_str(), op);
    }
}

#[test]
fn input_value_condition_is_sm_condition_item() {
    use crate::app::declared_user_item_type_matches;
    assert!(declared_user_item_type_matches("sm_input_value_condition", "sm_condition"));
    assert!(!declared_user_item_type_matches("sm_input_value_condition", "sm_consequence"));
}

// ─── InputNodeCondition ──────────────────────────────────────────────────────

#[test]
fn input_node_condition_type_id() {
    assert_eq!(InputNodeCondition::NODE_TYPE, "sm_input_node_condition");
}

#[test]
fn input_node_condition_defaults() {
    let c = InputNodeCondition::new();
    assert_eq!(c.node_data().meta.label, "Input Node");
    assert!(!*c.toggle_mode.get_ref());
    assert_eq!(*c.validation_delay_s.get_ref(), 0.0);
    assert_eq!(*c.invalidation_delay_s.get_ref(), 0.0);
    assert!(c.endpoint_id.get_ref().is_empty());
    assert_eq!(c.comparator.get_ref().as_str(), "equal");
    assert_eq!(*c.reference.get_ref(), 0.0);
    assert_eq!(*c.reference_max.get_ref(), 1.0);
    assert!(c.reference_string.get_ref().is_empty());
}

#[test]
fn input_node_condition_is_sm_condition_item() {
    use crate::app::declared_user_item_type_matches;
    assert!(declared_user_item_type_matches("sm_input_node_condition", "sm_condition"));
}

// ─── ScriptCondition ─────────────────────────────────────────────────────────

#[test]
fn script_condition_type_id() {
    assert_eq!(ScriptCondition::NODE_TYPE, "sm_script_condition");
}

#[test]
fn script_condition_defaults() {
    let c = ScriptCondition::new();
    assert_eq!(c.node_data().meta.label, "Script");
    assert!(!*c.toggle_mode.get_ref());
    assert_eq!(*c.validation_delay_s.get_ref(), 0.0);
    assert_eq!(*c.invalidation_delay_s.get_ref(), 0.0);
    assert!(c.script.get_ref().is_empty());
}

#[test]
fn script_condition_is_sm_condition_item() {
    use crate::app::declared_user_item_type_matches;
    assert!(declared_user_item_type_matches("sm_script_condition", "sm_condition"));
}

// ─── Common fields shared by all condition types ──────────────────────────────

#[test]
fn all_condition_types_default_toggle_mode_off() {
    assert!(!*InputValueCondition::new().toggle_mode.get_ref());
    assert!(!*InputNodeCondition::new().toggle_mode.get_ref());
    assert!(!*ScriptCondition::new().toggle_mode.get_ref());
    assert!(!*ConditionGroup::new().toggle_mode.get_ref());
}

// ─── ConditionGroup ───────────────────────────────────────────────────────────

#[test]
fn condition_group_type_id() {
    assert_eq!(ConditionGroup::NODE_TYPE, "sm_condition_group");
}

#[test]
fn condition_group_defaults() {
    let g = ConditionGroup::new();
    assert!(!*g.toggle_mode.get_ref());
    assert_eq!(g.operator.get_ref().as_str(), "all");
    assert_eq!(*g.operator_count.get_ref(), 1.0);
    assert_eq!(*g.validation_delay_s.get_ref(), 0.0);
    assert_eq!(*g.invalidation_delay_s.get_ref(), 0.0);
}

#[test]
fn condition_group_accepts_extended_operators() {
    use golden_core::parameter::ParamValue;
    let mut g = ConditionGroup::new();
    for op in ["all", "any", "none", "at_least", "exactly"] {
        g.operator.apply_runtime_value(&ParamValue::Str(op.to_string()));
        assert_eq!(g.operator.get_ref().as_str(), op);
    }
}

#[test]
fn condition_group_operator_visibility_tracks_condition_items() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ConditionGroup::new().into(), None);
    engine.apply_edits().expect("condition group should attach");
    let group = node_by_type(&engine, ConditionGroup::NODE_TYPE);

    assert_operator_visibility(&engine, group, false);

    create_input_value_condition(&mut engine, group);
    assert_operator_visibility(&engine, group, false);

    create_input_value_condition(&mut engine, group);
    assert_operator_visibility(&engine, group, true);
}

#[test]
fn condition_group_accepts_all_condition_types() {
    let g = ConditionGroup::new();
    assert!(g.user_container_accepts_item("sm_input_value_condition", "sm_condition"));
    assert!(g.user_container_accepts_item("sm_input_node_condition", "sm_condition"));
    assert!(g.user_container_accepts_item("sm_script_condition", "sm_condition"));
    assert!(g.user_container_accepts_item("sm_condition_group", "sm_condition"));
}

#[test]
fn condition_group_rejects_wrong_item_kind() {
    let g = ConditionGroup::new();
    assert!(!g.user_container_accepts_item("sm_condition_group", "sm_consequence"));
    assert!(!g.user_container_accepts_item("sm_input_value_condition", "sm_filter"));
}

#[test]
fn condition_group_creatable_items_include_all_types() {
    let g = ConditionGroup::new();
    let items = g.user_creatable_items();
    let node_types: Vec<&str> = items.iter().map(|i| i.node_type.as_str()).collect();
    assert!(node_types.contains(&InputValueCondition::NODE_TYPE));
    assert!(node_types.contains(&InputNodeCondition::NODE_TYPE));
    assert!(node_types.contains(&ScriptCondition::NODE_TYPE));
    assert!(node_types.contains(&ConditionGroup::NODE_TYPE));
}

#[test]
fn condition_group_creatable_items_are_authored_order_with_group_separated() {
    let g = ConditionGroup::new();
    let items = g.user_creatable_items();
    let node_types: Vec<&str> = items.iter().map(|i| i.node_type.as_str()).collect();
    assert_eq!(
        node_types,
        vec![
            InputValueCondition::NODE_TYPE,
            InputNodeCondition::NODE_TYPE,
            ScriptCondition::NODE_TYPE,
            ConditionGroup::NODE_TYPE,
        ]
    );
    // Only the Condition Group wrapper is rendered below a divider.
    for item in &items {
        let expects_separator = item.node_type == ConditionGroup::NODE_TYPE;
        assert_eq!(item.separator_before, expects_separator, "{}", item.node_type);
    }
}

#[test]
fn condition_group_is_sm_condition_item() {
    use crate::app::{ConditionManager, declared_user_item_type_matches};
    let cm = ConditionManager::new();
    assert!(cm.user_container_accepts_item("sm_condition_group", "sm_condition"));
    assert!(declared_user_item_type_matches("sm_condition_group", "sm_condition"));
}

#[test]
fn condition_manager_accepts_all_condition_types() {
    use crate::app::ConditionManager;
    let cm = ConditionManager::new();
    assert!(cm.user_container_accepts_item("sm_input_value_condition", "sm_condition"));
    assert!(cm.user_container_accepts_item("sm_input_node_condition", "sm_condition"));
    assert!(cm.user_container_accepts_item("sm_script_condition", "sm_condition"));
    assert!(cm.user_container_accepts_item("sm_condition_group", "sm_condition"));
}

#[test]
fn condition_manager_reuses_identical_current_and_settled_programs() {
    use crate::app::ConditionManager;

    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ConditionManager::new().into(), None);
    engine.apply_edits().expect("condition manager should attach");
    let manager = node_by_type(&engine, ConditionManager::NODE_TYPE);

    let mut condition = ScriptCondition::new();
    condition
        .script
        .apply_runtime_value(&ParamValue::Str("true".to_owned()));
    engine.add_node(condition.into(), Some(manager));
    engine.apply_edits().expect("script condition should attach");

    let snapshot = engine.process_tree_snapshot();
    let compiled = compile_manager_condition(snapshot.as_ref(), manager)
        .expect("condition manager should compile");
    assert!(Arc::ptr_eq(&compiled.current, &compiled.settled));
}

fn node_by_type(engine: &crate::app::AppEngine, node_type: &str) -> NodeId {
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == node_type)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("{node_type} should exist"))
}

fn assert_operator_visibility(engine: &crate::app::AppEngine, group: NodeId, expected: bool) {
    let snapshot = engine.process_tree_snapshot();
    let operator = snapshot
        .find_child_by_decl_id(group, "operator")
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

fn create_input_value_condition(engine: &mut crate::app::AppEngine, parent: NodeId) {
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: InputValueCondition::NODE_TYPE.to_owned(),
        label: None,
        initial_params: Vec::new(),
    });
    assert!(
        ack.success,
        "input value condition should attach to condition group: {ack:?}"
    );
}
