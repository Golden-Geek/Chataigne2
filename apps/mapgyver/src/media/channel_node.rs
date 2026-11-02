//! Variant settings are real Parameters; media-owned settings join them only in runtime projections.
use super::protocol::{ChannelDefinition, ChannelSource, SizePolicy};
use crate::matrix::protocol::{GroupId, LayerId, MatrixDefinition, MatrixOutput};
use golden_core::{
    edit::Edit,
    node,
    node::{
        Node, NodeId, NodeReference, ParameterHandle, ParameterValidationContext, UserContainerRules, UserCreatableItem,
    },
    parameter::{Color, Enum, ParamValue, ParameterEventBehaviour, ReferenceTargetKind},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};

use std::sync::OnceLock;

#[node("mapgyver.media.channel", label = "Channel", collapsed = true)]
#[children(
    folder(parameters, label = "Parameters") {
        source_kind: Enum = "SolidColor" (label = "Source kind", read_only = true, persist_read_only_value = true,
            enum_options = crate::parameters::enum_options(&["Picture", "SolidColor", "SharedTexture", "Composition", "Matrix"]));
        rgba: Color = Color::new(1.0, 1.0, 1.0, 1.0) [0.0..1.0] (label = "Color", description = "Straight linear-light RGBA.", constraint_policy = "reject");
        asset: NodeReference = NodeReference::empty() (label = "Picture asset", reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.media.asset".into()]);
        sender: String = String::new() (label = "Spout sender");
        output_kind: Enum = "Final" (label = "Matrix output", read_only = true, persist_read_only_value = true, enum_options = crate::parameters::enum_options(&["Final", "Layer", "Group"]));
        output_item: NodeReference = NodeReference::empty() (label = "Output item", read_only = true, persist_read_only_value = true, reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.matrix.layer".into(), "mapgyver.matrix.group".into()]);
    }
)]
pub struct ChannelNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
    #[state(default = OnceLock::new())]
    matrix_playback: OnceLock<crate::matrix::playback::PlaybackControl>,
}
impl ChannelNode {
    pub(crate) fn from_definition(label: String, definition: ChannelDefinition) -> Self {
        let mut node = Self::new();
        let presentation = node.node_data().meta.presentation.clone();
        node.node_data_mut().meta = golden_core::node::NodeMeta::new(label).with_presentation(presentation);
        macro_rules! seed {
            ($field:ident, $value:expr) => {
                node.$field = ParameterHandle::new($value);
            };
        }
        seed!(
            source_kind,
            Enum::new(super::media_node::kind_name(definition.media_kind()))
        );
        match definition.source {
            ChannelSource::SolidColor { rgba } => {
                seed!(
                    rgba,
                    Color::new(rgba[0].into(), rgba[1].into(), rgba[2].into(), rgba[3].into())
                );
            }
            ChannelSource::Picture { asset } => {
                seed!(
                    asset,
                    asset.map_or_else(NodeReference::empty, |id| NodeReference::new(
                        golden_core::node::NodeUuid(id.get())
                    ))
                );
            }
            ChannelSource::Composition { .. } => {}
            ChannelSource::Spout { sender } => {
                seed!(sender, sender.unwrap_or_default());
            }
            ChannelSource::Matrix { output } => match output {
                MatrixOutput::Final { .. } => {
                    let _ = node.matrix_playback.set(Default::default());
                }
                MatrixOutput::Layer { layer } => {
                    seed!(output_kind, Enum::new("Layer"));
                    seed!(output_item, NodeReference::new(layer.0));
                }
                MatrixOutput::Group { group } => {
                    seed!(output_kind, Enum::new("Group"));
                    seed!(output_item, NodeReference::new(group.0));
                }
            },
        }
        node
    }
    pub(crate) fn kind(&self) -> super::protocol::MediaKind {
        super::media_node::parse_kind(self.source_kind.get_ref().as_str())
    }
    pub(crate) fn is_matrix_final(&self) -> bool {
        self.kind() == super::protocol::MediaKind::Matrix && self.output_kind.get_ref().as_str() == "Final"
    }
    pub(crate) fn matrix_item(&self) -> Option<crate::matrix::protocol::MatrixItemId> {
        if self.kind() != super::protocol::MediaKind::Matrix {
            return None;
        }
        match self.output_kind.get_ref().as_str() {
            "Layer" => Some(crate::matrix::protocol::MatrixItemId::Layer {
                id: LayerId(self.output_item.get_ref().uuid),
            }),
            "Group" => Some(crate::matrix::protocol::MatrixItemId::Group {
                id: GroupId(self.output_item.get_ref().uuid),
            }),
            _ => None,
        }
    }
    pub(crate) fn definition(&self, media: &super::MediaNode, engine: &crate::app::AppEngine) -> ChannelDefinition {
        let size = media.size_policy(engine);
        let canvas = match size {
            SizePolicy::Automatic => super::resolution::default_canvas(),
            SizePolicy::Fixed { extent } => extent,
        };
        let settings = media.matrix_settings();
        let source = match self.source_kind.get_ref().as_str() {
            "Picture" => ChannelSource::Picture {
                asset: (!self.asset.get_ref().uuid.0.is_nil())
                    .then(|| golden_assets::AssetId::from_uuid(self.asset.get_ref().uuid.0)),
            },
            "Composition" => ChannelSource::Composition { canvas },
            "SharedTexture" => ChannelSource::Spout {
                sender: (!self.sender.get_ref().is_empty()).then(|| self.sender.get_ref().clone()),
            },
            "Matrix" => ChannelSource::Matrix {
                output: match self.output_kind.get_ref().as_str() {
                    "Layer" => MatrixOutput::Layer {
                        layer: LayerId(self.output_item.get_ref().uuid),
                    },
                    "Group" => MatrixOutput::Group {
                        group: GroupId(self.output_item.get_ref().uuid),
                    },
                    _ => MatrixOutput::Final {
                        definition: MatrixDefinition {
                            canvas,
                            empty_cell: settings.empty_cell,
                            transition: settings.transition,
                        },
                    },
                },
            },
            _ => {
                let c = self.rgba.get();
                ChannelSource::SolidColor {
                    rgba: [c.red as f32, c.green as f32, c.blue as f32, c.alpha as f32],
                }
            }
        };
        ChannelDefinition {
            source,
            size,
            filter: media.sampling(),
        }
    }
    pub(crate) fn parameter_edits(&self, source: ChannelSource) -> Result<Vec<Edit>, String> {
        let mut values = Vec::new();
        match source {
            ChannelSource::SolidColor { rgba } => values.push((
                self.rgba.id(),
                ParamValue::Color(rgba[0].into(), rgba[1].into(), rgba[2].into(), rgba[3].into()),
            )),
            ChannelSource::Picture { asset } => values.push((
                self.asset.id(),
                ParamValue::Reference(asset.map_or_else(NodeReference::empty, |id| {
                    NodeReference::new(golden_core::node::NodeUuid(id.get()))
                })),
            )),
            ChannelSource::Composition { .. } | ChannelSource::Matrix { .. } => {
                return Err("derived outputs have no authored channel settings".into());
            }
            ChannelSource::Spout { sender } => {
                values.push((self.sender.id(), ParamValue::Str(sender.unwrap_or_default())))
            }
        }
        Ok(values
            .into_iter()
            .map(|(node, value)| Edit::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            })
            .collect())
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn matrix_playback(&self) -> Option<&crate::matrix::playback::PlaybackControl> {
        (self.source_kind.get_ref().as_str() == "Matrix" && self.output_kind.get_ref().as_str() == "Final")
            .then(|| self.matrix_playback.get_or_init(Default::default))
    }
    fn invalidate(&self) {
        self.revision.advance();
        if let Some(control) = self.matrix_playback.get() {
            control.invalidate();
            control.invalidate_applications();
        }
    }
}
#[node("mapgyver.media.channel", from_struct)]
impl Node for ChannelNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn on_structure_changed(&mut self, ctx: &mut ProcessCtx) {
        use golden_core::events::EventKind;
        // Params DSL observes two child levels to bind its own folder. That must
        // not broaden logical application membership to descendant Matrix cells.
        let owner = self.id();
        if ctx.events.iter().any(|event| match event.kind {
            EventKind::ChildAdded { parent, .. }
            | EventKind::ChildRemoved { parent, .. }
            | EventKind::ChildReplaced { parent, .. }
            | EventKind::ChildReordered { parent, .. } => parent == owner,
            EventKind::ChildMoved {
                old_parent, new_parent, ..
            } => old_parent == owner || new_parent == owner,
            EventKind::NodeCreated { node } | EventKind::NodeDeleted { node } => node == owner,
            _ => false,
        }) {
            self.invalidate();
        }
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        if [
            self.source_kind.id(),
            self.rgba.id(),
            self.asset.id(),
            self.sender.id(),
            self.output_kind.id(),
            self.output_item.id(),
        ]
        .contains(&parameter)
        {
            self.revision.advance();
            if let Some(control) = self.matrix_playback.get() {
                control.invalidate();
            }
        }
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.invalidate();
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        if let Some(control) = self.matrix_playback.get() {
            control.clear();
        }
        self.revision.advance();
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            crate::matrix::node::LAYER_TYPE,
            crate::matrix::node::GROUP_TYPE,
            crate::matrix::node::COLUMN_TYPE,
            super::composition::INSTANCE_TYPE,
        ]))
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        if self.kind() == super::protocol::MediaKind::Composition {
            vec![UserCreatableItem::new(
                super::composition::INSTANCE_TYPE,
                super::composition::INSTANCE_TYPE,
                "Instance",
            )]
        } else {
            Vec::new()
        }
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if self.is_matrix_final() {
            crate::matrix::node::create(node_type).map(|node| Box::new(node) as Box<dyn Node>)
        } else if self.kind() == super::protocol::MediaKind::Composition
            && node_type == super::composition::INSTANCE_TYPE
        {
            Some(Box::new(super::composition::InstanceNode::from_definition(
                "Instance".into(),
                Default::default(),
            )))
        } else {
            None
        }
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        for (value, allowed) in [
            (
                self.source_kind.get_ref().as_str(),
                &["Picture", "SolidColor", "SharedTexture", "Composition", "Matrix"][..],
            ),
            (self.output_kind.get_ref().as_str(), &["Final", "Layer", "Group"][..]),
        ] {
            if !allowed.contains(&value) {
                return Err(format!("unknown channel setting '{value}'"));
            }
        }
        if self.kind() == super::protocol::MediaKind::SolidColor {
            let color = self.rgba.get();
            golden_video::LinearColor::from_straight(
                color.red as f32,
                color.green as f32,
                color.blue as f32,
                color.alpha as f32,
            )
            .map_err(|error| error.to_string())?;
        }
        if self.sender.get_ref().len() > 255 || self.sender.get_ref().contains('\0') {
            return Err("Spout selector must contain at most 255 UTF-8 bytes without NUL".into());
        }
        Ok(())
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        for (id, current) in [
            (
                self.source_kind.id(),
                ParamValue::Enum(self.source_kind.get_ref().0.clone()),
            ),
            (
                self.output_kind.id(),
                ParamValue::Enum(self.output_kind.get_ref().0.clone()),
            ),
            (
                self.output_item.id(),
                ParamValue::Reference(self.output_item.get_ref().clone()),
            ),
        ] {
            if change.parameter == id && change.value != &current {
                return Err("Channel source/output identity is owned by structural operations".into());
            }
        }
        if change.parameter == self.sender.id()
            && let ParamValue::Str(value) = change.value
            && (value.len() > 255 || value.contains('\0'))
        {
            return Err("Spout selector must contain at most 255 UTF-8 bytes without NUL".into());
        }
        Ok(())
    }
}
