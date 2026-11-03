//! Layer/Group settings are authored Parameters; definitions are read projections only.
use super::protocol::*;
use golden_core::{
    edit::Edit,
    node,
    node::{Node, NodeId, ParameterHandle, ParameterValidationContext, UserContainerRules},
    parameter::{Enum, ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessCtx,
};
use std::sync::OnceLock;

pub const LAYER_TYPE: &str = "mapgyver.matrix.layer.v2";
pub const GROUP_TYPE: &str = "mapgyver.matrix.group.v2";
pub use super::cell_node::CELL_TYPE;
pub use super::column_node::COLUMN_TYPE;

#[derive(Clone, Copy)]
enum Kind {
    Layer,
    Group,
}

#[node("mapgyver.matrix.element.v2", label = "Matrix element", user_permissions = golden_core::node::NodeUserPermissions {
    can_edit_name: true, ..Default::default()
})]
#[children(folder(parameters, label = "Parameters", can_be_disabled = false) {
    opacity: f64 = 1.0 [0.0..1.0] (label = "Opacity", constraint_policy = "reject");
    blend: Enum = "Normal" (label = "Blend", enum_options = crate::parameters::enum_options(&["Normal", "Add", "Multiply", "Screen"]));
    render_enabled: bool = true (label = "Render enabled");
})]
pub struct ElementNode {
    #[state(default = Kind::Layer)]
    kind: Kind,
    #[state(default = OnceLock::new())]
    runtime: OnceLock<super::playback::RuntimeBinding>,
}
impl ElementNode {
    pub(crate) fn from_definition(label: String, definition: ElementDefinition) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        let settings = match definition {
            ElementDefinition::Layer { settings } => settings,
            ElementDefinition::Group { settings } => {
                node.kind = Kind::Group;
                settings
            }
        };
        node.opacity = ParameterHandle::new(f64::from(settings.opacity));
        node.blend = ParameterHandle::new(Enum::new(blend_name(settings.blend)));
        node.render_enabled = ParameterHandle::new(settings.enabled);
        node
    }
    pub(crate) fn definition(&self) -> ElementDefinition {
        let settings = CompositeSettings {
            opacity: self.opacity.get() as f32,
            blend: match self.blend.get_ref().as_str() {
                "Add" => golden_video::plan::BlendMode::Add,
                "Multiply" => golden_video::plan::BlendMode::Multiply,
                "Screen" => golden_video::plan::BlendMode::Screen,
                _ => golden_video::plan::BlendMode::Normal,
            },
            enabled: self.render_enabled.get(),
        };
        match self.kind {
            Kind::Layer => ElementDefinition::Layer { settings },
            Kind::Group => ElementDefinition::Group { settings },
        }
    }
    pub(super) fn configuration_edit(&self, settings: CompositeSettings) -> Edit {
        Edit::SetParamsAtomic {
            values: vec![
                (self.opacity.id(), ParamValue::Float(f64::from(settings.opacity))),
                (self.blend.id(), ParamValue::Enum(blend_name(settings.blend).into())),
                (self.render_enabled.id(), ParamValue::Bool(settings.enabled)),
            ],
            behaviour: ParameterEventBehaviour::Append,
        }
    }
    pub(super) fn bind_runtime(&self, binding: super::playback::RuntimeBinding) {
        let _ = self.runtime.set(binding);
    }
    fn invalidate(&self) {
        if let Some(binding) = self.runtime.get() {
            binding.invalidate();
        }
    }
    fn validate(&self, change: Option<&ParameterValidationContext<'_>>) -> Result<(), String> {
        let opacity = match change.and_then(|change| change.parameter_value(self.opacity.id())) {
            Some(ParamValue::Float(value)) => value,
            _ => self.opacity.get(),
        };
        let blend = match change.and_then(|change| change.parameter_value(self.blend.id())) {
            Some(ParamValue::Enum(value)) => value,
            _ => self.blend.get_ref().as_str().into(),
        };
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err("Matrix opacity must be finite in 0..1".into());
        }
        if !["Normal", "Add", "Multiply", "Screen"].contains(&blend.as_str()) {
            return Err("Unknown Matrix blend mode".into());
        }
        Ok(())
    }
}
pub(crate) fn create(node_type: &str) -> Option<ElementNode> {
    let definition = match node_type {
        LAYER_TYPE => ElementDefinition::Layer {
            settings: CompositeSettings::default(),
        },
        GROUP_TYPE => ElementDefinition::Group {
            settings: CompositeSettings::default(),
        },
        _ => return None,
    };
    Some(ElementNode::from_definition("Matrix element".into(), definition))
}
#[node("mapgyver.matrix.element.v2", from_struct)]
impl Node for ElementNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn get_type(&self) -> &str {
        match self.kind {
            Kind::Layer => LAYER_TYPE,
            Kind::Group => GROUP_TYPE,
        }
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(match self.kind {
            Kind::Group => UserContainerRules::new(&[GROUP_TYPE, LAYER_TYPE]),
            Kind::Layer => UserContainerRules::new(&[CELL_TYPE]),
        })
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if matches!(self.kind, Kind::Layer) && node_type == CELL_TYPE {
            return Some(Box::new(super::CellNode::new()));
        }
        create(node_type).map(|node| Box::new(node) as Box<dyn Node>)
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.invalidate();
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, _: NodeId, _: ParamValue) {
        self.invalidate();
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn project_create(node_type: &str) -> Option<Self> {
        create(node_type)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        self.validate(None)
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        self.validate(Some(change))
    }
}
fn blend_name(blend: golden_video::plan::BlendMode) -> &'static str {
    match blend {
        golden_video::plan::BlendMode::Normal => "Normal",
        golden_video::plan::BlendMode::Add => "Add",
        golden_video::plan::BlendMode::Multiply => "Multiply",
        golden_video::plan::BlendMode::Screen => "Screen",
    }
}
