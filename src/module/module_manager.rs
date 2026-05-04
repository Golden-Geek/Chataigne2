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
mod module_manager_tests;
