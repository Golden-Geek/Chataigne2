use golden_core::node::Node;

use super::{
    PROCESSOR_FOLDER_ITEM_KIND, PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_ITEM_KIND, StateProcessorFolder,
    StateProcessorManager,
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
