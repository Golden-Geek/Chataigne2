//! Columns own typed transition settings and one intrinsic reusable app-control subtree.
use super::protocol::{ColumnDefinition, TransitionCurve, TransitionDefinition};
use golden_core::{
    edit::Edit,
    node,
    node::{Node, NodeId, ParameterHandle, ParameterValidationContext},
    parameter::{Enum, ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessCtx,
};
use std::sync::OnceLock;

pub const COLUMN_TYPE: &str = "mapgyver.matrix.column.v2";

#[node("mapgyver.matrix.column.v2", label = "Column", user_permissions = golden_core::node::NodeUserPermissions {
    can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
})]
#[children(
    folder(parameters, label = "Parameters", can_be_disabled = false) {
        transition_override: bool = false (label = "Override transition");
        duration_ms: f64 = 250.0 [0.0..4294967295.0] (
            label = "Transition duration (ms)", step = 1.0, constraint_policy = "reject"
        );
        curve: Enum = "Linear" (label = "Transition curve", enum_options = crate::parameters::enum_options(&["Linear", "SmoothStep"]));
    }
    node app_control: crate::applications::AppControlNode = (self.create_app_control()) (label = "App control");
)]
pub struct ColumnNode {
    #[state(default = OnceLock::new())]
    runtime: OnceLock<super::playback::RuntimeBinding>,
}
impl ColumnNode {
    pub(crate) fn from_definition(label: String, definition: ColumnDefinition) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        if let Some(transition) = definition.transition {
            node.transition_override = ParameterHandle::new(true);
            node.duration_ms = ParameterHandle::new(f64::from(transition.duration_ms));
            node.curve = ParameterHandle::new(Enum::new(curve_name(transition.curve)));
        }
        node
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
    pub(crate) fn application_node(&self) -> Option<NodeId> {
        self.app_control.current_id()
    }
    pub(crate) fn bind_runtime(&self, binding: super::playback::RuntimeBinding) {
        let _ = self.runtime.set(binding);
    }
    fn invalidate(&self) {
        if let Some(binding) = self.runtime.get() {
            binding.invalidate();
            binding.invalidate_applications();
        }
    }
    pub(crate) fn definition(&self) -> ColumnDefinition {
        ColumnDefinition {
            transition: self.transition_override.get().then(|| TransitionDefinition {
                duration_ms: self.duration_ms.get() as u32,
                curve: if self.curve.get_ref().as_str() == "SmoothStep" {
                    TransitionCurve::SmoothStep
                } else {
                    TransitionCurve::Linear
                },
            }),
        }
    }
    pub(crate) fn configuration_edit(&self, definition: ColumnDefinition) -> Edit {
        let transition = definition.transition.unwrap_or_default();
        Edit::SetParamsAtomic {
            values: vec![
                (
                    self.transition_override.id(),
                    ParamValue::Bool(definition.transition.is_some()),
                ),
                (
                    self.duration_ms.id(),
                    ParamValue::Float(f64::from(transition.duration_ms)),
                ),
                (self.curve.id(), ParamValue::Enum(curve_name(transition.curve).into())),
            ],
            behaviour: ParameterEventBehaviour::Append,
        }
    }
}
#[node("mapgyver.matrix.column.v2", from_struct)]
impl Node for ColumnNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, _: NodeId, _: ParamValue) {
        self.invalidate();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.invalidate();
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn project_decode_data(&mut self, value: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(value)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        validate(self.duration_ms.get(), self.curve.get_ref().as_str())
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        let duration = match change.parameter_value(self.duration_ms.id()) {
            Some(ParamValue::Float(value)) => value,
            _ => self.duration_ms.get(),
        };
        let curve = match change.parameter_value(self.curve.id()) {
            Some(ParamValue::Enum(value)) => value,
            _ => self.curve.get_ref().as_str().into(),
        };
        validate(duration, &curve)
    }
}
fn curve_name(curve: TransitionCurve) -> &'static str {
    match curve {
        TransitionCurve::Linear => "Linear",
        TransitionCurve::SmoothStep => "SmoothStep",
    }
}
fn validate(duration: f64, curve: &str) -> Result<(), String> {
    if !duration.is_finite() || duration.fract() != 0.0 || !(0.0..=f64::from(u32::MAX)).contains(&duration) {
        return Err("Column duration must be a whole number of milliseconds in 0..4294967295".into());
    }
    if !["Linear", "SmoothStep"].contains(&curve) {
        return Err("Unknown Column transition curve".into());
    }
    Ok(())
}
