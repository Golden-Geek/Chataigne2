use golden_core::node::Node;

use super::{ConditionGroup, InputValueCondition};

// --- InputValueCondition ---

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

// --- ConditionGroup ---

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
fn condition_group_accepts_input_value_conditions() {
    let g = ConditionGroup::new();
    assert!(g.user_container_accepts_item("sm_input_value_condition", "sm_condition"));
}

#[test]
fn condition_group_accepts_nested_groups() {
    let g = ConditionGroup::new();
    assert!(g.user_container_accepts_item("sm_condition_group", "sm_condition"));
}

#[test]
fn condition_group_rejects_wrong_item_kind() {
    let g = ConditionGroup::new();
    assert!(!g.user_container_accepts_item("sm_condition_group", "sm_consequence"));
    assert!(!g.user_container_accepts_item("sm_input_value_condition", "sm_consequence"));
}

#[test]
fn condition_group_creatable_items_include_itself_and_input_value() {
    let g = ConditionGroup::new();
    let items = g.user_creatable_items();
    assert!(
        items.iter().any(|i| i.node_type == ConditionGroup::NODE_TYPE),
        "ConditionGroup must appear in its own creatable items for nesting"
    );
    assert!(
        items.iter().any(|i| i.node_type == InputValueCondition::NODE_TYPE),
        "InputValueCondition must appear in ConditionGroup creatable items"
    );
}

#[test]
fn condition_group_is_sm_condition_item() {
    use crate::app::{ConditionManager, declared_user_item_type_matches};
    let cm = ConditionManager::new();
    assert!(
        cm.user_container_accepts_item("sm_condition_group", "sm_condition"),
        "ConditionManager must accept ConditionGroup as an sm_condition item"
    );
    assert!(declared_user_item_type_matches("sm_condition_group", "sm_condition"));
}
