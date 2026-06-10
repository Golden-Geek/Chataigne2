use golden_core::node::{Folder, Node};

use crate::app::{AppEngine, AppNode, ConditionManager, ConsequencesManager, FilterChainManager, OutputsManager};

use super::{
    ActionStateProcessor, CustomStateProcessor, MappingStateProcessor, PROCESSOR_FOLDER_ITEM_KIND,
    PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_ITEM_KIND, StateProcessorFolder, StateProcessorManager,
};

#[test]
fn processor_manager_exposes_actions_mappings_custom_processors_and_folders() {
    let manager = StateProcessorManager::new();
    let items = manager.user_creatable_items();

    for label in ["Action", "Mapping"] {
        let item = items
            .iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("missing built-in processor item {label}"));
        assert_eq!(item.item_kind, PROCESSOR_ITEM_KIND);
    }
    for unavailable_label in [
        "Input Condition",
        "Multiplex",
        "Sequence Launcher",
        "State Controller",
        "Conductor",
    ] {
        assert!(!items.iter().any(|item| item.label == unavailable_label));
    }
    assert!(items.iter().any(|item| {
        item.node_type == "state_processor_custom" && item.item_kind == PROCESSOR_ITEM_KIND
    }));
    assert!(items.iter().any(|item| {
        item.node_type == PROCESSOR_FOLDER_NODE_TYPE && item.item_kind == PROCESSOR_FOLDER_ITEM_KIND
    }));
}

#[test]
fn processor_folders_accept_nested_folders_and_processors() {
    let folder = StateProcessorFolder::new();

    assert!(folder.user_container_accepts_item(PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_FOLDER_ITEM_KIND));
    assert!(folder.user_container_accepts_item("state_processor_action", PROCESSOR_ITEM_KIND));
    assert!(folder.create_user_item(PROCESSOR_FOLDER_NODE_TYPE).is_some());
    assert!(folder.create_user_item("state_processor_action").is_some());
}

#[test]
fn processor_nodes_have_correct_types_and_no_flat_formula_params() {
    assert_eq!(ActionStateProcessor::NODE_TYPE, "state_processor_action");
    assert_eq!(MappingStateProcessor::NODE_TYPE, "state_processor_mapping");
    assert_eq!(CustomStateProcessor::NODE_TYPE, "state_processor_custom");

    let action = ActionStateProcessor::new();
    let mapping = MappingStateProcessor::new();
    let custom = CustomStateProcessor::new();

    assert_eq!(action.get_type(), "state_processor_action");
    assert_eq!(mapping.get_type(), "state_processor_mapping");
    assert_eq!(custom.get_type(), "state_processor_custom");
}

#[test]
fn action_processor_generates_condition_and_consequences_managers_on_fresh_creation() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("StateProcessorManager not found");

    engine.add_user_item(ActionStateProcessor::new().into(), Some(manager_id));
    engine.apply_edits().expect("action processor should attach");

    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == ActionStateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("ActionStateProcessor not found");

    let children_by_type = |node_type: &str| {
        engine
            .nodes
            .iter()
            .filter(|(_, n)| n.get_type() == node_type && n.node_data().parent == Some(processor_id))
            .count()
    };

    assert_eq!(children_by_type(ConditionManager::NODE_TYPE), 1, "expected one ConditionManager child");
    assert_eq!(children_by_type(ConsequencesManager::NODE_TYPE), 2, "expected two ConsequencesManager children");
}

#[test]
fn mapping_processor_generates_filter_chain_and_outputs_managers_on_fresh_creation() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = AppEngine::new(root);
    engine.add_node(StateProcessorManager::new().into(), None);
    engine.apply_edits().expect("manager should attach");

    let manager_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == StateProcessorManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("StateProcessorManager not found");

    engine.add_user_item(MappingStateProcessor::new().into(), Some(manager_id));
    engine.apply_edits().expect("mapping processor should attach");

    let processor_id = engine
        .nodes
        .iter()
        .find(|(_, n)| n.get_type() == MappingStateProcessor::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("MappingStateProcessor not found");

    let children_by_type = |node_type: &str| {
        engine
            .nodes
            .iter()
            .filter(|(_, n)| n.get_type() == node_type && n.node_data().parent == Some(processor_id))
            .count()
    };

    assert_eq!(children_by_type(FilterChainManager::NODE_TYPE), 1, "expected one FilterChainManager child");
    assert_eq!(children_by_type(OutputsManager::NODE_TYPE), 1, "expected one OutputsManager child");
}
