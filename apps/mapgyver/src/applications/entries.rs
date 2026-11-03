//! Ordered process arguments and explicit environment entries are actual authored child nodes.
use golden_core::{
    node,
    node::{Node, NodeId, ParameterHandle, ParameterValidationContext},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

pub(crate) const ARGUMENT_TYPE: &str = "mapgyver.app_control.argument.v1";
pub(crate) const ENVIRONMENT_TYPE: &str = "mapgyver.app_control.environment.v1";

fn invalidate(parent: Option<NodeId>, ctx: &mut ProcessCtx) {
    if let Some(parent) = parent {
        ctx.call_node_mutation_without_snapshot(parent, |node, _| {
            let owner = node
                .as_any_mut()
                .downcast_mut::<super::AppControlNode>()
                .ok_or("app-control entry has no owner")?;
            owner.invalidate();
            Ok(())
        });
    }
}
fn text(value: &str, name: bool) -> Result<(), String> {
    if value.contains('\0') || value.len() > 32 * 1024 || (name && (value.is_empty() || value.contains('='))) {
        Err("app-control strings must be NUL-free and bounded; environment names must be nonempty without '='".into())
    } else {
        Ok(())
    }
}

#[node(
    "mapgyver.app_control.argument.v1", label = "Argument", can_be_disabled = false,
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
    }
)]
#[children(folder(parameters, label = "Parameters", can_be_disabled = false) {
    value: String = "" (label = "Value");
})]
pub struct ArgumentNode {}
impl ArgumentNode {
    pub(super) fn from_value(value: String) -> Self {
        let mut node = Self::new();
        node.value = ParameterHandle::new(value);
        node
    }
    pub(super) fn project_value(&self, change: Option<&ParameterValidationContext<'_>>) -> Result<String, String> {
        super::node::read(&self.value, change)
    }
    pub(super) fn parameter_value(&self, value: String) -> (NodeId, ParamValue) {
        (self.value.id(), ParamValue::Str(value))
    }
}
#[node("mapgyver.app_control.argument.v1", from_struct)]
impl Node for ArgumentNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, _: NodeId, _: ParamValue) {
        invalidate(self.node_data().parent, ctx);
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        text(self.value.get_ref(), false)
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if let ParamValue::Str(value) = change.value {
            text(value, false)?;
        }
        Ok(())
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
}

#[node(
    "mapgyver.app_control.environment.v1", label = "Environment", can_be_disabled = false,
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
    }
)]
#[children(folder(parameters, label = "Parameters", can_be_disabled = false) {
    name: String = "VARIABLE" (label = "Name");
    value: String = "" (label = "Value");
})]
pub struct EnvironmentNode {}
impl EnvironmentNode {
    pub(super) fn from_entry(name: String, value: String) -> Self {
        let mut node = Self::new();
        node.name = ParameterHandle::new(name);
        node.value = ParameterHandle::new(value);
        node
    }
    pub(super) fn project_entry(
        &self,
        change: Option<&ParameterValidationContext<'_>>,
    ) -> Result<(String, String), String> {
        Ok((
            super::node::read(&self.name, change)?,
            super::node::read(&self.value, change)?,
        ))
    }
    pub(super) fn parameter_values(&self, name: String, value: String) -> [(NodeId, ParamValue); 2] {
        [(self.name.id(), name), (self.value.id(), value)].map(|(node, value)| (node, ParamValue::Str(value)))
    }
}
#[node("mapgyver.app_control.environment.v1", from_struct)]
impl Node for EnvironmentNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, _: NodeId, _: ParamValue) {
        invalidate(self.node_data().parent, ctx);
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        text(self.name.get_ref(), true)?;
        text(self.value.get_ref(), false)
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if let ParamValue::Str(value) = change.value {
            text(value, change.parameter == self.name.id())?;
        }
        Ok(())
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
}
