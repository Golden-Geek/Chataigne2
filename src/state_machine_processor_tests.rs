use golden_core::node::{Folder, Node};

use crate::app::{
    AppEngine, AppNode, ConditionManager, ConsequencesManager, FilterChainManager, FormulaLibrary,
    OutputsManager,
};

use super::{
    StateProcessor, StateProcessorFolder, StateProcessorManager, PROCESSOR_FOLDER_ITEM_KIND,
    PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_ITEM_KIND,
};

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
        .find(|(_, n)| n.get_type() == crate::app::state_machine_formula::ActionBuiltinFormula::NODE_TYPE)
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
fn mapping_processor_instantiates_filter_chain_and_outputs_from_formula() {
    let (mut engine, _lib_id) = build_engine_with_formula_library();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    let mapping_formula_uuid = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == crate::app::state_machine_formula::MappingBuiltinFormula::NODE_TYPE)
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
