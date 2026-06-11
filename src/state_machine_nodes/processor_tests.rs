use golden_core::node::{Folder, Node, NodeId};
use golden_core::parameter::ParamValue;

use std::collections::HashMap;

use crate::app::{
    AppEngine, AppNode, ConditionManager, ConsequencesManager, FilterChainManager, FormulaLibrary,
    OutputsManager,
};

use super::{
    apply_comparator_v2, apply_toggle, reduce_results, valid_comparators_for_value, CondEval,
    StateProcessor, StateProcessorFolder, StateProcessorManager, PROCESSOR_FOLDER_ITEM_KIND,
    PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_ITEM_KIND,
};

// ─── Unit tests: apply_comparator_v2 ──────────────────────────────────────

fn cmpf(val: f64, op: &str, reference: f64) -> bool {
    apply_comparator_v2(&ParamValue::Float(val), op, reference, reference + 1.0, "")
}

#[test]
fn comparator_v2_equal() {
    assert!(cmpf(5.0, "equal", 5.0));
    assert!(!cmpf(5.0, "equal", 6.0));
}

#[test]
fn comparator_v2_not_equal() {
    assert!(!cmpf(5.0, "not_equal", 5.0));
    assert!(cmpf(5.0, "not_equal", 6.0));
}

#[test]
fn comparator_v2_greater_less() {
    assert!(cmpf(6.0, "greater_than", 5.0));
    assert!(!cmpf(5.0, "greater_than", 5.0));
    assert!(cmpf(4.0, "less_than", 5.0));
    assert!(cmpf(5.0, "less_than_or_equal", 5.0));
    assert!(cmpf(5.0, "greater_than_or_equal", 5.0));
}

#[test]
fn comparator_v2_between_outside() {
    assert!(apply_comparator_v2(
        &ParamValue::Float(3.0),
        "between",
        2.0,
        4.0,
        ""
    ));
    assert!(!apply_comparator_v2(
        &ParamValue::Float(5.0),
        "between",
        2.0,
        4.0,
        ""
    ));
    assert!(apply_comparator_v2(
        &ParamValue::Float(5.0),
        "outside",
        2.0,
        4.0,
        ""
    ));
}

#[test]
fn comparator_v2_is_true_is_false_float() {
    assert!(cmpf(1.0, "is_true", 0.0));
    assert!(!cmpf(0.0, "is_true", 0.0));
    assert!(cmpf(0.0, "is_false", 0.0));
    assert!(!cmpf(1.0, "is_false", 0.0));
}

#[test]
fn comparator_v2_bool_is_true_is_false() {
    assert!(apply_comparator_v2(
        &ParamValue::Bool(true),
        "is_true",
        0.0,
        1.0,
        ""
    ));
    assert!(!apply_comparator_v2(
        &ParamValue::Bool(false),
        "is_true",
        0.0,
        1.0,
        ""
    ));
    assert!(apply_comparator_v2(
        &ParamValue::Bool(false),
        "is_false",
        0.0,
        1.0,
        ""
    ));
}

#[test]
fn comparator_v2_bool_equal_not_equal() {
    assert!(apply_comparator_v2(
        &ParamValue::Bool(true),
        "equal",
        1.0,
        1.0,
        ""
    ));
    assert!(apply_comparator_v2(
        &ParamValue::Bool(false),
        "equal",
        0.0,
        1.0,
        ""
    ));
    assert!(apply_comparator_v2(
        &ParamValue::Bool(true),
        "not_equal",
        0.0,
        1.0,
        ""
    ));
}

#[test]
fn comparator_v2_string_ops() {
    let v = ParamValue::Str("hello world".into());
    assert!(apply_comparator_v2(&v, "contains", 0.0, 1.0, "world"));
    assert!(apply_comparator_v2(&v, "starts_with", 0.0, 1.0, "hello"));
    assert!(apply_comparator_v2(&v, "ends_with", 0.0, 1.0, "world"));
    assert!(!apply_comparator_v2(&v, "contains", 0.0, 1.0, "xyz"));
}

#[test]
fn comparator_v2_enum_equal() {
    let v = ParamValue::Enum("active".into());
    assert!(apply_comparator_v2(&v, "equal", 0.0, 1.0, "active"));
    assert!(!apply_comparator_v2(&v, "equal", 0.0, 1.0, "idle"));
}

#[test]
fn comparator_v2_int_numeric() {
    assert!(apply_comparator_v2(&ParamValue::Int(5), "equal", 5.0, 6.0, ""));
    assert!(apply_comparator_v2(&ParamValue::Int(5), "greater_than", 4.0, 5.0, ""));
}

// ─── Unit tests: valid_comparators_for_value ───────────────────────────────

#[test]
fn valid_comparators_float_includes_numeric_and_value_changed() {
    let s = valid_comparators_for_value(&ParamValue::Float(1.0));
    assert!(s.contains("greater_than"));
    assert!(s.contains("between"));
    assert!(s.contains("value_changed"));
}

#[test]
fn valid_comparators_bool_includes_is_true_and_value_changed() {
    let s = valid_comparators_for_value(&ParamValue::Bool(true));
    assert!(s.contains("is_true"));
    assert!(s.contains("is_false"));
    assert!(s.contains("value_changed"));
    assert!(!s.contains("greater_than"));
}

#[test]
fn valid_comparators_string_includes_string_ops() {
    let s = valid_comparators_for_value(&ParamValue::Str(String::new()));
    assert!(s.contains("contains"));
    assert!(s.contains("starts_with"));
    assert!(s.contains("value_changed"));
    assert!(!s.contains("greater_than"));
}

#[test]
fn valid_comparators_enum_same_as_string() {
    let str_set = valid_comparators_for_value(&ParamValue::Str(String::new()));
    let enum_set = valid_comparators_for_value(&ParamValue::Enum(String::new()));
    assert_eq!(str_set, enum_set);
}

#[test]
fn valid_comparators_unknown_type_only_value_changed() {
    let s = valid_comparators_for_value(&ParamValue::Trigger());
    assert_eq!(s, "value_changed");
}

// ─── Unit tests: apply_toggle ──────────────────────────────────────────────

fn make_id(n: u64) -> NodeId {
    NodeId(n)
}

#[test]
fn toggle_off_passes_through() {
    let id = make_id(1);
    let mut ts: HashMap<NodeId, bool> = HashMap::new();
    let mut pr: HashMap<NodeId, bool> = HashMap::new();
    assert!(apply_toggle(id, true, false, &mut ts, &mut pr));
    assert!(!apply_toggle(id, false, false, &mut ts, &mut pr));
}

#[test]
fn toggle_on_rising_edge_flips_state() {
    let id = make_id(2);
    let mut ts: HashMap<NodeId, bool> = HashMap::new();
    let mut pr: HashMap<NodeId, bool> = HashMap::new();

    // First call: raw=false → prev_raw=false, no edge, toggle stays false
    assert!(!apply_toggle(id, false, true, &mut ts, &mut pr));

    // Rising edge: raw=true, prev_raw=false → toggle flips to true
    assert!(apply_toggle(id, true, true, &mut ts, &mut pr));

    // Still true: raw=true, prev_raw=true → no rising edge, toggle stays true
    assert!(apply_toggle(id, true, true, &mut ts, &mut pr));

    // Falling: raw=false, prev_raw=true → no edge, toggle stays true
    assert!(apply_toggle(id, false, true, &mut ts, &mut pr));

    // Rising again: raw=true, prev_raw=false → toggle flips back to false
    assert!(!apply_toggle(id, true, true, &mut ts, &mut pr));
}

#[test]
fn toggle_on_with_initial_raw_false_stays_false() {
    let id = make_id(3);
    let mut ts: HashMap<NodeId, bool> = HashMap::new();
    let mut pr: HashMap<NodeId, bool> = HashMap::new();
    // No rising edge → toggle output is false
    for _ in 0..5 {
        assert!(!apply_toggle(id, false, true, &mut ts, &mut pr));
    }
}

// ─── Unit tests: reduce_results ────────────────────────────────────────────

fn reduce(results: &[CondEval], op: &str) -> bool {
    reduce_results(results, op, 1.0, "invalid", "ignore", "treat_as_invalid")
}

#[test]
fn reduce_all_valid() {
    assert!(reduce(&[CondEval::Known(true), CondEval::Known(true)], "all"));
    assert!(!reduce(&[CondEval::Known(true), CondEval::Known(false)], "all"));
}

#[test]
fn reduce_any() {
    assert!(reduce(&[CondEval::Known(false), CondEval::Known(true)], "any"));
    assert!(!reduce(&[CondEval::Known(false), CondEval::Known(false)], "any"));
}

#[test]
fn reduce_none() {
    assert!(reduce(&[CondEval::Known(false), CondEval::Known(false)], "none"));
    assert!(!reduce(&[CondEval::Known(true), CondEval::Known(false)], "none"));
}

#[test]
fn reduce_at_least() {
    let r = [CondEval::Known(true), CondEval::Known(true), CondEval::Known(false)];
    assert!(reduce_results(&r, "at_least", 2.0, "invalid", "ignore", "treat_as_invalid"));
    assert!(!reduce_results(&r, "at_least", 3.0, "invalid", "ignore", "treat_as_invalid"));
}

#[test]
fn reduce_exactly() {
    let r = [CondEval::Known(true), CondEval::Known(true), CondEval::Known(false)];
    assert!(reduce_results(&r, "exactly", 2.0, "invalid", "ignore", "treat_as_invalid"));
    assert!(!reduce_results(&r, "exactly", 1.0, "invalid", "ignore", "treat_as_invalid"));
}

#[test]
fn reduce_empty_invalid_policy() {
    let result = reduce_results(&[], "all", 1.0, "invalid", "ignore", "treat_as_invalid");
    assert!(!result);
}

#[test]
fn reduce_empty_valid_policy() {
    let result = reduce_results(&[], "all", 1.0, "valid", "ignore", "treat_as_invalid");
    assert!(result);
}

#[test]
fn reduce_disabled_ignore_with_one_valid() {
    // Disabled item ignored; one Known(true) still satisfies "any"
    let r = [CondEval::Known(true), CondEval::Disabled];
    assert!(reduce_results(&r, "any", 1.0, "invalid", "ignore", "treat_as_invalid"));
}

#[test]
fn reduce_disabled_treat_as_invalid_breaks_all() {
    let r = [CondEval::Known(true), CondEval::Disabled];
    assert!(!reduce_results(
        &r, "all", 1.0, "invalid", "treat_as_invalid", "treat_as_invalid"
    ));
}

#[test]
fn reduce_disabled_treat_as_valid_satisfies_all() {
    let r = [CondEval::Known(true), CondEval::Disabled];
    assert!(reduce_results(
        &r, "all", 1.0, "invalid", "treat_as_valid", "treat_as_invalid"
    ));
}

#[test]
fn reduce_error_treat_as_invalid_breaks_all() {
    let r = [CondEval::Known(true), CondEval::Error];
    assert!(!reduce_results(
        &r, "all", 1.0, "invalid", "ignore", "treat_as_invalid"
    ));
}

#[test]
fn reduce_all_disabled_ignored_falls_to_empty_policy() {
    // All items are disabled and ignored → effective set is empty → use empty_policy
    let r = [CondEval::Disabled, CondEval::Disabled];
    assert!(!reduce_results(&r, "all", 1.0, "invalid", "ignore", "treat_as_invalid"));
    assert!(reduce_results(&r, "all", 1.0, "valid", "ignore", "treat_as_invalid"));
}

fn build_engine_with_formula_library() -> (AppEngine, golden_core::node::NodeId) {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.apply_edits().expect("formula library should attach");
    let lib_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("FormulaLibrary not found");
    (engine, lib_id)
}

#[test]
fn processor_manager_exposes_action_and_mapping_items() {
    let (mut engine, lib_id) = build_engine_with_formula_library();

    // StateProcessorManager needs FormulaLibrary already in tree when it initialises.
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    let manager = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(_, n)| n)
        .expect("StateProcessorManager not found");

    let items = manager.user_creatable_items();

    // Should expose one item per formula in the library (Action + Mapping).
    let formula_items: Vec<_> = items
        .iter()
        .filter(|i| i.item_kind == PROCESSOR_ITEM_KIND)
        .collect();
    assert_eq!(formula_items.len(), 2, "expected 2 formula items (Action + Mapping)");

    // Labels should match the built-in formula labels.
    let labels: Vec<&str> = formula_items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Action"), "missing Action item");
    assert!(labels.contains(&"Mapping"), "missing Mapping item");

    // All formula items use the "state_processor:UUID" node_type encoding.
    for item in &formula_items {
        assert!(
            item.node_type.starts_with("state_processor:"),
            "item node_type should be 'state_processor:UUID' but got '{}'",
            item.node_type
        );
    }

    // Folder item should also be present.
    assert!(
        items.iter().any(|i| i.node_type == PROCESSOR_FOLDER_NODE_TYPE),
        "folder item missing"
    );

    let _ = lib_id; // suppress unused warning
}

#[test]
fn processor_folder_accepts_processors_and_folders() {
    let folder = StateProcessorFolder::new();

    // Folder items accepted by both item_type and item_kind checks.
    assert!(folder.user_container_accepts_item(PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_FOLDER_ITEM_KIND));
    assert!(folder.create_user_item(PROCESSOR_FOLDER_NODE_TYPE).is_some());

    // Processor items accepted via the "state_processor:{UUID}" pattern.
    let fake_uuid = format!("state_processor:{}", uuid::Uuid::nil());
    assert!(folder.user_container_accepts_item(&fake_uuid, PROCESSOR_ITEM_KIND));
}

#[test]
fn state_processor_has_correct_type() {
    assert_eq!(StateProcessor::NODE_TYPE, "state_processor");
    let processor = StateProcessor::new();
    assert_eq!(processor.get_type(), "state_processor");
}

#[test]
fn action_processor_instantiates_condition_and_consequences_managers_from_formula() {
    let (mut engine, _lib_id) = build_engine_with_formula_library();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    // Find the Action formula UUID from the library.
    let action_formula_uuid = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == crate::app::ActionBuiltinFormula::NODE_TYPE)
        .map(|(_, n)| n.node_data().meta.uuid)
        .expect("ActionBuiltinFormula not found");

    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("StateProcessorManager not found");

    // Create a StateProcessor pre-set with the Action formula UUID.
    let mut processor = StateProcessor::new();
    processor
        .formula_uuid
        .apply_runtime_value(&golden_core::parameter::ParamValue::Str(
            action_formula_uuid.0.to_string(),
        ));

    engine.add_user_item(AppNode::from(processor), Some(manager_id));
    engine.apply_edits().expect("processor should attach");

    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("StateProcessor not found");

    let children_of_type = |node_type: &str| {
        engine
            .nodes
            .iter()
            .filter(|(_, n)| n.get_type() == node_type && n.node_data().parent == Some(processor_id))
            .count()
    };

    assert_eq!(
        children_of_type(ConditionManager::NODE_TYPE),
        1,
        "expected one ConditionManager child"
    );
    assert_eq!(
        children_of_type(ConsequencesManager::NODE_TYPE),
        2,
        "expected two ConsequencesManager children (True + False)"
    );
}

#[test]
fn mapping_processor_instantiates_inputs_filter_chain_and_outputs_from_formula() {
    let (mut engine, _lib_id) = build_engine_with_formula_library();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    let mapping_formula_uuid = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == crate::app::MappingBuiltinFormula::NODE_TYPE)
        .map(|(_, n)| n.node_data().meta.uuid)
        .expect("MappingBuiltinFormula not found");

    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("StateProcessorManager not found");

    let mut processor = StateProcessor::new();
    processor
        .formula_uuid
        .apply_runtime_value(&golden_core::parameter::ParamValue::Str(
            mapping_formula_uuid.0.to_string(),
        ));

    engine.add_user_item(AppNode::from(processor), Some(manager_id));
    engine.apply_edits().expect("processor should attach");

    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("StateProcessor not found");

    let children_of_type = |node_type: &str| {
        engine
            .nodes
            .iter()
            .filter(|(_, n)| n.get_type() == node_type && n.node_data().parent == Some(processor_id))
            .count()
    };

    assert_eq!(
        children_of_type(crate::app::InputsManager::NODE_TYPE),
        1,
        "expected one InputsManager child"
    );
    assert_eq!(
        children_of_type(FilterChainManager::NODE_TYPE),
        1,
        "expected one FilterChainManager child"
    );
    assert_eq!(
        children_of_type(OutputsManager::NODE_TYPE),
        1,
        "expected one OutputsManager child"
    );
}
