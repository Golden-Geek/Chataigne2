//! Composition settings are declared Golden parameters; definitions are read projections.
use super::protocol::{
    BlendMode, InstanceDefinition, InstanceGeometryMode, InstanceId, InstanceParameterBindings, Transform2D,
};
use crate::media::protocol::{ChannelId, ChannelReference, MediaId};
use golden_core::{
    node,
    node::{Node, NodeId, NodeReference, ParameterHandle, ParameterValidationContext},
    parameter::{Enum, ParamValue, ReferenceTargetKind, Vec2},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};

pub const PLACEMENT_TYPE: &str = "mapgyver.composition.placement.v2";

#[node("mapgyver.composition.placement.v2", label = "Placement")]
#[children(
    folder(parameters, label = "Parameters") {
        source_channel: NodeReference = NodeReference::empty() (
            label = "Output", description = "Empty follows the first available output.", reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.channel".into()]
        );
        position: Vec2 = Vec2::new(0.0, 0.0) (label = "Position");
        scale: Vec2 = Vec2::new(1.0, 1.0) (label = "Scale");
        rotation_degrees: f64 = 0.0 (label = "Rotation", description = "Clockwise rotation in degrees.");
        pivot: Vec2 = Vec2::new(0.5, 0.5) (label = "Pivot");
        opacity: f64 = 1.0 [0.0..1.0] (label = "Opacity", constraint_policy = "reject");
        blend: Enum = "Normal" (label = "Blend", enum_options = crate::parameters::enum_options(&["Normal", "Add", "Multiply", "Screen"]));
        render_enabled: bool = true (label = "Enabled");
    }
)]
pub struct PlacementNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
}

impl PlacementNode {
    pub(crate) fn parameter_bindings(
        &self,
        engine: &crate::app::AppEngine,
        instance: InstanceId,
    ) -> InstanceParameterBindings {
        let uuid = |id| {
            engine
                .nodes
                .get(id)
                .expect("materialized Placement parameter")
                .node_data()
                .meta
                .uuid
        };
        InstanceParameterBindings {
            instance,
            position: uuid(self.position.id()),
            scale: uuid(self.scale.id()),
            rotation_degrees: uuid(self.rotation_degrees.id()),
            pivot: uuid(self.pivot.id()),
            opacity: uuid(self.opacity.id()),
            enabled: uuid(self.render_enabled.id()),
        }
    }
    pub(crate) fn geometry_edits(
        &self,
        definition: InstanceDefinition,
        mode: InstanceGeometryMode,
    ) -> Vec<golden_core::edit::Edit> {
        use golden_core::{edit::Edit, parameter::ParameterEventBehaviour};
        let write = |node, value| Edit::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        };
        let position = definition.transform.position.map(f64::from);
        match mode {
            InstanceGeometryMode::Translate => {
                vec![write(self.position.id(), ParamValue::Vec2(position[0], position[1]))]
            }
            InstanceGeometryMode::Rotate => vec![write(
                self.rotation_degrees.id(),
                ParamValue::Float(f64::from(definition.transform.rotation_degrees)),
            )],
            InstanceGeometryMode::Resize { .. } => {
                let scale = definition.transform.scale.map(f64::from);
                vec![
                    write(self.position.id(), ParamValue::Vec2(position[0], position[1])),
                    write(self.scale.id(), ParamValue::Vec2(scale[0], scale[1])),
                ]
            }
        }
    }
    pub(crate) fn output(&self) -> Option<ChannelId> {
        let uuid = self.source_channel.get_ref().uuid;
        (!uuid.0.is_nil()).then_some(ChannelId(uuid))
    }
    pub(crate) fn output_parameter(&self) -> NodeId {
        self.source_channel.id()
    }
    pub(crate) fn position_parameter(&self) -> NodeId {
        self.position.id()
    }
    pub(crate) fn from_definition(label: String, definition: InstanceDefinition) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        let channel = source_reference(definition.source);
        node.source_channel = ParameterHandle::new(channel);
        node.position = ParameterHandle::new(vector(definition.transform.position));
        node.scale = ParameterHandle::new(vector(definition.transform.scale));
        node.rotation_degrees = ParameterHandle::new(f64::from(definition.transform.rotation_degrees));
        node.pivot = ParameterHandle::new(vector(definition.transform.pivot));
        node.opacity = ParameterHandle::new(f64::from(definition.opacity));
        node.blend = ParameterHandle::new(Enum::new(blend_name(definition.blend)));
        node.render_enabled = ParameterHandle::new(definition.enabled);
        node
    }
    pub(crate) fn definition(&self, media: MediaId) -> InstanceDefinition {
        let channel = self.source_channel.get_ref().uuid;
        InstanceDefinition {
            source: (!channel.0.is_nil()).then_some(ChannelReference {
                media,
                channel: ChannelId(channel),
            }),
            transform: Transform2D {
                position: components(self.position.get()),
                scale: components(self.scale.get()),
                rotation_degrees: self.rotation_degrees.get() as f32,
                pivot: components(self.pivot.get()),
            },
            opacity: self.opacity.get() as f32,
            blend: match self.blend.get_ref().as_str() {
                "Add" => BlendMode::Add,
                "Multiply" => BlendMode::Multiply,
                "Screen" => BlendMode::Screen,
                _ => BlendMode::Normal,
            },
            enabled: self.render_enabled.get(),
        }
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn parameter_edits(&self, definition: InstanceDefinition) -> Vec<golden_core::edit::Edit> {
        use golden_core::{edit::Edit, parameter::ParameterEventBehaviour};
        let position = vector(definition.transform.position);
        let scale = vector(definition.transform.scale);
        let pivot = vector(definition.transform.pivot);
        [
            (self.position.id(), ParamValue::Vec2(position.x, position.y)),
            (self.scale.id(), ParamValue::Vec2(scale.x, scale.y)),
            (
                self.rotation_degrees.id(),
                ParamValue::Float(f64::from(definition.transform.rotation_degrees)),
            ),
            (self.pivot.id(), ParamValue::Vec2(pivot.x, pivot.y)),
            (self.opacity.id(), ParamValue::Float(f64::from(definition.opacity))),
            (self.blend.id(), ParamValue::Enum(blend_name(definition.blend).into())),
            (self.render_enabled.id(), ParamValue::Bool(definition.enabled)),
        ]
        .into_iter()
        .map(|(node, value)| Edit::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        })
        .collect()
    }
}

#[node("mapgyver.composition.placement.v2", from_struct)]
impl Node for PlacementNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.revision.advance();
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, _: NodeId, _: ParamValue) {
        self.revision.advance();
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        if !["Normal", "Add", "Multiply", "Screen"].contains(&self.blend.get_ref().as_str()) {
            return Err("Unknown Composition blend mode".into());
        }
        self.definition(MediaId(golden_core::node::NodeUuid::nil())).validate()
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        if !data.is_null() {
            return Err(
                "Composition instances use parameter children; opaque definition records are unsupported".into(),
            );
        }
        Ok(())
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.scale.id()
            && let ParamValue::Vec2(x, y) = change.value
            && (!x.is_finite()
                || !y.is_finite()
                || (*x as f32) == 0.0
                || (*y as f32) == 0.0
                || !(*x as f32).is_finite()
                || !(*y as f32).is_finite())
        {
            return Err("Composition scale must be finite and nonzero".into());
        }
        if [self.position.id(), self.pivot.id()].contains(&change.parameter)
            && let ParamValue::Vec2(x, y) = change.value
            && (!(*x as f32).is_finite() || !(*y as f32).is_finite())
        {
            return Err("Composition transform must be finite".into());
        }
        if change.parameter == self.rotation_degrees.id()
            && let ParamValue::Float(value) = change.value
            && !(*value as f32).is_finite()
        {
            return Err("Composition rotation must be finite".into());
        }
        if change.parameter == self.source_channel.id()
            && let ParamValue::Reference(output) = change.value
            && let Some(media) = self.node_data().parent.and_then(|id| change.node(id))
        {
            crate::parameters::reference_cycle::validate_output(change, media.node_data().meta.uuid, output.uuid)?;
        }
        Ok(())
    }
}

fn vector(value: [f32; 2]) -> Vec2 {
    Vec2::new(f64::from(value[0]), f64::from(value[1]))
}
fn components(value: Vec2) -> [f32; 2] {
    [value.x as f32, value.y as f32]
}
fn source_reference(source: Option<ChannelReference>) -> NodeReference {
    source.map_or_else(NodeReference::empty, |source| NodeReference::new(source.channel.0))
}
fn blend_name(blend: BlendMode) -> &'static str {
    match blend {
        BlendMode::Normal => "Normal",
        BlendMode::Add => "Add",
        BlendMode::Multiply => "Multiply",
        BlendMode::Screen => "Screen",
    }
}
