use golden_core::node::Node;

use super::{
    ActionStateProcessor, MappingStateProcessor, PROCESSOR_FOLDER_ITEM_KIND,
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
fn action_processor_exposes_editable_formula_parameters_and_graph() {
    let action = ActionStateProcessor::new();

    assert!(action.enabled.get());
    assert!(!action.condition.get());
    assert_eq!(action.edge_mode.get_ref(), "Both");
    assert_eq!(action.cooldown_ms.get(), 0.0);

    let graph: serde_json::Value =
        serde_json::from_str(action.authored_graph.get_ref())
            .expect("action graph should be valid JSON");
    assert_eq!(graph["version"], 1);
    assert_eq!(graph["nodes"].as_array().map(Vec::len), Some(4));
    assert_eq!(graph["edges"].as_array().map(Vec::len), Some(3));
}

#[test]
fn mapping_processor_exposes_editable_formula_parameters_and_graph() {
    let mapping = MappingStateProcessor::new();

    assert!(mapping.enabled.get());
    assert_eq!(mapping.input_min.get(), 0.0);
    assert_eq!(mapping.input_max.get(), 127.0);
    assert_eq!(mapping.output_min.get(), 0.0);
    assert_eq!(mapping.output_max.get(), 1.0);
    assert_eq!(mapping.smoothing_ms.get(), 100.0);

    let graph: serde_json::Value = serde_json::from_str(mapping.authored_graph.get_ref())
        .expect("mapping graph should be valid JSON");
    assert_eq!(graph["version"], 1);
    assert_eq!(graph["nodes"].as_array().map(Vec::len), Some(4));
    assert_eq!(graph["edges"].as_array().map(Vec::len), Some(3));
}
