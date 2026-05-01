use golden_core::{
    node,
    node::{
        Node, NodeId, UserContainerRules, UserContextNode, UserCreatableItem,
        USER_CONTEXT_DEFAULT_LABEL, USER_CONTEXT_ITEM_KIND, USER_CONTEXT_NODE_TYPE,
    },
    process_ctx::ProcessCtx,
    script::{ScriptNode, ScriptNodeConfig},
};

pub(crate) mod common;
mod constants;
mod permissions;
mod reference_filters;

pub(crate) use permissions::{enable_module_authoring, enable_module_manager_authoring};
pub(crate) use reference_filters::{register_module_reference_filters, resolve_enclosing_module_root};

#[node("module_base", label = "Module")]
#[children(
    folder(infos, label = "Infos") {
        connected: bool = false (
            label = "Connected",
            description = "Whether the module is currently connected to its remote interface.",
            read_only = true
        );
        log_incoming: bool = false (
            label = "Log Incoming",
            description = "Whether incoming module traffic should be recorded in logs."
        );
        log_outgoing: bool = false (
            label = "Log Outgoing",
            description = "Whether outgoing module traffic should be recorded in logs."
        );
    }
    folder(parameters, label = "Parameters") {}
    folder(values, label = "Values") {}
)]
pub struct ModuleBase {
    #[potential_node(decl_id = "infos")]
    infos_folder: golden_core::node::PotentialNodeHandle,
    #[potential_node(decl_id = "parameters")]
    parameters_folder: golden_core::node::PotentialNodeHandle,
    #[potential_node(decl_id = "values")]
    values_folder: golden_core::node::PotentialNodeHandle,
}

impl ModuleBase {
    pub fn parameters_id(&self) -> Option<NodeId> {
        self.parameters_folder.current_id()
    }

    pub fn values_id(&self) -> Option<NodeId> {
        self.values_folder.current_id()
    }

    pub fn log_incoming_enabled(&self) -> bool {
        self.log_incoming.get()
    }

    pub fn log_outgoing_enabled(&self) -> bool {
        self.log_outgoing.get()
    }

    pub fn set_connected(&mut self, ctx: &mut ProcessCtx, connected: bool) {
        self.connected.set(ctx, connected);
    }
}

#[node("module_base", from_struct, scriptable, contextualizable)]
impl Node for ModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        permissions::initialize_module_root(self, ctx);
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            crate::app::descriptors::SCRIPT_ITEM_KIND,
            USER_CONTEXT_ITEM_KIND,
        ]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = Vec::new();
        if self.script_host_policy().is_some_and(|policy| policy.enabled) {
            let descriptor = crate::app::descriptors::SCRIPT_ITEM;
            items.push(
                UserCreatableItem::new(
                    descriptor.type_id,
                    descriptor.user_item_kind.unwrap_or(crate::app::descriptors::SCRIPT_ITEM_KIND),
                    descriptor.display_name,
                )
                .with_select_when_created(false),
            );
        }
        if self.user_context_host_policy().is_some_and(|policy| policy.enabled) {
            items.push(
                UserCreatableItem::new(
                    USER_CONTEXT_NODE_TYPE,
                    USER_CONTEXT_ITEM_KIND,
                    USER_CONTEXT_DEFAULT_LABEL,
                )
                .with_select_when_created(false),
            );
        }
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        let node_type = node_type.trim().to_ascii_lowercase();
        if node_type == crate::app::descriptors::SCRIPT_NODE_TYPE
            && self.script_host_policy().is_some_and(|policy| policy.enabled)
        {
            return Some(Box::new(ScriptNode::new(
                "Script",
                ScriptNodeConfig::for_host_node_type(self.get_type()),
            )));
        }

        if (node_type == USER_CONTEXT_NODE_TYPE || node_type == "context")
            && self.user_context_host_policy().is_some_and(|policy| policy.enabled)
        {
            return Some(Box::new(UserContextNode::new(USER_CONTEXT_DEFAULT_LABEL)));
        }

        None
    }

    fn user_item_kind(&self) -> &str {
        crate::app::descriptors::MODULE_ITEM_KIND
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == crate::app::descriptors::MODULE_BASE.type_id).then(Self::new)
    }
}
