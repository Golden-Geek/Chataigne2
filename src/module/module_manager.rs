use golden_core::{
    node,
    node::{Node, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

pub(crate) const MODULE_FOLDER_ITEM_KIND: &str = "module_folder";
pub(crate) const MODULE_FOLDER_NODE_TYPE: &str = "module_folder";

fn module_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[
        crate::app::module::MODULE_ITEM_KIND,
        MODULE_FOLDER_ITEM_KIND,
    ])
}

fn module_container_accepts(item_type: &str, item_kind: &str) -> bool {
    match item_kind {
        crate::app::module::MODULE_ITEM_KIND => {
            crate::app::declared_user_item_type_matches(item_type, crate::app::module::MODULE_ITEM_KIND)
        }
        MODULE_FOLDER_ITEM_KIND => item_type == MODULE_FOLDER_NODE_TYPE,
        _ => false,
    }
}

fn module_container_creatable_items() -> Vec<UserCreatableItem> {
    let mut items = crate::app::declared_user_creatable_items(crate::app::module::MODULE_ITEM_KIND);
    items.push(UserCreatableItem::new(
        MODULE_FOLDER_NODE_TYPE,
        MODULE_FOLDER_ITEM_KIND,
        "Folder",
    ));
    items
}

fn create_module_container_item(node_type: &str) -> Option<Box<dyn Node>> {
    crate::app::create_declared_user_item(node_type, crate::app::module::MODULE_ITEM_KIND).or_else(|| {
        (node_type == MODULE_FOLDER_NODE_TYPE).then(|| Box::new(ModuleFolder::new()) as Box<dyn Node>)
    })
}

#[node("module_manager", label = "Module Manager")]
pub struct ModuleManager {}

#[node("module_manager", from_struct)]
impl Node for ModuleManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(module_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        module_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        module_container_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_module_container_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
    }
}

#[node("module_folder", label = "Folder")]
pub struct ModuleFolder {}

#[node("module_folder", from_struct)]
impl Node for ModuleFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(module_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        module_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        module_container_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_module_container_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

#[cfg(test)]
mod module_manager_tests;
