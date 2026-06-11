use golden_core::{
    node::{Folder, Node, NodeReference},
    parameter::{ParamValue, Parameter},
};

use crate::app::{AlchemistFormulaDefinition, AppEngine, AppNode, FormulaLibrary};

use super::{
    PROCESSOR_FOLDER_ITEM_KIND, PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_ITEM_KIND, StateProcessor,
    StateProcessorFolder, StateProcessorManager,
};

fn engine_with_formula() -> (AppEngine, golden_core::node::NodeUuid) {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(FormulaLibrary::new().into(), None);
    engine.apply_edits().expect("formula library should attach");
    let library = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == FormulaLibrary::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Formula Library should exist");
    let mut formula = <AlchemistFormulaDefinition as Node>::project_create(
        AlchemistFormulaDefinition::NODE_TYPE,
    )
    .expect("custom Formula should be creatable");
    formula.node_data_mut().meta.label = "Custom Formula".into();
    let formula_uuid = formula.node_data().meta.uuid;
    engine.add_user_item(formula.into(), Some(library));
    engine.apply_edits().expect("formula should attach");
    (engine, formula_uuid)
}

#[test]
fn processor_manager_lists_custom_formulas() {
    let (mut engine, _) = engine_with_formula();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    let manager = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(_, node)| node)
        .expect("processor manager should exist");
    let formula_items = manager
        .user_creatable_items()
        .into_iter()
        .filter(|item| item.item_kind == PROCESSOR_ITEM_KIND)
        .collect::<Vec<_>>();

    assert_eq!(formula_items.len(), 1);
    assert_eq!(formula_items[0].label, "Custom Formula");
    assert!(formula_items[0].node_type.starts_with("state_processor:"));
}

#[test]
fn processor_created_from_formula_item_keeps_a_stable_reference() {
    let (mut engine, formula_uuid) = engine_with_formula();
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");
    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("processor manager should exist");
    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    engine.add_user_item(processor.into(), Some(manager_id));
    engine.apply_edits().expect("Processor should attach");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");
    let formula_param = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().parent == Some(processor_id))
        .find_map(|(_, node)| node.as_any().downcast_ref::<Parameter>())
        .expect("Processor should have a Formula reference");

    assert!(matches!(
        &formula_param.value,
        ParamValue::Reference(reference) if reference.uuid() == formula_uuid
    ));
}

#[test]
fn processor_folder_accepts_processors_and_folders() {
    let folder = StateProcessorFolder::new();
    assert!(folder.user_container_accepts_item(
        PROCESSOR_FOLDER_NODE_TYPE,
        PROCESSOR_FOLDER_ITEM_KIND
    ));
    assert!(folder
        .create_user_item(PROCESSOR_FOLDER_NODE_TYPE)
        .is_some());
    assert!(folder.user_container_accepts_item(
        &format!("state_processor:{}", uuid::Uuid::nil()),
        PROCESSOR_ITEM_KIND
    ));
}

#[test]
fn processor_has_no_formula_specific_children() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(StateProcessor::new().into(), None);
    engine.apply_edits().expect("Processor should attach");
    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == StateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Processor should exist");
    let children = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().parent == Some(processor_id))
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].1.node_data().meta.decl_id.0, "formula");
}
