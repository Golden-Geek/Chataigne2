use golden_core::node::Node;

use super::{
    PROCESSOR_FOLDER_ITEM_KIND, PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_ITEM_KIND, StateProcessorFolder,
    StateProcessorManager,
};

#[test]
fn processor_manager_exposes_runtime_models_custom_processors_and_folders() {
    let manager = StateProcessorManager::new();
    let items = manager.user_creatable_items();
    let builtin_labels: Vec<_> = chataigne_state_machine::builtin_processor_models()
        .into_iter()
        .map(|model| model.label)
        .collect();

    for label in builtin_labels {
        let item = items
            .iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("missing built-in processor item {label}"));
        assert_eq!(item.item_kind, PROCESSOR_ITEM_KIND);
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
