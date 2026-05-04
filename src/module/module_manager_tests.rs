use golden_core::{
    engine::EngineTime,
    node::Node,
    process_ctx::{ExecutionPhase, ProcessCtx},
};

use super::ModuleManager;

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
    assert!(permissions.can_edit_name);
    assert!(!permissions.can_remove_and_duplicate);
    assert!(permissions.can_edit_constraints);
    assert!(permissions.can_edit_tags);
    assert!(permissions.can_edit_color);
}

#[test]
fn module_manager_uses_declared_module_item_metadata() {
    let manager = ModuleManager::new();
    let items = manager.user_creatable_items();

    for (node_type, label) in [
        (
            crate::app::GenericOscModule::NODE_TYPE,
            crate::app::GenericOscModule::DEFAULT_LABEL,
        ),
        (
            crate::app::SerialModule::NODE_TYPE,
            crate::app::SerialModule::DEFAULT_LABEL,
        ),
        (crate::app::UdpModule::NODE_TYPE, crate::app::UdpModule::DEFAULT_LABEL),
        (
            crate::app::TcpServerModule::NODE_TYPE,
            crate::app::TcpServerModule::DEFAULT_LABEL,
        ),
        (crate::app::TcpClientModule::NODE_TYPE, crate::app::TcpClientModule::DEFAULT_LABEL),
    ] {
        let item = items
            .iter()
            .find(|item| item.node_type == node_type)
            .unwrap_or_else(|| panic!("module manager should expose module item type {node_type}"));
        assert_eq!(item.item_kind, crate::app::module::MODULE_ITEM_KIND);
        assert_eq!(item.label, label);
        assert_eq!(item.menu_path, vec!["Generic".to_string()]);

        let created = manager
            .create_user_item(node_type)
            .unwrap_or_else(|| panic!("module manager should create module item type {node_type}"));
        assert_eq!(created.node_data().meta.label, label);
    }

    let folder = items
        .last()
        .expect("module manager should expose a trailing Folder item");
    assert_eq!(folder.node_type, golden_core::node::FOLDER_NODE_TYPE);
    assert!(folder.menu_path.is_empty());
}
