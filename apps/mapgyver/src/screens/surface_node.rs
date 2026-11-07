//! Surface references and mapping properties are real typed Golden Parameters.
use super::protocol::{
    Calibration, Point, Quad, SurfaceDefinition, SurfaceGeometryEdit, SurfaceId, SurfaceParameterBindings, UnitRect,
};
use crate::media::protocol::{ChannelId, ChannelReference, MediaId};
use golden_core::{
    node,
    node::{Node, NodeId, NodeMeta, NodeReference, ParameterHandle, ParameterValidationContext, ParameterValueType},
    parameter::{Enum, ParamValue, ReferenceTargetKind, Vec2},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};

#[node(
    "mapgyver.surface",
    label = "Surface",
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true,
        can_remove_and_duplicate: true,
        ..Default::default()
    }
)]
#[children(
    folder(parameters, label = "Parameters") {
        source_media: NodeReference = NodeReference::empty() (label = "Source media", reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.media.v8".into(), "mapgyver.media.reference.v4".into()]);
        source_channel: NodeReference = NodeReference::empty() (label = "Source channel", reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.media.channel.v2".into(), crate::media::reference::OUTPUT_TYPE.into()]);
        crop_origin: Vec2 = Vec2::new(0.0, 0.0) [(0.0, 0.0)..(1.0, 1.0)] (label = "Crop origin");
        crop_size: Vec2 = Vec2::new(1.0, 1.0) [(0.0, 0.0)..(1.0, 1.0)] (label = "Crop size");
        top_left: Vec2 = Vec2::new(0.0, 0.0) (label = "Top left");
        top_right: Vec2 = Vec2::new(1.0, 0.0) (label = "Top right");
        bottom_right: Vec2 = Vec2::new(1.0, 1.0) (label = "Bottom right");
        bottom_left: Vec2 = Vec2::new(0.0, 1.0) (label = "Bottom left");
        opacity: f64 = 1.0 [0.0..1.0] (label = "Opacity", constraint_policy = "reject");
        calibration: Enum = "Off" (label = "Calibration", enum_options = crate::parameters::enum_options(&["Off", "Grid", "ColorBars"]));
    }
)]
pub struct SurfaceNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
}

impl SurfaceNode {
    pub(super) fn parameter_bindings(
        &self,
        engine: &crate::app::AppEngine,
        surface: SurfaceId,
    ) -> SurfaceParameterBindings {
        let uuid = |id| {
            engine
                .nodes
                .get(id)
                .expect("materialized Surface Parameter")
                .node_data()
                .meta
                .uuid
        };
        SurfaceParameterBindings {
            surface,
            crop_origin: uuid(self.crop_origin.id()),
            crop_size: uuid(self.crop_size.id()),
            corners: [
                self.top_left.id(),
                self.top_right.id(),
                self.bottom_right.id(),
                self.bottom_left.id(),
            ]
            .map(uuid),
            opacity: uuid(self.opacity.id()),
        }
    }

    pub(super) fn quad_parameter_values(&self, quad: Quad) -> [(NodeId, ParamValue); 4] {
        let ids = [
            self.top_left.id(),
            self.top_right.id(),
            self.bottom_right.id(),
            self.bottom_left.id(),
        ];
        std::array::from_fn(|index| (ids[index], vector_value(point_vector(quad.corners[index]))))
    }

    /// The candidate and corner index have already passed geometry validation.
    pub(super) fn geometry_parameter_values(
        &self,
        definition: &SurfaceDefinition,
        edit: SurfaceGeometryEdit,
    ) -> Vec<(NodeId, ParamValue)> {
        match edit {
            SurfaceGeometryEdit::QuadTranslation { .. } => self.quad_parameter_values(definition.quad).into(),
            SurfaceGeometryEdit::QuadCorner { corner, .. } | SurfaceGeometryEdit::QuadCornerPixels { corner, .. } => {
                vec![self.quad_parameter_values(definition.quad)[usize::from(corner)].clone()]
            }
            SurfaceGeometryEdit::CropOrigin { .. } => vec![(
                self.crop_origin.id(),
                vector_value(vector(definition.crop.x, definition.crop.y)),
            )],
            SurfaceGeometryEdit::CropCorner { .. } => vec![
                (
                    self.crop_origin.id(),
                    vector_value(vector(definition.crop.x, definition.crop.y)),
                ),
                (
                    self.crop_size.id(),
                    vector_value(vector(definition.crop.width, definition.crop.height)),
                ),
            ],
        }
    }
    pub(crate) fn from_definition(label: String, definition: SurfaceDefinition) -> Self {
        let mut node = Self::new();
        let permissions = node.node_data().meta.user_permissions.clone();
        node.node_data_mut().meta = NodeMeta::new(label).with_user_permissions(permissions);
        node.node_data_mut().meta.enabled = definition.enabled;
        let (media, channel) = source_references(definition.source);
        node.source_media = ParameterHandle::new(media);
        node.source_channel = ParameterHandle::new(channel);
        node.crop_origin = ParameterHandle::new(vector(definition.crop.x, definition.crop.y));
        node.crop_size = ParameterHandle::new(vector(definition.crop.width, definition.crop.height));
        node.top_left = ParameterHandle::new(point_vector(definition.quad.corners[0]));
        node.top_right = ParameterHandle::new(point_vector(definition.quad.corners[1]));
        node.bottom_right = ParameterHandle::new(point_vector(definition.quad.corners[2]));
        node.bottom_left = ParameterHandle::new(point_vector(definition.quad.corners[3]));
        node.opacity = ParameterHandle::new(f64::from(definition.opacity));
        node.calibration = ParameterHandle::new(Enum::new(calibration_name(definition.calibration)));
        node
    }

    pub fn definition(&self) -> SurfaceDefinition {
        self.values(None).definition()
    }

    pub(crate) fn parameter_values(&self, definition: SurfaceDefinition) -> Vec<(NodeId, ParamValue)> {
        let mut values = Vec::from(self.source_parameter_values(definition.source));
        values.extend([
            (
                self.crop_origin.id(),
                vector_value(vector(definition.crop.x, definition.crop.y)),
            ),
            (
                self.crop_size.id(),
                vector_value(vector(definition.crop.width, definition.crop.height)),
            ),
            (
                self.top_left.id(),
                vector_value(point_vector(definition.quad.corners[0])),
            ),
            (
                self.top_right.id(),
                vector_value(point_vector(definition.quad.corners[1])),
            ),
            (
                self.bottom_right.id(),
                vector_value(point_vector(definition.quad.corners[2])),
            ),
            (
                self.bottom_left.id(),
                vector_value(point_vector(definition.quad.corners[3])),
            ),
            (self.opacity.id(), ParamValue::Float(f64::from(definition.opacity))),
            (
                self.calibration.id(),
                ParamValue::Enum(calibration_name(definition.calibration).into()),
            ),
        ]);
        values
    }

    pub(crate) fn source_parameter_values(&self, source: Option<ChannelReference>) -> [(NodeId, ParamValue); 2] {
        let (media, channel) = source_references(source);
        [
            (self.source_media.id(), ParamValue::Reference(media)),
            (self.source_channel.id(), ParamValue::Reference(channel)),
        ]
    }

    pub fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }

    fn parameter_ids(&self) -> [NodeId; 10] {
        [
            self.source_media.id(),
            self.source_channel.id(),
            self.crop_origin.id(),
            self.crop_size.id(),
            self.top_left.id(),
            self.top_right.id(),
            self.bottom_right.id(),
            self.bottom_left.id(),
            self.opacity.id(),
            self.calibration.id(),
        ]
    }

    fn values(&self, change: Option<&ParameterValidationContext<'_>>) -> SurfaceValues {
        SurfaceValues {
            source_media: read(&self.source_media, change),
            source_channel: read(&self.source_channel, change),
            crop_origin: read(&self.crop_origin, change),
            crop_size: read(&self.crop_size, change),
            corners: [
                read(&self.top_left, change),
                read(&self.top_right, change),
                read(&self.bottom_right, change),
                read(&self.bottom_left, change),
            ],
            opacity: read(&self.opacity, change),
            calibration: read(&self.calibration, change),
            enabled: self.node_data().meta.enabled,
        }
    }
}

#[node("mapgyver.surface", from_struct)]
impl Node for SurfaceNode {
    fn needs_update(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }

    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }

    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }

    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.revision.advance();
    }

    fn on_param_change(&mut self, _: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        if self.parameter_ids().contains(&parameter) {
            self.revision.advance();
        }
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }

    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        self.values(None).validate()
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if self.parameter_ids().contains(&change.parameter) {
            self.values(Some(change)).validate()?;
        }
        if change.parameter == self.source_media.id() || change.parameter == self.source_channel.id() {
            let values = self.values(Some(change));
            if values.source_media.uuid.0.is_nil() && !values.source_channel.uuid.0.is_nil() {
                return Err("Surface Channel requires a Media target".into());
            }
            if !values.source_channel.uuid.0.is_nil() {
                crate::media::targeting::validate_output_membership(
                    change,
                    values.source_media.uuid,
                    values.source_channel.uuid,
                )?;
            }
        }
        Ok(())
    }

    fn parameter_companion_changes(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Vec<(NodeId, ParamValue)>, String> {
        if change.parameter != self.source_media.id() || change.is_proposed(self.source_channel.id()) {
            return Ok(Vec::new());
        }
        let ParamValue::Reference(media) = change.value else {
            return Err("Surface Media target must be a Reference".into());
        };
        if media.uuid == self.source_media.get_ref().uuid {
            return Ok(Vec::new());
        }
        let channel = if media.uuid.0.is_nil() {
            None
        } else {
            crate::media::targeting::first_output(change, media.uuid)?
        };
        Ok(vec![(
            self.source_channel.id(),
            ParamValue::Reference(channel.map_or_else(NodeReference::empty, NodeReference::new)),
        )])
    }

    fn normalize_parameter_change(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Option<ParamValue>, String> {
        let normalize: fn(f64, f64) -> f64;
        let sibling;
        if change.parameter == self.crop_origin.id() {
            normalize = crop_origin;
            sibling = read(&self.crop_size, Some(change));
        } else if change.parameter == self.crop_size.id() {
            normalize = crop_size;
            sibling = read(&self.crop_origin, Some(change));
        } else {
            return Ok(None);
        }
        let ParamValue::Vec2(x, y) = change.value else {
            return Ok(None);
        };
        if !x.is_finite() || !y.is_finite() || !finite_point(sibling) {
            return Err("surface crop positions must be finite".into());
        }
        let value = ParamValue::Vec2(normalize(*x, sibling.x), normalize(*y, sibling.y));
        Ok((value != *change.value).then_some(value))
    }
}

fn crop_origin(value: f64, size: f64) -> f64 {
    let projected = value as f32;
    if projected < 1.0 && projected + size as f32 <= 1.0 {
        value
    } else {
        f64::from(bounded_crop_origin(projected, size as f32))
    }
}

pub(super) fn bounded_crop_origin(value: f32, size: f32) -> f32 {
    // Preserve size when translating. Keep room for a positive extent even when a
    // subnormal size rounds away in the subtraction at the source's far edge.
    value.clamp(0.0, (1.0 - size).min(1.0_f32.next_down()))
}

fn crop_size(value: f64, origin: f64) -> f64 {
    if value as f32 > 0.0 && origin as f32 + value as f32 <= 1.0 {
        return value;
    }
    let maximum = (1.0 - origin as f32).max(f32::MIN_POSITIVE);
    let value = value as f32;
    // Zero is a reachable slider endpoint, but a sampled crop must have positive
    // extent. Existing positive precision is retained rather than quantized.
    f64::from(if value <= 0.0 {
        f32::EPSILON.min(maximum)
    } else {
        value.min(maximum)
    })
}

struct SurfaceValues {
    source_media: NodeReference,
    source_channel: NodeReference,
    crop_origin: Vec2,
    crop_size: Vec2,
    corners: [Vec2; 4],
    opacity: f64,
    calibration: Enum,
    enabled: bool,
}

impl SurfaceValues {
    fn definition(&self) -> SurfaceDefinition {
        let media = self.source_media.uuid;
        let channel = self.source_channel.uuid;
        SurfaceDefinition {
            source: (!media.0.is_nil() && !channel.0.is_nil()).then_some(ChannelReference {
                media: MediaId(media),
                channel: ChannelId(channel),
            }),
            crop: UnitRect {
                x: self.crop_origin.x as f32,
                y: self.crop_origin.y as f32,
                width: self.crop_size.x as f32,
                height: self.crop_size.y as f32,
            },
            quad: Quad {
                corners: self.corners.map(point),
            },
            opacity: self.opacity as f32,
            calibration: match self.calibration.as_str() {
                "Grid" => Calibration::Grid,
                "ColorBars" => Calibration::ColorBars,
                _ => Calibration::Off,
            },
            enabled: self.enabled,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if !["Off", "Grid", "ColorBars"].contains(&self.calibration.as_str()) {
            return Err("unknown Surface calibration mode".into());
        }
        if [self.crop_origin, self.crop_size]
            .into_iter()
            .chain(self.corners)
            .any(|value| !finite_point(value))
        {
            return Err("surface crop and corner positions must be finite".into());
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err("surface opacity must be finite and in 0..1".into());
        }
        self.definition().validate()
    }
}

fn read<T: ParameterValueType + PartialEq>(
    handle: &ParameterHandle<T>,
    change: Option<&ParameterValidationContext<'_>>,
) -> T {
    change
        .and_then(|change| change.parameter_value(handle.id()))
        .and_then(|value| T::from_param_value(&value))
        .unwrap_or_else(|| handle.get_ref().clone())
}

fn finite_point(value: Vec2) -> bool {
    (value.x as f32).is_finite() && (value.y as f32).is_finite()
}

fn vector(x: f32, y: f32) -> Vec2 {
    Vec2::new(f64::from(x), f64::from(y))
}

fn point_vector(value: Point) -> Vec2 {
    vector(value.x, value.y)
}

fn point(value: Vec2) -> Point {
    Point {
        x: value.x as f32,
        y: value.y as f32,
    }
}

fn vector_value(value: Vec2) -> ParamValue {
    ParamValue::Vec2(value.x, value.y)
}

fn source_references(source: Option<ChannelReference>) -> (NodeReference, NodeReference) {
    source.map_or_else(
        || (NodeReference::empty(), NodeReference::empty()),
        |source| (NodeReference::new(source.media.0), NodeReference::new(source.channel.0)),
    )
}

fn calibration_name(value: Calibration) -> &'static str {
    match value {
        Calibration::Off => "Off",
        Calibration::Grid => "Grid",
        Calibration::ColorBars => "ColorBars",
    }
}
