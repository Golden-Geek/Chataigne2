use golden_core::{
    node,
    node::{Node, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

#[node("module_manager", label = "Module Manager")]
pub struct ModuleManager {}

#[node("module_manager", from_struct)]
impl Node for ModuleManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            crate::app::module::MODULE_ITEM_KIND,
            golden_core::node::FOLDER_NODE_TYPE,
        ]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        if !self
            .user_container_rules()
            .is_some_and(|rules| rules.accepts(item_kind))
        {
            return false;
        }

        (item_kind == crate::app::module::MODULE_ITEM_KIND
            && crate::app::declared_user_item_type_matches(item_type, crate::app::module::MODULE_ITEM_KIND))
            || (item_type == golden_core::node::FOLDER_NODE_TYPE && item_kind == golden_core::node::FOLDER_NODE_TYPE)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = crate::app::declared_user_creatable_items(crate::app::module::MODULE_ITEM_KIND);
        items.push(UserCreatableItem::new(
            golden_core::node::FOLDER_NODE_TYPE,
            golden_core::node::FOLDER_NODE_TYPE,
            "Folder",
        ));
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, crate::app::module::MODULE_ITEM_KIND).or_else(|| {
            (node_type == golden_core::node::FOLDER_NODE_TYPE)
                .then(|| Box::new(golden_core::node::Folder::new("Folder")) as Box<dyn Node>)
        })
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
    }
}

#[cfg(test)]
mod tests {
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
            (crate::app::TcpModule::NODE_TYPE, crate::app::TcpModule::DEFAULT_LABEL),
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
}
