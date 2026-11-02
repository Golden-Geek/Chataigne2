//! All channel source/output settings are real Parameters. DTOs project their values.
use super::protocol::{ChannelDefinition, ChannelSource, SizePolicy};
use crate::matrix::protocol::{
    EmptyCellPolicy, GroupId, LayerId, MatrixDefinition, MatrixOutput, TransitionCurve, TransitionDefinition,
};
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
use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, plan::SampleFilter};
use std::sync::OnceLock;

#[node("mapgyver.media.channel", label = "Channel", collapsed = true)]
#[children(
    folder(parameters, label = "Parameters") {
        source_kind: Enum = "SolidColor" (label = "Source kind", read_only = true, persist_read_only_value = true,
            enum_options = crate::parameters::enum_options(&["Picture", "SolidColor", "SharedTexture", "Composition", "Matrix"]));
        filter: Enum = "Linear" (label = "Sampling", enum_options = crate::parameters::enum_options(&["Linear", "Nearest"]));
        rgba: Color = Color::new(1.0, 1.0, 1.0, 1.0) [0.0..1.0] (label = "Color", description = "Straight linear-light RGBA.", constraint_policy = "reject");
        asset: NodeReference = NodeReference::empty() (label = "Picture asset", reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.media.asset".into()]);
        sender: String = String::new() (label = "Spout sender");
        maximum_width: i32 = 1920 [1..16384] (label = "Maximum width", constraint_policy = "reject");
        maximum_height: i32 = 1080 [1..16384] (label = "Maximum height", constraint_policy = "reject");
        pixel_format: Enum = "Bgra8Unorm" (label = "Pixel format", enum_options = crate::parameters::enum_options(&["Bgra8Unorm", "Rgba8Unorm"]));
        color_encoding: Enum = "Srgb" (label = "Color encoding", enum_options = crate::parameters::enum_options(&["Srgb", "Linear"]));
        alpha_mode: Enum = "Straight" (label = "Alpha mode", enum_options = crate::parameters::enum_options(&["Straight", "Premultiplied", "Opaque"]));
        canvas_width: i32 = 1920 [1..16384] (label = "Canvas width", constraint_policy = "reject");
        canvas_height: i32 = 1080 [1..16384] (label = "Canvas height", constraint_policy = "reject");
        output_kind: Enum = "Final" (label = "Matrix output", read_only = true, persist_read_only_value = true, enum_options = crate::parameters::enum_options(&["Final", "Layer", "Group"]));
        output_item: NodeReference = NodeReference::empty() (label = "Output item", read_only = true, persist_read_only_value = true, reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.matrix.layer".into(), "mapgyver.matrix.group".into()]);
        empty_cell: Enum = "Transparent" (label = "Empty cell", enum_options = crate::parameters::enum_options(&["Transparent"]));
        transition_ms: f64 = 250.0 [0.0..4294967295.0] (label = "Transition duration", step = 1.0, description = "Milliseconds.", constraint_policy = "reject");
        transition_curve: Enum = "Linear" (label = "Transition curve", enum_options = crate::parameters::enum_options(&["Linear", "SmoothStep"]));
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
        seed!(
            filter,
            Enum::new(match definition.filter {
                SampleFilter::Linear => "Linear",
                SampleFilter::Nearest => "Nearest",
            })
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
            ChannelSource::Composition { canvas } => {
                seed!(canvas_width, canvas.width() as i32);
                seed!(canvas_height, canvas.height() as i32);
            }
            ChannelSource::Spout { sender, maximum } => {
                seed!(sender, sender.unwrap_or_default());
                seed!(maximum_width, maximum.extent.width() as i32);
                seed!(maximum_height, maximum.extent.height() as i32);
                seed!(
                    pixel_format,
                    Enum::new(match maximum.format {
                        PixelFormat::Bgra8Unorm => "Bgra8Unorm",
                        PixelFormat::Rgba8Unorm => "Rgba8Unorm",
                    })
                );
                seed!(
                    color_encoding,
                    Enum::new(match maximum.color_encoding {
                        ColorEncoding::Linear => "Linear",
                        ColorEncoding::Srgb => "Srgb",
                    })
                );
                seed!(
                    alpha_mode,
                    Enum::new(match maximum.alpha_mode {
                        AlphaMode::Straight => "Straight",
                        AlphaMode::Premultiplied => "Premultiplied",
                        AlphaMode::Opaque => "Opaque",
                    })
                );
            }
            ChannelSource::Matrix { output } => match output {
                MatrixOutput::Final { definition } => {
                    seed!(canvas_width, definition.canvas.width() as i32);
                    seed!(canvas_height, definition.canvas.height() as i32);
                    seed!(transition_ms, f64::from(definition.transition.duration_ms));
                    seed!(transition_curve, Enum::new(curve_name(definition.transition.curve)));
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
    pub(crate) fn definition(&self) -> ChannelDefinition {
        let source = match self.source_kind.get_ref().as_str() {
            "Picture" => ChannelSource::Picture {
                asset: (!self.asset.get_ref().uuid.0.is_nil())
                    .then(|| golden_assets::AssetId::from_uuid(self.asset.get_ref().uuid.0)),
            },
            "Composition" => ChannelSource::Composition {
                canvas: extent(self.canvas_width.get(), self.canvas_height.get()),
            },
            "SharedTexture" => ChannelSource::Spout {
                sender: (!self.sender.get_ref().is_empty()).then(|| self.sender.get_ref().clone()),
                maximum: TextureDescriptor {
                    extent: extent(self.maximum_width.get(), self.maximum_height.get()),
                    format: if self.pixel_format.get_ref().as_str() == "Rgba8Unorm" {
                        PixelFormat::Rgba8Unorm
                    } else {
                        PixelFormat::Bgra8Unorm
                    },
                    color_encoding: if self.color_encoding.get_ref().as_str() == "Linear" {
                        ColorEncoding::Linear
                    } else {
                        ColorEncoding::Srgb
                    },
                    alpha_mode: match self.alpha_mode.get_ref().as_str() {
                        "Opaque" => AlphaMode::Opaque,
                        "Premultiplied" => AlphaMode::Premultiplied,
                        _ => AlphaMode::Straight,
                    },
                },
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
                            canvas: extent(self.canvas_width.get(), self.canvas_height.get()),
                            empty_cell: EmptyCellPolicy::Transparent,
                            transition: TransitionDefinition {
                                duration_ms: self.transition_ms.get() as u32,
                                curve: if self.transition_curve.get_ref().as_str() == "SmoothStep" {
                                    TransitionCurve::SmoothStep
                                } else {
                                    TransitionCurve::Linear
                                },
                            },
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
            size: SizePolicy::Automatic,
            filter: if self.filter.get_ref().as_str() == "Nearest" {
                SampleFilter::Nearest
            } else {
                SampleFilter::Linear
            },
        }
    }
    pub(crate) fn parameter_edits(&self, definition: ChannelDefinition) -> Vec<Edit> {
        let mut values = vec![(
            self.filter.id(),
            ParamValue::Enum(
                match definition.filter {
                    SampleFilter::Linear => "Linear",
                    SampleFilter::Nearest => "Nearest",
                }
                .into(),
            ),
        )];
        match definition.source {
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
            ChannelSource::Composition { canvas } => values.extend([
                (self.canvas_width.id(), ParamValue::Int(canvas.width() as i32)),
                (self.canvas_height.id(), ParamValue::Int(canvas.height() as i32)),
            ]),
            ChannelSource::Spout { sender, maximum } => values.extend([
                (self.sender.id(), ParamValue::Str(sender.unwrap_or_default())),
                (self.maximum_width.id(), ParamValue::Int(maximum.extent.width() as i32)),
                (
                    self.maximum_height.id(),
                    ParamValue::Int(maximum.extent.height() as i32),
                ),
                (
                    self.pixel_format.id(),
                    ParamValue::Enum(
                        match maximum.format {
                            PixelFormat::Bgra8Unorm => "Bgra8Unorm",
                            PixelFormat::Rgba8Unorm => "Rgba8Unorm",
                        }
                        .into(),
                    ),
                ),
                (
                    self.color_encoding.id(),
                    ParamValue::Enum(
                        match maximum.color_encoding {
                            ColorEncoding::Linear => "Linear",
                            ColorEncoding::Srgb => "Srgb",
                        }
                        .into(),
                    ),
                ),
                (
                    self.alpha_mode.id(),
                    ParamValue::Enum(
                        match maximum.alpha_mode {
                            AlphaMode::Straight => "Straight",
                            AlphaMode::Premultiplied => "Premultiplied",
                            AlphaMode::Opaque => "Opaque",
                        }
                        .into(),
                    ),
                ),
            ]),
            ChannelSource::Matrix {
                output: MatrixOutput::Final { definition },
            } => values.extend([
                (
                    self.canvas_width.id(),
                    ParamValue::Int(definition.canvas.width() as i32),
                ),
                (
                    self.canvas_height.id(),
                    ParamValue::Int(definition.canvas.height() as i32),
                ),
                (
                    self.transition_ms.id(),
                    ParamValue::Float(definition.transition.duration_ms.into()),
                ),
                (
                    self.transition_curve.id(),
                    ParamValue::Enum(curve_name(definition.transition.curve).into()),
                ),
            ]),
            ChannelSource::Matrix { .. } => {}
        }
        values
            .into_iter()
            .map(|(node, value)| Edit::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            })
            .collect()
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
            self.filter.id(),
            self.rgba.id(),
            self.asset.id(),
            self.sender.id(),
            self.maximum_width.id(),
            self.maximum_height.id(),
            self.pixel_format.id(),
            self.color_encoding.id(),
            self.alpha_mode.id(),
            self.canvas_width.id(),
            self.canvas_height.id(),
            self.output_kind.id(),
            self.output_item.id(),
            self.empty_cell.id(),
            self.transition_ms.id(),
            self.transition_curve.id(),
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
        if matches!(self.definition().source, ChannelSource::Composition { .. }) {
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
        match self.definition().source {
            ChannelSource::Matrix {
                output: MatrixOutput::Final { .. },
            } => crate::matrix::node::create(node_type).map(|node| Box::new(node) as Box<dyn Node>),
            ChannelSource::Composition { .. } if node_type == super::composition::INSTANCE_TYPE => Some(Box::new(
                super::composition::InstanceNode::from_definition("Instance".into(), Default::default()),
            )),
            _ => None,
        }
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        for value in [
            self.maximum_width.get(),
            self.maximum_height.get(),
            self.canvas_width.get(),
            self.canvas_height.get(),
        ] {
            if !(1..=16384).contains(&value) {
                return Err("channel dimensions must contain 1..16384 texels".into());
            }
        }
        for (value, allowed) in [
            (
                self.source_kind.get_ref().as_str(),
                &["Picture", "SolidColor", "SharedTexture", "Composition", "Matrix"][..],
            ),
            (self.filter.get_ref().as_str(), &["Linear", "Nearest"][..]),
            (self.output_kind.get_ref().as_str(), &["Final", "Layer", "Group"][..]),
            (self.pixel_format.get_ref().as_str(), &["Bgra8Unorm", "Rgba8Unorm"][..]),
            (self.color_encoding.get_ref().as_str(), &["Srgb", "Linear"][..]),
            (
                self.alpha_mode.get_ref().as_str(),
                &["Straight", "Premultiplied", "Opaque"][..],
            ),
            (self.empty_cell.get_ref().as_str(), &["Transparent"][..]),
            (self.transition_curve.get_ref().as_str(), &["Linear", "SmoothStep"][..]),
        ] {
            if !allowed.contains(&value) {
                return Err(format!("unknown channel setting '{value}'"));
            }
        }
        if !self.transition_ms.get().is_finite()
            || !(0.0..=f64::from(u32::MAX)).contains(&self.transition_ms.get())
            || self.transition_ms.get().fract() != 0.0
        {
            return Err("transition duration must be a whole number of milliseconds in the u32 range".into());
        }
        self.definition().validate()
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
fn curve_name(curve: TransitionCurve) -> &'static str {
    match curve {
        TransitionCurve::Linear => "Linear",
        TransitionCurve::SmoothStep => "SmoothStep",
    }
}
fn extent(width: i32, height: i32) -> Extent2D {
    // Detached malformed records can be inspected during lifecycle binding before
    // candidate validation rejects them. Never panic while projecting that candidate.
    Extent2D::new(width as u32, height as u32).unwrap_or_else(|_| super::resolution::default_canvas())
}
