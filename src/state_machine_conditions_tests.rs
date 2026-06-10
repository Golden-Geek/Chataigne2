use golden_core::node::Node;

use super::{BoolCondition, ConditionGroup, NumberCompareCondition};

// --- NumberCompareCondition ---

#[test]
fn number_compare_condition_defaults() {
    let c = NumberCompareCondition::new();
    assert_eq!(c.node_data().meta.label, "Number Compare");
    assert!(!*c.invert.get_ref());
    assert_eq!(*c.validation_delay_ms.get_ref(), 0.0);
    assert_eq!(*c.invalidation_delay_ms.get_ref(), 0.0);
    assert_eq!(c.comparator.get_ref().as_str(), "greater_than");
    assert_eq!(*c.threshold.get_ref(), 0.0);
}

// --- BoolCondition ---

#[test]
fn bool_condition_defaults() {
    let c = BoolCondition::new();
    assert_eq!(c.node_data().meta.label, "Boolean");
    assert!(!*c.invert.get_ref());
    assert_eq!(*c.validation_delay_ms.get_ref(), 0.0);
    assert_eq!(*c.invalidation_delay_ms.get_ref(), 0.0);
    assert!(*c.expected.get_ref());
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
fn condition_group_accepts_leaf_conditions() {
    let g = ConditionGroup::new();
    assert!(g.user_container_accepts_item("sm_number_compare_condition", "sm_condition"));
    assert!(g.user_container_accepts_item("sm_bool_condition", "sm_condition"));
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
    assert!(!g.user_container_accepts_item("sm_number_compare_condition", "sm_consequence"));
}

#[test]
fn condition_group_creatable_items_include_itself() {
    let g = ConditionGroup::new();
    let items = g.user_creatable_items();
    assert!(
        items.iter().any(|i| i.node_type == ConditionGroup::NODE_TYPE),
        "ConditionGroup should appear in its own creatable items for nesting"
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
    assert!(
        declared_user_item_type_matches("sm_condition_group", "sm_condition"),
        "declared_user_item_type_matches should recognise sm_condition_group"
    );
}
