use golden_core::{
    events::CustomEvent,
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

pub(crate) use constants::MODULE_ITEM_KIND;
pub(crate) use permissions::{enable_module_authoring, enable_module_manager_authoring};
pub(crate) use reference_filters::{register_module_reference_filters, resolve_enclosing_module_root};

const SCRIPT_ITEM_KIND: &str = "script";
const SCRIPT_NODE_TYPE: &str = "script";
const SCRIPT_DEFAULT_LABEL: &str = "Script";

pub(crate) const MODULE_INCOMING_TRAFFIC_EVENT_TOPIC: &str = "chataigne.module.traffic.incoming";
pub(crate) const MODULE_OUTGOING_TRAFFIC_EVENT_TOPIC: &str = "chataigne.module.traffic.outgoing";
pub(crate) const MODULE_TRAFFIC_INCOMING_DIRECTION: &str = "incoming";
pub(crate) const MODULE_TRAFFIC_OUTGOING_DIRECTION: &str = "outgoing";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ModuleDataCapabilities {
    incoming: bool,
    outgoing: bool,
}

impl ModuleDataCapabilities {
    pub(crate) const fn new(incoming: bool, outgoing: bool) -> Self {
        Self { incoming, outgoing }
    }
}

#[node("module_base", label = "Module")]
#[children(
    folder(connection, label = "Connection") {
        connected: bool = false (
            label = "Connected",
            description = "Whether the module is currently connected to its remote interface.",
            read_only = true
        );
        can_receive: bool = false (
            label = "Can Receive",
            description = "Whether the current module configuration can receive external data.",
            read_only = true,
            show_in_inspector_content = false
        );
        can_send: bool = false (
            label = "Can Send",
            description = "Whether the current module configuration can send external data.",
            read_only = true,
            show_in_inspector_content = false
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
    #[potential_node(decl_id = "connection")]
    connection_folder: golden_core::node::PotentialNodeHandle,
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

    pub(crate) fn set_data_capabilities(&mut self, ctx: &mut ProcessCtx, capabilities: ModuleDataCapabilities) {
        self.can_receive.set(ctx, capabilities.incoming);
        self.can_send.set(ctx, capabilities.outgoing);
    }

    pub fn emit_incoming_traffic(&self, ctx: &mut ProcessCtx) {
        self.emit_traffic(
            ctx,
            MODULE_INCOMING_TRAFFIC_EVENT_TOPIC,
            MODULE_TRAFFIC_INCOMING_DIRECTION,
        );
    }

    pub fn emit_outgoing_traffic(&self, ctx: &mut ProcessCtx) {
        self.emit_traffic(
            ctx,
            MODULE_OUTGOING_TRAFFIC_EVENT_TOPIC,
            MODULE_TRAFFIC_OUTGOING_DIRECTION,
        );
    }

    fn emit_traffic(&self, ctx: &mut ProcessCtx, topic: &str, direction: &str) {
        ctx.emit_custom_event(CustomEvent::new(
            topic,
            Some(self.id()),
            serde_json::json!({ "direction": direction }),
        ));
    }
}

#[node("module_base", from_struct, scriptable, contextualizable)]
impl Node for ModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        permissions::initialize_module_root(self, ctx);
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[SCRIPT_ITEM_KIND, USER_CONTEXT_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = Vec::new();
        if self.script_host_policy().is_some_and(|policy| policy.enabled) {
            items.push(
                UserCreatableItem::new(SCRIPT_NODE_TYPE, SCRIPT_ITEM_KIND, SCRIPT_DEFAULT_LABEL)
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
        if node_type == SCRIPT_NODE_TYPE && self.script_host_policy().is_some_and(|policy| policy.enabled) {
            return Some(Box::new(ScriptNode::new(
                SCRIPT_DEFAULT_LABEL,
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
        MODULE_ITEM_KIND
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
