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
            crate::app::descriptors::MODULE_ITEM_KIND,
            golden_core::node::FOLDER_NODE_TYPE,
        ]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        if !self.user_container_rules().is_some_and(|rules| rules.accepts(item_kind)) {
            return false;
        }

        let generic_osc = crate::app::descriptors::GENERIC_OSC_MODULE;
        let folder = crate::app::descriptors::FOLDER_ITEM;
        (item_type == generic_osc.type_id && item_kind == crate::app::descriptors::MODULE_ITEM_KIND)
            || (item_type == folder.type_id && item_kind == golden_core::node::FOLDER_NODE_TYPE)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let generic_osc = crate::app::descriptors::GENERIC_OSC_MODULE;
        let folder = crate::app::descriptors::FOLDER_ITEM;
        vec![
            UserCreatableItem::new(
                generic_osc.type_id,
                crate::app::descriptors::MODULE_ITEM_KIND,
                generic_osc.display_name,
            ),
            UserCreatableItem::new(
                folder.type_id,
                golden_core::node::FOLDER_NODE_TYPE,
                folder.display_name,
            ),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        let generic_osc = crate::app::descriptors::GENERIC_OSC_MODULE;
        if node_type == generic_osc.type_id {
            let mut module = crate::app::GenericOscModule::create();
            module.node_data_mut().meta.label = generic_osc.display_name.to_string();
            return Some(Box::new(module));
        }
        if node_type == golden_core::node::FOLDER_NODE_TYPE {
            return Some(Box::new(golden_core::node::Folder::new(
                crate::app::descriptors::FOLDER_ITEM.display_name,
            )));
        }

        None
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
}
