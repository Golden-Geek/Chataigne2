//! Composition settings are declared Golden parameters; definitions are read projections.
use super::protocol::{BlendMode, InstanceDefinition, InstanceId, Transform2D};
use crate::media::protocol::{ChannelId, ChannelReference, MediaId};
use golden_core::{
    node,
    node::{Node, NodeId, NodeReference, ParameterHandle, ParameterValidationContext},
    parameter::{Enum, ParamValue, ReferenceTargetKind, Vec2},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};

pub const INSTANCE_TYPE: &str = "mapgyver.composition.instance";

#[node("mapgyver.composition.instance", label = "Instance")]
#[children(
    folder(parameters, label = "Parameters") {
        source_media: NodeReference = NodeReference::empty() (
            label = "Source media", reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.v4".into()]
        );
        source_channel: NodeReference = NodeReference::empty() (
            label = "Source channel", reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.channel".into()]
        );
        position: Vec2 = Vec2::new(0.0, 0.0) (label = "Position");
        scale: Vec2 = Vec2::new(1.0, 1.0) (label = "Scale");
        rotation: f64 = 0.0 (label = "Rotation", description = "Rotation in radians.");
        pivot: Vec2 = Vec2::new(0.5, 0.5) (label = "Pivot");
        opacity: f64 = 1.0 [0.0..1.0] (label = "Opacity", constraint_policy = "reject");
        blend: Enum = "Normal" (label = "Blend", enum_options = crate::parameters::enum_options(&["Normal", "Add", "Multiply", "Screen"]));
        render_enabled: bool = true (label = "Enabled");
    }
)]
pub struct InstanceNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
}

impl InstanceNode {
    pub(crate) fn from_definition(label: String, definition: InstanceDefinition) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        let (media, channel) = source_references(definition.source);
        node.source_media = ParameterHandle::new(media);
        node.source_channel = ParameterHandle::new(channel);
        node.position = ParameterHandle::new(vector(definition.transform.position));
        node.scale = ParameterHandle::new(vector(definition.transform.scale));
        node.rotation = ParameterHandle::new(f64::from(definition.transform.rotation));
        node.pivot = ParameterHandle::new(vector(definition.transform.pivot));
        node.opacity = ParameterHandle::new(f64::from(definition.opacity));
        node.blend = ParameterHandle::new(Enum::new(blend_name(definition.blend)));
        node.render_enabled = ParameterHandle::new(definition.enabled);
        node
    }
    pub(crate) fn id(&self) -> InstanceId {
        InstanceId(self.node_data().meta.uuid)
    }
    pub(crate) fn definition(&self) -> InstanceDefinition {
        let media = self.source_media.get_ref().uuid;
        let channel = self.source_channel.get_ref().uuid;
        InstanceDefinition {
            source: (!media.0.is_nil() && !channel.0.is_nil()).then_some(ChannelReference {
                media: MediaId(media),
                channel: ChannelId(channel),
            }),
            transform: Transform2D {
                position: components(self.position.get()),
                scale: components(self.scale.get()),
                rotation: self.rotation.get() as f32,
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
    /// Reads the complete proposed binding while validating a parameter batch.
    pub(crate) fn source_for_validation(&self, change: &ParameterValidationContext<'_>) -> Option<ChannelReference> {
        let ParamValue::Reference(media) = change.parameter_value(self.source_media.id())? else {
            return None;
        };
        let ParamValue::Reference(channel) = change.parameter_value(self.source_channel.id())? else {
            return None;
        };
        (!media.uuid.0.is_nil() && !channel.uuid.0.is_nil()).then_some(ChannelReference {
            media: MediaId(media.uuid),
            channel: ChannelId(channel.uuid),
        })
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn source_parameter_values(&self, source: ChannelReference) -> Vec<(NodeId, ParamValue)> {
        let (media, channel) = source_references(Some(source));
        vec![
            (self.source_media.id(), ParamValue::Reference(media)),
            (self.source_channel.id(), ParamValue::Reference(channel)),
        ]
    }
    pub(crate) fn parameter_edits(&self, definition: InstanceDefinition) -> Vec<golden_core::edit::Edit> {
        use golden_core::{edit::Edit, parameter::ParameterEventBehaviour};
        let (media, channel) = source_references(definition.source);
        let position = vector(definition.transform.position);
        let scale = vector(definition.transform.scale);
        let pivot = vector(definition.transform.pivot);
        [
            (self.source_media.id(), ParamValue::Reference(media)),
            (self.source_channel.id(), ParamValue::Reference(channel)),
            (self.position.id(), ParamValue::Vec2(position.x, position.y)),
            (self.scale.id(), ParamValue::Vec2(scale.x, scale.y)),
            (
                self.rotation.id(),
                ParamValue::Float(f64::from(definition.transform.rotation)),
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

#[node("mapgyver.composition.instance", from_struct)]
impl Node for InstanceNode {
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
        self.definition().validate()
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
        if change.parameter == self.rotation.id()
            && let ParamValue::Float(value) = change.value
            && !(*value as f32).is_finite()
        {
            return Err("Composition rotation must be finite".into());
        }
        if [self.source_media.id(), self.source_channel.id()].contains(&change.parameter)
            && let Some(source) = self.source_for_validation(change)
            && let Some(owner) = self.node_data().parent
        {
            crate::parameters::reference_cycle::validate(change, owner, source.channel.0)?;
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
fn source_references(source: Option<ChannelReference>) -> (NodeReference, NodeReference) {
    source.map_or_else(
        || (NodeReference::empty(), NodeReference::empty()),
        |source| (NodeReference::new(source.media.0), NodeReference::new(source.channel.0)),
    )
}
fn blend_name(blend: BlendMode) -> &'static str {
    match blend {
        BlendMode::Normal => "Normal",
        BlendMode::Add => "Add",
        BlendMode::Multiply => "Multiply",
        BlendMode::Screen => "Screen",
    }
}
