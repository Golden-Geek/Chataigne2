use golden_core::{
    engine::EngineTime,
    node::Node,
    process_ctx::{ExecutionPhase, ProcessCtx},
};

use super::{MODULE_FOLDER_ITEM_KIND, MODULE_FOLDER_NODE_TYPE, ModuleFolder, ModuleManager};

#[test]
fn module_manager_cannot_be_removed_or_duplicated() {
    let mut manager = ModuleManager::new();
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    manager.init(&mut ctx);

    let permissions = &manager.node_data().meta.user_permissions;
    assert!(
        !permissions.can_edit_name,
        "the fixed top-level module manager must keep its declared identity"
    );
    assert!(!permissions.can_remove_and_duplicate);
    assert!(permissions.can_edit_constraints);
    assert!(permissions.can_edit_tags);
    assert!(permissions.can_edit_color);
}

#[test]
fn module_manager_uses_declared_module_item_metadata() {
    let manager = ModuleManager::new();
    let items = manager.user_creatable_items();

    for (node_type, label, menu_path) in [
        (
            crate::app::GenericOscModule::NODE_TYPE,
            crate::app::GenericOscModule::DEFAULT_LABEL,
            vec!["Network".to_string()],
        ),
        (
            crate::app::SerialModule::NODE_TYPE,
            crate::app::SerialModule::DEFAULT_LABEL,
            vec!["Hardware".to_string()],
        ),
        (
            crate::app::UdpModule::NODE_TYPE,
            crate::app::UdpModule::DEFAULT_LABEL,
            vec!["Network".to_string()],
        ),
        (
            crate::app::TcpServerModule::NODE_TYPE,
            crate::app::TcpServerModule::DEFAULT_LABEL,
            vec!["Network".to_string()],
        ),
        (
            crate::app::TcpClientModule::NODE_TYPE,
            crate::app::TcpClientModule::DEFAULT_LABEL,
            vec!["Network".to_string()],
        ),
        (
            crate::app::WebSocketServerModule::NODE_TYPE,
            crate::app::WebSocketServerModule::DEFAULT_LABEL,
            vec!["Network".to_string()],
        ),
        (
            crate::app::WebSocketClientModule::NODE_TYPE,
            crate::app::WebSocketClientModule::DEFAULT_LABEL,
            vec!["Network".to_string()],
        ),
    ] {
        let item = items
            .iter()
            .find(|item| item.node_type == node_type)
            .unwrap_or_else(|| panic!("module manager should expose module item type {node_type}"));
        assert_eq!(item.item_kind, crate::app::module::MODULE_ITEM_KIND);
        assert_eq!(item.label, label);
        assert_eq!(item.menu_path, menu_path);

        let created = manager
            .create_user_item(node_type)
            .unwrap_or_else(|| panic!("module manager should create module item type {node_type}"));
        assert_eq!(created.node_data().meta.label, label);
    }

    let folder = items
        .last()
        .expect("module manager should expose a trailing Folder item");
    assert_eq!(folder.node_type, MODULE_FOLDER_NODE_TYPE);
    assert_eq!(folder.item_kind, MODULE_FOLDER_ITEM_KIND);
    assert!(folder.menu_path.is_empty());

    let folder = manager
        .create_user_item(MODULE_FOLDER_NODE_TYPE)
        .expect("module manager should create typed module folders");
    assert_eq!(folder.get_type(), MODULE_FOLDER_NODE_TYPE);
}

#[test]
fn module_folder_accepts_only_modules_and_module_folders() {
    let folder = ModuleFolder::new();

    assert!(folder.user_container_accepts_item(
        crate::app::GenericOscModule::NODE_TYPE,
        crate::app::module::MODULE_ITEM_KIND,
    ));
    assert!(folder.user_container_accepts_item(MODULE_FOLDER_NODE_TYPE, MODULE_FOLDER_ITEM_KIND));
    assert!(!folder.user_container_accepts_item(
        crate::app::StateProcessorFolder::NODE_TYPE,
        crate::app::state_machine_nodes_processor::PROCESSOR_FOLDER_ITEM_KIND,
    ));
}
