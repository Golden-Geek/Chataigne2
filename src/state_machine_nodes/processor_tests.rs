use golden_core::node::{Folder, Node};
use golden_core::parameter::ParamValue;

use crate::app::{
    AppEngine, AppNode, ConditionManager, ConsequencesManager, FilterChainManager, FormulaLibrary,
    OutputsManager,
};

use super::{
    apply_comparator, project_to_scalar, reduce_results, CondEval, StateProcessor,
    StateProcessorFolder, StateProcessorManager, PROCESSOR_FOLDER_ITEM_KIND,
    PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_ITEM_KIND,
};

// ─── Unit tests: project_to_scalar ─────────────────────────────────────────

#[test]
fn project_float_auto() {
    assert_eq!(project_to_scalar(&ParamValue::Float(3.5), "auto"), Some(3.5));
}

#[test]
fn project_int_number() {
    assert_eq!(project_to_scalar(&ParamValue::Int(7), "number"), Some(7.0));
}

#[test]
fn project_bool_true_as_number() {
    assert_eq!(project_to_scalar(&ParamValue::Bool(true), "auto"), Some(1.0));
    assert_eq!(project_to_scalar(&ParamValue::Bool(false), "auto"), Some(0.0));
}

#[test]
fn project_bool_projection() {
    assert_eq!(project_to_scalar(&ParamValue::Float(5.0), "bool"), Some(1.0));
    assert_eq!(project_to_scalar(&ParamValue::Float(0.0), "bool"), Some(0.0));
}

#[test]
fn project_vec2_components_and_magnitude() {
    let v = ParamValue::Vec2(3.0, 4.0);
    assert_eq!(project_to_scalar(&v, "vec2_x"), Some(3.0));
    assert_eq!(project_to_scalar(&v, "vec2_y"), Some(4.0));
    assert_eq!(project_to_scalar(&v, "vec2_magnitude"), Some(5.0));
}

#[test]
fn project_vec3_components_and_magnitude() {
    let v = ParamValue::Vec3(1.0, 0.0, 0.0);
    assert_eq!(project_to_scalar(&v, "vec3_x"), Some(1.0));
    assert_eq!(project_to_scalar(&v, "vec3_magnitude"), Some(1.0));
}

#[test]
fn project_color_channels() {
    let c = ParamValue::Color(0.2, 0.5, 0.8, 1.0);
    assert_eq!(project_to_scalar(&c, "color_red"), Some(0.2));
    assert_eq!(project_to_scalar(&c, "color_alpha"), Some(1.0));
    // luminance = 0.2126*0.2 + 0.7152*0.5 + 0.0722*0.8
    let expected = 0.2126 * 0.2 + 0.7152 * 0.5 + 0.0722 * 0.8;
    let actual = project_to_scalar(&c, "color_luminance").unwrap();
    assert!((actual - expected).abs() < 1e-9);
}

// ─── Unit tests: apply_comparator ──────────────────────────────────────────

fn cmp(val: f64, op: &str, reference: f64) -> bool {
    apply_comparator(&ParamValue::Float(val), "auto", op, reference, reference + 1.0, "")
}

#[test]
fn comparator_equal() {
    assert!(cmp(5.0, "equal", 5.0));
    assert!(!cmp(5.0, "equal", 6.0));
}

#[test]
fn comparator_not_equal() {
    assert!(!cmp(5.0, "not_equal", 5.0));
    assert!(cmp(5.0, "not_equal", 6.0));
}

#[test]
fn comparator_greater_less() {
    assert!(cmp(6.0, "greater_than", 5.0));
    assert!(!cmp(5.0, "greater_than", 5.0));
    assert!(cmp(4.0, "less_than", 5.0));
    assert!(cmp(5.0, "less_than_or_equal", 5.0));
    assert!(cmp(5.0, "greater_than_or_equal", 5.0));
}

#[test]
fn comparator_between_outside() {
    assert!(apply_comparator(
        &ParamValue::Float(3.0), "auto", "between", 2.0, 4.0, ""
    ));
    assert!(!apply_comparator(
        &ParamValue::Float(5.0), "auto", "between", 2.0, 4.0, ""
    ));
    assert!(apply_comparator(
        &ParamValue::Float(5.0), "auto", "outside", 2.0, 4.0, ""
    ));
}

#[test]
fn comparator_is_true_is_false() {
    assert!(cmp(1.0, "is_true", 0.0));
    assert!(!cmp(0.0, "is_true", 0.0));
    assert!(cmp(0.0, "is_false", 0.0));
    assert!(!cmp(1.0, "is_false", 0.0));
}

#[test]
fn comparator_bool_is_true_direct() {
    assert!(apply_comparator(
        &ParamValue::Bool(true), "auto", "is_true", 0.0, 1.0, ""
    ));
    assert!(!apply_comparator(
        &ParamValue::Bool(false), "auto", "is_true", 0.0, 1.0, ""
    ));
}

#[test]
fn comparator_string_ops() {
    let v = ParamValue::Str("hello world".into());
    assert!(apply_comparator(&v, "auto", "contains", 0.0, 1.0, "world"));
    assert!(apply_comparator(&v, "auto", "starts_with", 0.0, 1.0, "hello"));
    assert!(apply_comparator(&v, "auto", "ends_with", 0.0, 1.0, "world"));
    assert!(!apply_comparator(&v, "auto", "contains", 0.0, 1.0, "xyz"));
}

#[test]
fn comparator_string_projection_equal() {
    let v = ParamValue::Enum("active".into());
    assert!(apply_comparator(&v, "enum_value", "equal", 0.0, 1.0, "active"));
    assert!(!apply_comparator(&v, "enum_value", "equal", 0.0, 1.0, "idle"));
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
