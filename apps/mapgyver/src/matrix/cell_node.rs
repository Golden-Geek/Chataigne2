//! Typed sparse address and selected output; content is an actual owned media child.
use super::protocol::{CellDefinition, ColumnId, TransitionCurve, TransitionDefinition};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{ChannelId, MediaId},
};
use golden_core::{
    node,
    node::{Node, NodeId, NodeReference, ParameterHandle, ParameterValidationContext, UserContainerRules},
    parameter::{Enum, ParamValue, ReferenceTargetKind},
    process_ctx::ProcessCtx,
};
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};

pub const CELL_TYPE: &str = "mapgyver.matrix.cell.v2";
#[node("mapgyver.matrix.cell.v2", label = "Cell")]
#[children(folder(parameters, label = "Parameters") {
    column: NodeReference = NodeReference::empty() (label = "Column", read_only = true, persist_read_only_value = true,
        reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.matrix.column.v2".into()]);
    output: NodeReference = NodeReference::empty() (label = "Output", description = "Empty follows the first available output.",
        reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.media.channel".into()]);
    transition_override: bool = false (label = "Override transition");
    duration_ms: f64 = 250.0 [0.0..4294967295.0] (label = "Transition duration (ms)", step = 1.0, constraint_policy = "reject");
    curve: Enum = "Linear" (label = "Transition curve", enum_options = crate::parameters::enum_options(&["Linear", "SmoothStep"]));
}
node app_control: crate::applications::AppControlNode = (self.create_app_control()) (
    label = "App control"
);
)]
pub struct CellNode {
    #[state(default = OnceLock::new())]
    runtime: OnceLock<super::playback::RuntimeBinding>,
    #[state(default = AtomicU64::new(0))]
    source_version: AtomicU64,
}
impl CellNode {
    pub(crate) fn transition(&self) -> Option<TransitionDefinition> {
        self.transition_override.get().then(|| TransitionDefinition {
            duration_ms: self.duration_ms.get() as u32,
            curve: if self.curve.get_ref().as_str() == "SmoothStep" {
                TransitionCurve::SmoothStep
            } else {
                TransitionCurve::Linear
            },
        })
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
    pub(crate) fn from_address(label: String, column: ColumnId, output: Option<ChannelId>) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        node.column = ParameterHandle::new(NodeReference::new(column.0));
        node.output = ParameterHandle::new(output.map_or_else(NodeReference::empty, |id| NodeReference::new(id.0)));
        node
    }
    pub(crate) fn column(&self) -> ColumnId {
        ColumnId(self.column.get_ref().uuid)
    }
    pub(crate) fn output_parameter(&self) -> NodeId {
        self.output.id()
    }
    pub(crate) fn content(&self, engine: &AppEngine) -> Result<Option<NodeId>, String> {
        let mut next = self.node_data().first_child;
        let mut found = None;
        while let Some(id) = next {
            let node = engine.nodes.get(id).ok_or("Cell child is missing")?;
            next = node.node_data().next_sibling;
            if crate::parameters::is_parameter_folder(node) || self.application_node() == Some(id) {
                continue;
            }
            if !matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_)) || found.replace(id).is_some() {
                return Err("Cell owns at most one media child".into());
            }
        }
        Ok(found)
    }
    pub(crate) fn selection(&self, engine: &AppEngine) -> Result<Option<crate::media::MediaSource>, String> {
        let output = self.output.get_ref().uuid;
        Ok(self
            .content(engine)?
            .and_then(|id| engine.nodes.get(id))
            .map(|node| crate::media::MediaSource {
                media: MediaId(node.node_data().meta.uuid),
                channel: (!output.0.is_nil()).then_some(ChannelId(output)),
            }))
    }
    pub(crate) fn definition(&self, engine: &AppEngine) -> Result<CellDefinition, String> {
        let source = self
            .selection(engine)?
            .map(|source| source.output(engine))
            .transpose()?
            .flatten();
        Ok(CellDefinition {
            column: self.column(),
            source,
            application: crate::applications::protocol::ApplicationId(self.node_data().meta.uuid),
            transition: self.transition(),
        })
    }
    pub(crate) fn bind_runtime(&self, binding: super::playback::RuntimeBinding) {
        let version = binding.version();
        if self.runtime.set(binding).is_ok() {
            self.source_version.store(version, Ordering::Release);
        }
    }
    pub(crate) fn source_version(&self) -> u64 {
        self.source_version.load(Ordering::Acquire)
    }
    fn invalidate(&self) {
        if let Some(binding) = self.runtime.get() {
            binding.invalidate();
            self.source_version.store(binding.version(), Ordering::Release);
        }
    }
}
#[node("mapgyver.matrix.cell.v2", from_struct)]
impl Node for CellNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            "mapgyver.media.v5",
            "mapgyver.media.reference.v1",
        ]))
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.invalidate();
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        if [self.transition_override.id(), self.duration_ms.id(), self.curve.id()].contains(&parameter) {
            if let Some(binding) = self.runtime.get() {
                binding.invalidate();
            }
            return;
        }
        self.invalidate();
        if parameter == self.column.id()
            && let Some(binding) = self.runtime.get()
        {
            binding.invalidate_applications();
        }
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        super::column_node::validate(self.duration_ms.get(), self.curve.get_ref().as_str())
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
        super::column_node::validate(duration, &curve)?;
        if change.parameter == self.column.id()
            && let ParamValue::Reference(column) = change.value
            && let Some(column) = change.resolve(column.uuid).and_then(|id| change.node(id))
        {
            let mut parent = self.node_data().parent;
            while let Some(id) = parent {
                let Some(node) = change.node(id) else { break };
                if node
                    .as_any()
                    .downcast_ref::<crate::media::ChannelNode>()
                    .is_some_and(|node| node.is_matrix_final())
                {
                    if column.node_data().parent != Some(id) {
                        return Err("Cell column belongs to another Matrix".into());
                    }
                    break;
                }
                parent = node.node_data().parent;
            }
        }
        if change.parameter == self.output.id()
            && let ParamValue::Reference(output) = change.value
        {
            let mut child = self.node_data().first_child;
            while let Some(id) = child {
                let Some(node) = change.node(id) else { break };
                child = node.node_data().next_sibling;
                if node.as_any().is::<crate::media::MediaNode>() || node.as_any().is::<crate::media::ReferenceNode>() {
                    crate::parameters::reference_cycle::validate_output(
                        change,
                        node.node_data().meta.uuid,
                        output.uuid,
                    )?;
                }
            }
        }
        Ok(())
    }
}
