use golden_core::{
    item, node,
    node::{DeclId, Node, NodeCreationContext, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

pub(crate) const PROCESSOR_ITEM_KIND: &str = "state_processor";
pub(crate) const PROCESSOR_FOLDER_ITEM_KIND: &str = "state_processor_folder";
pub(crate) const PROCESSOR_FOLDER_NODE_TYPE: &str = "state_processor_folder";

fn processor_items() -> Vec<UserCreatableItem> {
    crate::app::declared_user_creatable_items(PROCESSOR_ITEM_KIND)
}

fn processor_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[PROCESSOR_ITEM_KIND, PROCESSOR_FOLDER_ITEM_KIND])
}

fn processor_container_accepts(item_type: &str, item_kind: &str) -> bool {
    (item_kind == PROCESSOR_ITEM_KIND
        && crate::app::declared_user_item_type_matches(item_type, PROCESSOR_ITEM_KIND))
        || (item_type == PROCESSOR_FOLDER_NODE_TYPE && item_kind == PROCESSOR_FOLDER_ITEM_KIND)
}

fn processor_creatable_items() -> Vec<UserCreatableItem> {
    let mut items = processor_items();
    items.push(UserCreatableItem::new(
        PROCESSOR_FOLDER_NODE_TYPE,
        PROCESSOR_FOLDER_ITEM_KIND,
        "Folder",
    ));
    items
}

fn create_processor_item(node_type: &str) -> Option<Box<dyn Node>> {
    crate::app::create_declared_user_item(node_type, PROCESSOR_ITEM_KIND).or_else(|| {
        (node_type == PROCESSOR_FOLDER_NODE_TYPE)
            .then(|| Box::new(StateProcessorFolder::new()) as Box<dyn Node>)
    })
}

fn initialize_processor_item(node: &mut dyn Node) {
    node.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
}

#[node(
    "state_processor_manager",
    label = "Processors",
    presentation = golden_core::node::PresentationHint {
        show_in_nested_inspector: false,
        ..Default::default()
    }
)]
pub struct StateProcessorManager {}

#[node("state_processor_manager", from_struct)]
impl Node for StateProcessorManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        processor_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_processor_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }
}

#[node("state_processor_folder", label = "Folder")]
pub struct StateProcessorFolder {}

#[node("state_processor_folder", from_struct)]
impl Node for StateProcessorFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        processor_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_processor_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("state_processor_action", label = "Action")]
pub struct ActionStateProcessor {}

#[item(
    "state_processor",
    node = "state_processor_action",
    from_struct,
    menu_path = ["Built-in"]
)]
impl Node for ActionStateProcessor {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        if context != NodeCreationContext::Fresh {
            return;
        }
        let mut conditions = crate::app::ConditionManager::new();
        conditions.node_data_mut().meta.decl_id = DeclId("conditions".to_string());
        ctx.add_child(self.id(), conditions, None);

        let mut true_csq = crate::app::ConsequencesManager::new();
        true_csq.node_data_mut().meta.label = "True Consequences".to_string();
        true_csq.node_data_mut().meta.decl_id = DeclId("true_consequences".to_string());
        ctx.add_child(self.id(), true_csq, None);

        let mut false_csq = crate::app::ConsequencesManager::new();
        false_csq.node_data_mut().meta.label = "False Consequences".to_string();
        false_csq.node_data_mut().meta.decl_id = DeclId("false_consequences".to_string());
        ctx.add_child(self.id(), false_csq, None);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("state_processor_mapping", label = "Mapping")]
pub struct MappingStateProcessor {}

#[item(
    "state_processor",
    node = "state_processor_mapping",
    from_struct,
    menu_path = ["Built-in"]
)]
impl Node for MappingStateProcessor {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        if context != NodeCreationContext::Fresh {
            return;
        }
        let mut filter_chain = crate::app::FilterChainManager::new();
        filter_chain.node_data_mut().meta.decl_id = DeclId("filter_chain".to_string());
        ctx.add_child(self.id(), filter_chain, None);

        let mut outputs = crate::app::OutputsManager::new();
        outputs.node_data_mut().meta.decl_id = DeclId("outputs".to_string());
        ctx.add_child(self.id(), outputs, None);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("state_processor_custom", label = "Custom Processor")]
pub struct CustomStateProcessor {}

#[item("state_processor", node = "state_processor_custom", from_struct)]
impl Node for CustomStateProcessor {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod state_machine_processor_tests;
