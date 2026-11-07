//! Column hierarchy and intrinsic app-control ownership, separate from compositing Layer Groups.
use golden_core::{
    node,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};
use std::sync::OnceLock;

pub const COLUMN_GROUP_TYPE: &str = "mapgyver.matrix.column_group.v1";

#[node("mapgyver.matrix.column_group.v1", label = "Column Group", user_permissions = golden_core::node::NodeUserPermissions {
    can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
})]
#[children(
    folder(parameters, label = "Parameters", can_be_disabled = false) {
        stop: ParamValue = ParamValue::Trigger() (label = "Stop");
    }
    node app_control: crate::applications::AppControlNode = (self.create_app_control()) (label = "App control");
)]
pub struct ColumnGroupNode {
    #[state(default = super::controls::Pulses::default())]
    pulses: super::controls::Pulses,
    #[state(default = OnceLock::new())]
    runtime: OnceLock<super::playback::RuntimeBinding>,
}

impl ColumnGroupNode {
    pub(crate) fn named(label: String) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        node
    }
    pub(crate) fn application_node(&self) -> Option<NodeId> {
        self.app_control.current_id()
    }
    pub(crate) fn bind_runtime(&self, binding: super::playback::RuntimeBinding) {
        let _ = self.runtime.set(binding);
    }
    fn create_app_control(&self) -> crate::applications::AppControlNode {
        let node = crate::applications::AppControlNode::embedded();
        if let Some(binding) = self.runtime.get().cloned() {
            node.bind_invalidation(move || {
                binding.invalidate();
                binding.invalidate_applications();
            });
        }
        node
    }
    fn invalidate(&self) {
        if let Some(binding) = self.runtime.get() {
            binding.invalidate();
            binding.invalidate_applications();
        }
    }
}

#[node("mapgyver.matrix.column_group.v1", from_struct)]
impl Node for ColumnGroupNode {
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn on_node_ready(&mut self, _: &mut ProcessCtx, _: golden_core::node::NodeCreationContext) {
        self.pulses.ready();
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        if parameter == self.stop.id() {
            self.pulses.invoke(ctx, &self.node_data, super::controls::Action::Stop);
        }
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        super::controls::Pulses::result(ctx, self.id(), &event);
    }
    fn user_container_rules(&self) -> Option<golden_core::node::UserContainerRules> {
        Some(golden_core::node::UserContainerRules::new(&[
            super::column_node::COLUMN_TYPE,
            COLUMN_GROUP_TYPE,
        ]))
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            super::column_node::COLUMN_TYPE => Some(Box::new(super::ColumnNode::new())),
            COLUMN_GROUP_TYPE => Some(Box::new(Self::new())),
            _ => None,
        }
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
        if !enabled {
            self.pulses.revoke();
        }
        self.invalidate();
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.pulses.destroy();
        self.invalidate();
    }
    fn project_decode_data(&mut self, value: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(value)
    }
}
