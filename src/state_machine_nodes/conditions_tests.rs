use golden_core::node::Node;

use super::{ConditionGroup, InputNodeCondition, InputValueCondition, ScriptCondition};

// ---------------------------------------------------------------------------
// InputValueCondition
// ---------------------------------------------------------------------------

#[test]
fn input_value_condition_type_id() {
    assert_eq!(InputValueCondition::NODE_TYPE, "sm_input_value_condition");
}

#[test]
fn input_value_condition_defaults() {
    let c = InputValueCondition::new();
    assert_eq!(c.node_data().meta.label, "Input Value");
    assert!(!*c.invert.get_ref());
    assert_eq!(*c.validation_delay_ms.get_ref(), 0.0);
    assert_eq!(*c.invalidation_delay_ms.get_ref(), 0.0);
    assert_eq!(c.projection.get_ref().as_str(), "auto");
    assert_eq!(c.comparator.get_ref().as_str(), "equal");
    assert_eq!(*c.reference.get_ref(), 0.0);
    assert_eq!(*c.reference_max.get_ref(), 1.0);
    assert!(c.reference_string.get_ref().is_empty());
}

#[test]
fn input_value_condition_is_sm_condition_item() {
    use crate::app::declared_user_item_type_matches;
    assert!(declared_user_item_type_matches("sm_input_value_condition", "sm_condition"));
    assert!(!declared_user_item_type_matches("sm_input_value_condition", "sm_consequence"));
}

// ---------------------------------------------------------------------------
// InputNodeCondition
// ---------------------------------------------------------------------------

#[test]
fn input_node_condition_type_id() {
    assert_eq!(InputNodeCondition::NODE_TYPE, "sm_input_node_condition");
}

#[test]
fn input_node_condition_defaults() {
    let c = InputNodeCondition::new();
    assert_eq!(c.node_data().meta.label, "Input Node");
    assert!(!*c.invert.get_ref());
    assert_eq!(*c.validation_delay_ms.get_ref(), 0.0);
    assert_eq!(*c.invalidation_delay_ms.get_ref(), 0.0);
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

// ---------------------------------------------------------------------------
// ScriptCondition
// ---------------------------------------------------------------------------

#[test]
fn script_condition_type_id() {
    assert_eq!(ScriptCondition::NODE_TYPE, "sm_script_condition");
}

#[test]
fn script_condition_defaults() {
    let c = ScriptCondition::new();
    assert_eq!(c.node_data().meta.label, "Script");
    assert!(!*c.invert.get_ref());
    assert_eq!(*c.validation_delay_ms.get_ref(), 0.0);
    assert_eq!(*c.invalidation_delay_ms.get_ref(), 0.0);
    assert!(c.script.get_ref().is_empty());
}

#[test]
fn script_condition_is_sm_condition_item() {
    use crate::app::declared_user_item_type_matches;
    assert!(declared_user_item_type_matches("sm_script_condition", "sm_condition"));
}

// ---------------------------------------------------------------------------
// Phase 6: output_mode and pulse_duration_ms
// ---------------------------------------------------------------------------

#[test]
fn all_condition_types_default_to_normal_output_mode() {
    assert_eq!(InputValueCondition::new().output_mode.get_ref().as_str(), "normal");
    assert_eq!(InputNodeCondition::new().output_mode.get_ref().as_str(), "normal");
    assert_eq!(ScriptCondition::new().output_mode.get_ref().as_str(), "normal");
    assert_eq!(ConditionGroup::new().output_mode.get_ref().as_str(), "normal");
}

#[test]
fn all_condition_types_default_pulse_duration_to_200ms() {
    assert_eq!(*InputValueCondition::new().pulse_duration_ms.get_ref(), 200.0);
    assert_eq!(*InputNodeCondition::new().pulse_duration_ms.get_ref(), 200.0);
    assert_eq!(*ScriptCondition::new().pulse_duration_ms.get_ref(), 200.0);
    assert_eq!(*ConditionGroup::new().pulse_duration_ms.get_ref(), 200.0);
}

// ---------------------------------------------------------------------------
// ConditionGroup
// ---------------------------------------------------------------------------

#[test]
fn condition_group_type_id() {
    assert_eq!(ConditionGroup::NODE_TYPE, "sm_condition_group");
}

#[test]
fn condition_group_defaults() {
    let g = ConditionGroup::new();
    assert!(!*g.invert.get_ref());
    assert_eq!(g.operator.get_ref().as_str(), "all");
    assert_eq!(*g.validation_delay_ms.get_ref(), 0.0);
    assert_eq!(*g.invalidation_delay_ms.get_ref(), 0.0);
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
