use golden_core::{
    color,
    events::CustomEvent,
    node,
    node::{
        Node, NodeHandle, NodeId, UserContainerRules, UserContextNode, UserCreatableItem,
        USER_CONTEXT_DEFAULT_LABEL, USER_CONTEXT_ITEM_KIND, USER_CONTEXT_NODE_TYPE,
    },
    process_ctx::ProcessCtx,
    script::ScriptNode,
};

pub(crate) mod common;
mod constants;
mod permissions;
mod reference_filters;
#[cfg(test)]
mod perf_tests;
pub(crate) mod script_api;

pub(crate) use constants::MODULE_ITEM_KIND;
pub(crate) use permissions::{enable_module_authoring, enable_module_manager_authoring};
pub(crate) use reference_filters::{register_module_reference_filters, resolve_enclosing_module_root};

const SCRIPT_ITEM_KIND: &str = "script";
const SCRIPT_NODE_TYPE: &str = "script";
const SCRIPT_DEFAULT_LABEL: &str = "Script";
pub(crate) const MODULE_INCOMING_TRAFFIC_EVENT_TOPIC: &str = "chataigne.module.traffic.incoming";
pub(crate) const MODULE_OUTGOING_TRAFFIC_EVENT_TOPIC: &str = "chataigne.module.traffic.outgoing";

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
    folder(connection, label = "Connection", color = color::Color::new(0.1, 0.75, 0.3, 1.0)) {
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
            description = "Whether incoming module traffic should be recorded in logs.",
            show_in_inspector_content = false
        );
        log_outgoing: bool = false (
            label = "Log Outgoing",
            description = "Whether outgoing module traffic should be recorded in logs.",
            show_in_inspector_content = false
        );
    }
    folder(parameters, label = "Parameters", color = color::Color::new(0.8, 0.5, 0.1, 1.0)) {}
    folder(values, label = "Values", color = color::Color::new(0.1, 0.45, 0.75, 1.0)) {}
    node command_tester: crate::app::ModuleCommandTester = crate::app::ModuleCommandTester::create_empty() (
        label = "Command Tester",
        description = "Create and trigger ad-hoc commands through this module."
    );
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
    pub fn connection_id(&self) -> Option<NodeId> {
        self.connection_folder.current_id()
    }

    pub fn parameters_id(&self) -> Option<NodeId> {
        self.parameters_folder.current_id()
    }

    pub fn values_id(&self) -> Option<NodeId> {
        self.values_folder.current_id()
    }

    pub fn command_tester_id(&self) -> Option<NodeId> {
        self.command_tester.current_id()
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

    pub(crate) fn configure_command_tester(
        &mut self,
        ctx: &mut ProcessCtx,
        available_command_types: &'static [&'static str],
    ) {
        let Some(command_tester_id) = self
            .command_tester_id()
            .or_else(|| ctx.tree_snapshot().and_then(|snapshot| snapshot.find_child_by_decl_id(self.id(), "command_tester")))
        else {
            return;
        };

        let is_command_tester = ctx
            .tree_snapshot()
            .and_then(|snapshot| snapshot.node(command_tester_id))
            .is_some_and(|node| node.node_type == crate::app::ModuleCommandTester::NODE_TYPE);

        if is_command_tester {
            NodeHandle::new(command_tester_id).with_mut::<crate::app::ModuleCommandTester, _>(
                ctx,
                move |tester, _| tester.set_available_command_types(available_command_types),
            );
            return;
        }

        NodeHandle::new(command_tester_id)
            .replace_with(ctx, crate::app::ModuleCommandTester::create(available_command_types));
    }

    pub(crate) fn set_data_capabilities(&mut self, ctx: &mut ProcessCtx, capabilities: ModuleDataCapabilities) {
        self.can_receive.set(ctx, capabilities.incoming);
        self.can_send.set(ctx, capabilities.outgoing);
    }

    pub(crate) fn emit_script_param_callback(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
        param: NodeId,
        old_value: &golden_core::parameter::ParamValue,
    ) {
        script_api::emit_standard_module_param_callback(
            ctx,
            snapshot,
            self.id(),
            self.connection_id(),
            self.parameters_id(),
            self.values_id(),
            param,
            old_value,
        );
    }

    pub fn emit_incoming_traffic(&self, ctx: &mut ProcessCtx) {
        self.emit_traffic(ctx, MODULE_INCOMING_TRAFFIC_EVENT_TOPIC);
    }

    pub fn emit_outgoing_traffic(&self, ctx: &mut ProcessCtx) {
        self.emit_traffic(ctx, MODULE_OUTGOING_TRAFFIC_EVENT_TOPIC);
    }

    fn emit_traffic(&self, ctx: &mut ProcessCtx, topic: &str) {
        ctx.emit_custom_event(CustomEvent::new(topic, Some(self.id()), serde_json::Value::Null));
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
        self.create_user_item_for_host(self.get_type(), node_type)
    }

    fn create_user_item_for_host(&self, host_node_type: &str, node_type: &str) -> Option<Box<dyn Node>> {
        let node_type = node_type.trim().to_ascii_lowercase();
        if node_type == SCRIPT_NODE_TYPE && self.script_host_policy().is_some_and(|policy| policy.enabled) {
            return Some(Box::new(ScriptNode::new(
                SCRIPT_DEFAULT_LABEL,
                script_api::module_script_config_for_node(self.node_data(), host_node_type),
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
