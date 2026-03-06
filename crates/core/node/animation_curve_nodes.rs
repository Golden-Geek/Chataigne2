use std::collections::{HashMap, HashSet};

use crate::animation_curve::{AnimationCurve, AnimationCurveKey, CurveEasing, CurveHandle, CurvePhaseMode, CurveShape, CurveStepMode};
use crate::edit::Edit;
use crate::events::{Event, EventKind};
use crate::node::NodeId;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, ParameterEventBehaviour, RangeConstraint};
use crate::process_ctx::{ProcessCtx, ProcessTreeSnapshot};

use super::{
    DeclId, EventPropagation, Node, NodeData, PARAMETER_ANIMATION_CURVE_DECL_ID, PARAMETER_ANIMATION_CURVE_ITEM_KIND, PARAMETER_ANIMATION_CURVE_NODE_TYPE, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, PARAMETER_ANIMATION_EASING_DECL_ID, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, PARAMETER_ANIMATION_EASING_KIND_DECL_ID, PARAMETER_ANIMATION_EASING_NODE_TYPE,
    PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID, PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID, PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID, PARAMETER_ANIMATION_EASING_PHASE_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID, PARAMETER_ANIMATION_EASING_SEED_DECL_ID, PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID, PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID,
    PARAMETER_ANIMATION_KEY_ITEM_KIND, PARAMETER_ANIMATION_KEY_NODE_TYPE, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, PARAMETER_ANIMATION_RANGE_DECL_ID, PARAMETER_ANIMATION_RANGE_NODE_TYPE, PARAMETER_ANIMATION_RANGE_X_DECL_ID,
    PARAMETER_ANIMATION_RANGE_Y_DECL_ID, UserContainerRules, UserCreatableItem, parameter_child_exists,
};

const CURVE_RANGE_EPSILON: f64 = 1e-9;
const KEY_ORDER_POSITION_EPSILON: f64 = 1e-10;
const LEGACY_COORDINATE_SPACE_DECL_ID: &str = "coordinate_space";

fn make_float_parameter(label: &str, decl_id: &str, default_value: f64) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Float(default_value), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

fn make_non_negative_float_parameter(label: &str, decl_id: &str, default_value: f64) -> Parameter {
    let mut parameter = make_float_parameter(label, decl_id, default_value);
    parameter.constraints.range = Some(RangeConstraint::Uniform { min: Some(0.0), max: None });
    parameter
}

fn make_int_parameter(label: &str, decl_id: &str, default_value: i32) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Int(default_value), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

fn make_string_parameter(label: &str, decl_id: &str, default_value: &str) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Str(default_value.to_string()), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

fn make_vec2_parameter(label: &str, decl_id: &str, x: f64, y: f64) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Vec2(x, y), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

fn make_enum_parameter(label: &str, decl_id: &str, default_variant: &str, variants: &[(&str, &str)]) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Enum(default_variant.to_string()), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter.constraints.enum_options = variants
        .iter()
        .enumerate()
        .map(|(index, (variant_id, variant_label))| ParameterEnumOption {
            variant_id: (*variant_id).to_string(),
            value: ParamValue::Enum((*variant_id).to_string()),
            label: (*variant_label).to_string(),
            tags: Vec::new(),
            ordering: Some(index as i32),
        })
        .collect();
    parameter
}

fn make_easing_kind_parameter() -> Parameter {
    make_enum_parameter(
        "Kind",
        PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
        "bezier",
        &[("linear", "Linear"), ("bezier", "Bezier"), ("hold", "Hold"), ("steps", "Steps"), ("shape", "Shape"), ("perlinNoise", "Perlin Noise"), ("random", "Random"), ("script", "Script")],
    )
}

fn make_step_mode_parameter() -> Parameter {
    make_enum_parameter("Step Mode", PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID, "numSteps", &[("stepSize", "Step Size"), ("numSteps", "Number of Steps")])
}

fn make_shape_parameter() -> Parameter {
    make_enum_parameter("Shape", PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID, "sine", &[("sine", "Sine"), ("triangle", "Triangle"), ("saw", "Saw"), ("reverseSaw", "Reverse Saw"), ("square", "Square")])
}

fn make_phase_mode_parameter() -> Parameter {
    make_enum_parameter("Phase Mode", PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID, "frequency", &[("frequency", "Frequency"), ("numPhases", "Number of Phases")])
}

const EASING_REQUIRED_LINEAR_DECL_IDS: [&str; 1] = [PARAMETER_ANIMATION_EASING_KIND_DECL_ID];
const EASING_REQUIRED_BEZIER_DECL_IDS: [&str; 5] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID,
    PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID,
    PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID,
    PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID,
];
const EASING_REQUIRED_HOLD_DECL_IDS: [&str; 1] = [PARAMETER_ANIMATION_EASING_KIND_DECL_ID];
const EASING_REQUIRED_STEPS_DECL_IDS: [&str; 4] = [PARAMETER_ANIMATION_EASING_KIND_DECL_ID, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID, PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID, PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID];
const EASING_REQUIRED_SHAPE_DECL_IDS: [&str; 8] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID,
    PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID,
    PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
];
const EASING_REQUIRED_PERLIN_NOISE_DECL_IDS: [&str; 7] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID,
    PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID,
    PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_DECL_ID,
];
const EASING_REQUIRED_RANDOM_DECL_IDS: [&str; 5] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
    PARAMETER_ANIMATION_EASING_SEED_DECL_ID,
];
const EASING_REQUIRED_SCRIPT_DECL_IDS: [&str; 2] = [PARAMETER_ANIMATION_EASING_KIND_DECL_ID, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID];

const EASING_MANAGED_DECL_IDS: [&str; 20] = [
    PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    LEGACY_COORDINATE_SPACE_DECL_ID,
    PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID,
    PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID,
    PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID,
    PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID,
    PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID,
    PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID,
    PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID,
    PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID,
    PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
    PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID,
    PARAMETER_ANIMATION_EASING_PHASE_DECL_ID,
    PARAMETER_ANIMATION_EASING_SEED_DECL_ID,
    PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID,
];

fn read_child_param_value<'a>(snapshot: &'a ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<&'a ParamValue> {
    let child = snapshot.find_child(parent, decl_id)?;
    snapshot.node(child)?.param_value.as_ref()
}

fn read_child_param_f64(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: f64) -> f64 {
    read_child_param_value(snapshot, parent, decl_id).and_then(|value| value.as_float().or_else(|| value.as_int().map(|int_value| int_value as f64))).unwrap_or(default_value)
}

fn read_child_param_u32(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: u32) -> u32 {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(|value| value.as_int().or_else(|| value.as_float().map(|float_value| float_value.round() as i32)))
        .map(|value| value.max(0) as u32)
        .unwrap_or(default_value)
}

fn read_child_param_u64(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: u64) -> u64 {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(|value| value.as_int().or_else(|| value.as_float().map(|float_value| float_value.round() as i32)))
        .map(|value| value as i64 as u64)
        .unwrap_or(default_value)
}

fn read_child_param_enum(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: &str) -> String {
    read_child_param_value(snapshot, parent, decl_id).and_then(ParamValue::as_enum).unwrap_or_else(|| default_value.to_string())
}

fn read_child_param_string(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str, default_value: &str) -> String {
    read_child_param_value(snapshot, parent, decl_id).and_then(ParamValue::as_str).unwrap_or_else(|| default_value.to_string())
}

fn parse_step_mode(value: &str) -> CurveStepMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "stepsize" => CurveStepMode::StepSize,
        _ => CurveStepMode::NumSteps,
    }
}

fn parse_shape(value: &str) -> CurveShape {
    match value.trim().to_ascii_lowercase().as_str() {
        "triangle" => CurveShape::Triangle,
        "saw" => CurveShape::Saw,
        "reversesaw" => CurveShape::ReverseSaw,
        "square" => CurveShape::Square,
        _ => CurveShape::Sine,
    }
}

fn parse_phase_mode(value: &str) -> CurvePhaseMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "numphases" => CurvePhaseMode::NumPhases,
        _ => CurvePhaseMode::Frequency,
    }
}

fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

/// Axis-aligned curve bounds used to constrain key positions, key values, and sampled output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationCurveRangeConstraint {
    /// Minimum key position.
    pub x_min: f64,
    /// Maximum key position.
    pub x_max: f64,
    /// Minimum key/sample value.
    pub y_min: f64,
    /// Maximum key/sample value.
    pub y_max: f64,
}

impl AnimationCurveRangeConstraint {
    /// Default editable range.
    pub const DEFAULT: Self = Self { x_min: 0.0, x_max: 1.0, y_min: 0.0, y_max: 1.0 };

    /// Builds one normalized range.
    ///
    /// Returns `None` when any value is non-finite or a span is effectively zero.
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Option<Self> {
        if !x_min.is_finite() || !x_max.is_finite() || !y_min.is_finite() || !y_max.is_finite() {
            return None;
        }

        let (x_min, x_max) = if x_min <= x_max { (x_min, x_max) } else { (x_max, x_min) };
        let (y_min, y_max) = if y_min <= y_max { (y_min, y_max) } else { (y_max, y_min) };
        if (x_max - x_min).abs() <= CURVE_RANGE_EPSILON || (y_max - y_min).abs() <= CURVE_RANGE_EPSILON {
            return None;
        }

        Some(Self { x_min, x_max, y_min, y_max })
    }

    fn clamp_position(self, value: f64) -> f64 {
        clamp_f64(value, self.x_min, self.x_max)
    }

    fn clamp_value(self, value: f64) -> f64 {
        clamp_f64(value, self.y_min, self.y_max)
    }
}

fn read_range_constraint_from_range_node(snapshot: &ProcessTreeSnapshot, range_node: NodeId) -> Option<AnimationCurveRangeConstraint> {
    let node = snapshot.node(range_node)?;
    if node.node_type != PARAMETER_ANIMATION_RANGE_NODE_TYPE || !node.enabled {
        return None;
    }

    let x_bounds = read_child_param_value(snapshot, range_node, PARAMETER_ANIMATION_RANGE_X_DECL_ID).and_then(ParamValue::as_vec2)?;
    let y_bounds = read_child_param_value(snapshot, range_node, PARAMETER_ANIMATION_RANGE_Y_DECL_ID).and_then(ParamValue::as_vec2)?;
    AnimationCurveRangeConstraint::new(x_bounds.0, x_bounds.1, y_bounds.0, y_bounds.1)
}

fn read_range_axis_bounds(snapshot: &ProcessTreeSnapshot, range_node: NodeId, decl_id: &str) -> Option<(f64, f64)> {
    let bounds = read_child_param_value(snapshot, range_node, decl_id).and_then(ParamValue::as_vec2)?;
    Some((bounds.0, bounds.1))
}

fn read_uniform_constraint_bounds(range: Option<&RangeConstraint>) -> Option<(f64, f64)> {
    match range {
        Some(RangeConstraint::Uniform { min: Some(min), max: Some(max) }) if min.is_finite() && max.is_finite() => {
            if min <= max {
                Some((*min, *max))
            } else {
                Some((*max, *min))
            }
        }
        _ => None,
    }
}

fn read_range_constraint_from_key_param_constraints(snapshot: &ProcessTreeSnapshot, curve_node: NodeId) -> Option<AnimationCurveRangeConstraint> {
    let mut x_bounds: Option<(f64, f64)> = None;
    let mut y_bounds: Option<(f64, f64)> = None;

    for child_id in snapshot.child_ids(curve_node) {
        let Some(child_snapshot) = snapshot.node(child_id) else {
            continue;
        };
        if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
            continue;
        }

        if x_bounds.is_none() {
            if let Some(position_param) = snapshot.find_child(child_id, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID) {
                x_bounds = read_uniform_constraint_bounds(snapshot.node(position_param).and_then(|entry| entry.param_constraints.as_ref()).and_then(|constraints| constraints.range.as_ref()));
            }
        }

        if y_bounds.is_none() {
            if let Some(value_param) = snapshot.find_child(child_id, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID) {
                y_bounds = read_uniform_constraint_bounds(snapshot.node(value_param).and_then(|entry| entry.param_constraints.as_ref()).and_then(|constraints| constraints.range.as_ref()));
            }
        }

        if x_bounds.is_some() && y_bounds.is_some() {
            break;
        }
    }

    let (x_min, x_max) = x_bounds?;
    let (y_min, y_max) = y_bounds?;
    AnimationCurveRangeConstraint::new(x_min, x_max, y_min, y_max)
}

/// Internal node storing editable X/Y curve range bounds.
pub struct AnimationCurveRangeNode {
    node_data: NodeData,
    default_range: AnimationCurveRangeConstraint,
}

impl AnimationCurveRangeNode {
    /// Creates one range node with optional initial bounds and enable state.
    pub fn new(initial_range: Option<AnimationCurveRangeConstraint>, enabled: bool) -> Self {
        let mut node_data = NodeData::new("Range".to_string());
        node_data.meta.can_be_disabled = true;
        node_data.meta.enabled = enabled;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_RANGE_DECL_ID.to_string());
        Self {
            node_data,
            default_range: initial_range.unwrap_or(AnimationCurveRangeConstraint::DEFAULT),
        }
    }
}

impl Node for AnimationCurveRangeNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_RANGE_NODE_TYPE
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_RANGE_X_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_vec2_parameter("X", PARAMETER_ANIMATION_RANGE_X_DECL_ID, self.default_range.x_min, self.default_range.x_max)), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_RANGE_Y_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(make_vec2_parameter("Y", PARAMETER_ANIMATION_RANGE_Y_DECL_ID, self.default_range.y_min, self.default_range.y_max)), None);
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

/// Internal node hosting one animation-curve key list.
pub struct AnimationCurveNode {
    node_data: NodeData,
    user_can_edit_range: bool,
    code_range_constraint: Option<AnimationCurveRangeConstraint>,
    range_node: Option<NodeId>,
    default_keys_seeded: bool,
}

impl AnimationCurveNode {
    /// Creates one animation-curve node.
    pub fn new() -> Self {
        Self::new_with_label("Curve")
    }

    /// Creates one animation-curve node with custom label.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_CURVE_DECL_ID.to_string());
        Self {
            node_data,
            user_can_edit_range: true,
            code_range_constraint: None,
            range_node: None,
            default_keys_seeded: false,
        }
    }

    /// Enables/disables user-editable range controls.
    pub fn with_user_editable_range(mut self, user_can_edit_range: bool) -> Self {
        self.user_can_edit_range = user_can_edit_range;
        self
    }

    /// Sets whether range controls are user-editable.
    pub fn set_user_editable_range(&mut self, user_can_edit_range: bool) {
        self.user_can_edit_range = user_can_edit_range;
    }

    /// Sets one code-authored range constraint.
    pub fn with_range_constraint(mut self, range: Option<AnimationCurveRangeConstraint>) -> Self {
        self.code_range_constraint = range;
        self
    }

    /// Replaces the code-authored range constraint.
    pub fn set_range_constraint(&mut self, range: Option<AnimationCurveRangeConstraint>) {
        self.code_range_constraint = range;
    }

    fn bind_decl_child(&mut self, decl_id: &str, child: NodeId) {
        if decl_id == PARAMETER_ANIMATION_RANGE_DECL_ID {
            self.range_node = Some(child);
        }
    }

    fn unbind_child(&mut self, child: NodeId) {
        if self.range_node == Some(child) {
            self.range_node = None;
        }
    }

    fn initial_key_range_constraint(&self) -> Option<AnimationCurveRangeConstraint> {
        if self.user_can_edit_range { None } else { self.code_range_constraint }
    }

    fn pending_key_add_count(&self, ctx: &ProcessCtx) -> usize {
        ctx.edits
            .pending
            .iter()
            .filter(|request| {
                matches!(
                    &request.edit,
                    Edit::AddNode { parent, node, .. } | Edit::AddUserItem { parent, node, .. }
                        if *parent == self.id() && node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE
                )
            })
            .count()
    }

    fn effective_range_constraint(&self, snapshot: &ProcessTreeSnapshot) -> Option<AnimationCurveRangeConstraint> {
        if self.user_can_edit_range {
            let range_node = self.range_node.or_else(|| snapshot.find_child(self.id(), PARAMETER_ANIMATION_RANGE_DECL_ID))?;
            return read_range_constraint_from_range_node(snapshot, range_node);
        }
        self.code_range_constraint
    }

    fn editable_range_constraint(enabled: bool, x_bounds: Option<(f64, f64)>, y_bounds: Option<(f64, f64)>) -> Option<AnimationCurveRangeConstraint> {
        if !enabled {
            return None;
        }

        let (x_min, x_max) = x_bounds?;
        let (y_min, y_max) = y_bounds?;
        AnimationCurveRangeConstraint::new(x_min, x_max, y_min, y_max)
    }

    fn clamped_key_param_update(&self, snapshot: &ProcessTreeSnapshot, param: NodeId, new_value: &ParamValue, range: AnimationCurveRangeConstraint) -> Option<(NodeId, f64)> {
        let Some(param_snapshot) = snapshot.node(param) else {
            return None;
        };
        let Some(key_node) = param_snapshot.parent else {
            return None;
        };
        let Some(key_snapshot) = snapshot.node(key_node) else {
            return None;
        };
        if key_snapshot.parent != Some(self.id()) || key_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
            return None;
        }

        let Some(raw_value) = new_value.as_float().or_else(|| new_value.as_int().map(|int_value| int_value as f64)) else {
            return None;
        };

        let clamped = if param_snapshot.decl_id == PARAMETER_ANIMATION_KEY_POSITION_DECL_ID {
            range.clamp_position(raw_value)
        } else if param_snapshot.decl_id == PARAMETER_ANIMATION_KEY_VALUE_DECL_ID {
            range.clamp_value(raw_value)
        } else {
            return None;
        };

        if (clamped - raw_value).abs() <= CURVE_RANGE_EPSILON {
            return None;
        }

        Some((param, clamped))
    }

    fn collect_range_clamp_updates_for_all_keys(&self, snapshot: &ProcessTreeSnapshot, range: AnimationCurveRangeConstraint) -> Vec<(NodeId, f64)> {
        let mut updates = Vec::<(NodeId, f64)>::new();
        for child in snapshot.child_ids(self.id()) {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
                continue;
            }

            if let Some(position_param) = snapshot.find_child(child, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID) {
                if let Some(new_value) = snapshot.node(position_param).and_then(|entry| entry.param_value.as_ref()) {
                    if let Some(update) = self.clamped_key_param_update(snapshot, position_param, new_value, range) {
                        updates.push(update);
                    }
                }
            }
            if let Some(value_param) = snapshot.find_child(child, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID) {
                if let Some(new_value) = snapshot.node(value_param).and_then(|entry| entry.param_value.as_ref()) {
                    if let Some(update) = self.clamped_key_param_update(snapshot, value_param, new_value, range) {
                        updates.push(update);
                    }
                }
            }
        }
        updates
    }

    fn collect_key_reorder_moves_by_position(&self, snapshot: &ProcessTreeSnapshot) -> Vec<(NodeId, Option<NodeId>)> {
        let children = snapshot.child_ids(self.id());
        if children.len() < 2 {
            return Vec::new();
        }

        let mut key_entries = Vec::<(usize, NodeId, f64)>::new();
        for (index, child) in children.iter().copied().enumerate() {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
                continue;
            }
            let position = read_child_param_f64(snapshot, child, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, 0.0);
            key_entries.push((index, child, position));
        }
        if key_entries.len() < 2 {
            return Vec::new();
        }

        let mut sorted_key_entries = key_entries.clone();
        sorted_key_entries.sort_by(|left, right| {
            if (left.2 - right.2).abs() <= KEY_ORDER_POSITION_EPSILON {
                left.0.cmp(&right.0)
            } else {
                left.2.total_cmp(&right.2)
            }
        });

        let current_key_order = key_entries.iter().map(|(_, key_id, _)| *key_id).collect::<Vec<_>>();
        let sorted_key_order = sorted_key_entries.iter().map(|(_, key_id, _)| *key_id).collect::<Vec<_>>();
        if current_key_order == sorted_key_order {
            return Vec::new();
        }

        let sorted_key_set = sorted_key_order.iter().copied().collect::<HashSet<NodeId>>();
        let mut sorted_key_iter = sorted_key_order.into_iter();
        let mut desired_children = Vec::<NodeId>::with_capacity(children.len());
        for child in children.iter().copied() {
            if sorted_key_set.contains(&child) {
                if let Some(next_key) = sorted_key_iter.next() {
                    desired_children.push(next_key);
                } else {
                    desired_children.push(child);
                }
            } else {
                desired_children.push(child);
            }
        }

        let mut simulated_children = children;
        let mut moves = Vec::<(NodeId, Option<NodeId>)>::new();
        for index in 0..desired_children.len() {
            let desired_child = desired_children[index];
            if !sorted_key_set.contains(&desired_child) {
                continue;
            }
            if simulated_children.get(index).copied() == Some(desired_child) {
                continue;
            }
            let Some(current_index) = simulated_children.iter().position(|node_id| *node_id == desired_child) else {
                continue;
            };
            let moved_child = simulated_children.remove(current_index);
            simulated_children.insert(index, moved_child);
            let after = if index == 0 { None } else { Some(simulated_children[index - 1]) };
            moves.push((desired_child, after));
        }

        moves
    }

    fn sync_range_node_presence(&mut self, ctx: &mut ProcessCtx) {
        let mut range_children = Vec::<NodeId>::new();
        if let Some(snapshot) = ctx.tree_snapshot() {
            for child in snapshot.child_ids(self.id()) {
                let Some(child_snapshot) = snapshot.node(child) else {
                    continue;
                };
                if child_snapshot.decl_id == PARAMETER_ANIMATION_RANGE_DECL_ID {
                    range_children.push(child);
                }
            }
        }

        if self.user_can_edit_range {
            self.range_node = range_children.first().copied();
            for duplicate in range_children.iter().skip(1).copied() {
                self.remove_child(ctx, duplicate);
            }
            if self.range_node.is_none() {
                self.add_child_boxed(ctx, Box::new(AnimationCurveRangeNode::new(self.code_range_constraint, self.code_range_constraint.is_some())), None);
            }
            return;
        }

        self.range_node = None;
        for child in range_children {
            self.remove_child(ctx, child);
        }
    }

    fn sync_keys_and_clamp_to_active_range(&mut self, ctx: &mut ProcessCtx) {
        self.sync_range_node_presence(ctx);

        let existing_key_count = ctx
            .tree_snapshot()
            .map(|snapshot| snapshot.child_ids(self.id()).into_iter().filter(|node_id| snapshot.node(*node_id).is_some_and(|node| node.node_type == PARAMETER_ANIMATION_KEY_NODE_TYPE)).count())
            .unwrap_or(0);
        let pending_key_count = self.pending_key_add_count(ctx);
        if existing_key_count > 0 {
            self.default_keys_seeded = true;
        }

        if existing_key_count + pending_key_count == 0 && !self.default_keys_seeded {
            let initial_range = self.initial_key_range_constraint();
            ctx.add_child_boxed(self.id(), Box::new(AnimationCurveKeyNode::new_with_values_and_range(0.0, 0.0, initial_range)), None);
            ctx.add_child_boxed(self.id(), Box::new(AnimationCurveKeyNode::new_with_values_and_range(1.0, 1.0, initial_range)), None);
            self.default_keys_seeded = true;
        }

        let clamp_updates = ctx.tree_snapshot().and_then(|snapshot| self.effective_range_constraint(snapshot).map(|range| self.collect_range_clamp_updates_for_all_keys(snapshot, range))).unwrap_or_default();
        for (param, value) in clamp_updates {
            ctx.set_param_with_behaviour(param, ParamValue::Float(value), ParameterEventBehaviour::Coalesce);
        }

        let reorder_moves = ctx.tree_snapshot().map(|snapshot| self.collect_key_reorder_moves_by_position(snapshot)).unwrap_or_default();
        for (child, after) in reorder_moves {
            self.move_child(ctx, child, self.id(), after);
        }
    }
}

impl Node for AnimationCurveNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_CURVE_NODE_TYPE
    }

    fn user_item_kind(&self) -> &str {
        PARAMETER_ANIMATION_CURVE_ITEM_KIND
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[PARAMETER_ANIMATION_KEY_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new(PARAMETER_ANIMATION_KEY_NODE_TYPE, PARAMETER_ANIMATION_KEY_ITEM_KIND, "Key")]
    }

    fn create_user_item(&self, node_type: &str, label: String) -> Option<Box<dyn Node>> {
        let normalized = node_type.trim().to_ascii_lowercase();
        if normalized == PARAMETER_ANIMATION_KEY_NODE_TYPE || normalized == "key" {
            return Some(Box::new(AnimationCurveKeyNode::new_with_label_and_range(label, self.initial_key_range_constraint())));
        }
        None
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        self.sync_keys_and_clamp_to_active_range(ctx);
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.sync_keys_and_clamp_to_active_range(ctx);
    }

    fn engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        let (mut remove_children, add_range_node, clamp_updates, reorder_moves) = {
            let Some(snapshot) = ctx.tree_snapshot() else {
                return;
            };

            let mut remove_children = Vec::<NodeId>::new();
            let mut add_range_node = false;
            let mut clamp_updates = HashMap::<NodeId, f64>::new();
            let mut pending_key_param_values = HashMap::<NodeId, f64>::new();
            let mut enforce_all = false;
            let mut should_reorder_keys = false;
            let mut tracked_range_node = self.range_node.or_else(|| snapshot.find_child(self.id(), PARAMETER_ANIMATION_RANGE_DECL_ID));
            let mut editable_range_enabled = tracked_range_node.and_then(|range_node| snapshot.node(range_node).map(|node| node.enabled)).unwrap_or(false);
            let mut editable_x_bounds = tracked_range_node.and_then(|range_node| read_range_axis_bounds(snapshot, range_node, PARAMETER_ANIMATION_RANGE_X_DECL_ID));
            let mut editable_y_bounds = tracked_range_node.and_then(|range_node| read_range_axis_bounds(snapshot, range_node, PARAMETER_ANIMATION_RANGE_Y_DECL_ID));
            let mut range_constraint = if self.user_can_edit_range {
                Self::editable_range_constraint(editable_range_enabled, editable_x_bounds, editable_y_bounds)
            } else {
                self.code_range_constraint
            };

            for event in &ctx.events {
                match &event.kind {
                    EventKind::ChildAdded { parent, child, decl_id } if *parent == self.id() => {
                        if decl_id.0 == PARAMETER_ANIMATION_RANGE_DECL_ID {
                            if !self.user_can_edit_range {
                                remove_children.push(*child);
                                continue;
                            }
                            if let Some(existing) = self.range_node {
                                if existing != *child {
                                    remove_children.push(*child);
                                    continue;
                                }
                            }
                            tracked_range_node = Some(*child);
                            editable_range_enabled = snapshot.node(*child).is_some_and(|node| node.enabled);
                            editable_x_bounds = read_range_axis_bounds(snapshot, *child, PARAMETER_ANIMATION_RANGE_X_DECL_ID);
                            editable_y_bounds = read_range_axis_bounds(snapshot, *child, PARAMETER_ANIMATION_RANGE_Y_DECL_ID);
                            range_constraint = Self::editable_range_constraint(editable_range_enabled, editable_x_bounds, editable_y_bounds);
                        }
                        self.bind_decl_child(decl_id.0.as_str(), *child);
                        enforce_all = true;
                        should_reorder_keys = true;
                    }
                    EventKind::ChildReplaced { parent, old, new, decl_id } if *parent == self.id() => {
                        self.unbind_child(*old);
                        if decl_id.0 == PARAMETER_ANIMATION_RANGE_DECL_ID {
                            if !self.user_can_edit_range {
                                remove_children.push(*new);
                                continue;
                            }
                            if let Some(existing) = self.range_node {
                                if existing != *new {
                                    remove_children.push(*new);
                                    continue;
                                }
                            }
                            tracked_range_node = Some(*new);
                            editable_range_enabled = snapshot.node(*new).is_some_and(|node| node.enabled);
                            editable_x_bounds = read_range_axis_bounds(snapshot, *new, PARAMETER_ANIMATION_RANGE_X_DECL_ID);
                            editable_y_bounds = read_range_axis_bounds(snapshot, *new, PARAMETER_ANIMATION_RANGE_Y_DECL_ID);
                            range_constraint = Self::editable_range_constraint(editable_range_enabled, editable_x_bounds, editable_y_bounds);
                        }
                        self.bind_decl_child(decl_id.0.as_str(), *new);
                        enforce_all = true;
                        should_reorder_keys = true;
                    }
                    EventKind::ChildRemoved { parent, child } if *parent == self.id() => {
                        let removed_range = self.range_node == Some(*child);
                        self.unbind_child(*child);
                        if removed_range && self.user_can_edit_range {
                            tracked_range_node = None;
                            editable_range_enabled = false;
                            editable_x_bounds = None;
                            editable_y_bounds = None;
                            range_constraint = None;
                            add_range_node = true;
                        }
                        enforce_all = true;
                        should_reorder_keys = true;
                    }
                    EventKind::ChildReordered { parent, .. } if *parent == self.id() => {
                        enforce_all = true;
                        should_reorder_keys = true;
                    }
                    EventKind::ParamChanged { param, new_value, .. } => {
                        if let Some(param_snapshot) = snapshot.node(*param) {
                            if let Some(key_node) = param_snapshot.parent {
                                if let Some(key_snapshot) = snapshot.node(key_node) {
                                    if key_snapshot.parent == Some(self.id()) && key_snapshot.node_type == PARAMETER_ANIMATION_KEY_NODE_TYPE {
                                        let decl_id = param_snapshot.decl_id.as_str();
                                        if decl_id == PARAMETER_ANIMATION_KEY_POSITION_DECL_ID || decl_id == PARAMETER_ANIMATION_KEY_VALUE_DECL_ID {
                                            if let Some(raw_value) = new_value.as_float().or_else(|| new_value.as_int().map(|int_value| int_value as f64)) {
                                                pending_key_param_values.insert(*param, raw_value);
                                            }
                                        }
                                        if decl_id == PARAMETER_ANIMATION_KEY_POSITION_DECL_ID {
                                            should_reorder_keys = true;
                                        }
                                    }
                                }
                            }
                        }

                        if let (true, Some(range_node), Some(param_snapshot)) = (self.user_can_edit_range, tracked_range_node, snapshot.node(*param)) {
                            if param_snapshot.parent == Some(range_node) {
                                if param_snapshot.decl_id == PARAMETER_ANIMATION_RANGE_X_DECL_ID {
                                    editable_x_bounds = new_value.as_vec2().map(|bounds| (bounds.0, bounds.1));
                                    range_constraint = Self::editable_range_constraint(editable_range_enabled, editable_x_bounds, editable_y_bounds);
                                    enforce_all = true;
                                } else if param_snapshot.decl_id == PARAMETER_ANIMATION_RANGE_Y_DECL_ID {
                                    editable_y_bounds = new_value.as_vec2().map(|bounds| (bounds.0, bounds.1));
                                    range_constraint = Self::editable_range_constraint(editable_range_enabled, editable_x_bounds, editable_y_bounds);
                                    enforce_all = true;
                                }
                            }
                        }
                        if let Some(range) = range_constraint {
                            if let Some(update) = self.clamped_key_param_update(snapshot, *param, new_value, range) {
                                clamp_updates.insert(update.0, update.1);
                            }
                        }
                    }
                    EventKind::MetaChanged { node, patch } => {
                        if self.user_can_edit_range && Some(*node) == tracked_range_node && patch.enabled.is_some() {
                            editable_range_enabled = patch.enabled.unwrap_or(editable_range_enabled);
                            range_constraint = Self::editable_range_constraint(editable_range_enabled, editable_x_bounds, editable_y_bounds);
                            enforce_all = true;
                        }
                    }
                    _ => {}
                }
            }

            if enforce_all {
                if let Some(range) = range_constraint {
                    for (param, value) in self.collect_range_clamp_updates_for_all_keys(snapshot, range) {
                        clamp_updates.insert(param, value);
                    }
                    for (param, raw_value) in pending_key_param_values {
                        if let Some(update) = self.clamped_key_param_update(snapshot, param, &ParamValue::Float(raw_value), range) {
                            clamp_updates.insert(update.0, update.1);
                        }
                    }
                }
            }

            let mut clamp_updates: Vec<(NodeId, f64)> = clamp_updates.into_iter().collect();
            clamp_updates.sort_unstable_by_key(|(param, _)| param.0);
            let reorder_moves = if should_reorder_keys {
                self.collect_key_reorder_moves_by_position(snapshot)
            } else {
                Vec::new()
            };
            (remove_children, add_range_node, clamp_updates, reorder_moves)
        };

        if !remove_children.is_empty() {
            remove_children.sort_unstable_by_key(|node_id| node_id.0);
            remove_children.dedup();
            for child in remove_children {
                self.remove_child(ctx, child);
            }
        }

        if add_range_node {
            self.add_child_boxed(ctx, Box::new(AnimationCurveRangeNode::new(self.code_range_constraint, self.code_range_constraint.is_some())), None);
        }

        if !clamp_updates.is_empty() {
            for (param, value) in clamp_updates {
                ctx.set_param_with_behaviour(param, ParamValue::Float(value), ParameterEventBehaviour::Coalesce);
            }
        }

        if !reorder_moves.is_empty() {
            for (child, after) in reorder_moves {
                self.move_child(ctx, child, self.id(), after);
            }
        }
    }

    fn on_child_added_decl(&mut self, _ctx: &mut ProcessCtx, parent: NodeId, child: NodeId, decl_id: &DeclId) {
        if parent != self.id() {
            return;
        }
        self.bind_decl_child(decl_id.0.as_str(), child);
    }

    fn on_child_replaced_decl(&mut self, _ctx: &mut ProcessCtx, parent: NodeId, old: NodeId, new: NodeId, decl_id: &DeclId) {
        if parent != self.id() {
            return;
        }
        self.unbind_child(old);
        self.bind_decl_child(decl_id.0.as_str(), new);
    }

    fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        if parent != self.id() {
            return;
        }
        self.unbind_child(child);
    }

    fn engine_child_event_interest_depth(&self, _event: &Event) -> u32 {
        2
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}

/// Internal node representing one animation curve key.
pub struct AnimationCurveKeyNode {
    node_data: NodeData,
    default_position: f64,
    default_value: f64,
    range_constraint: Option<AnimationCurveRangeConstraint>,
}

impl AnimationCurveKeyNode {
    /// Creates one key node with default position/value set to `0`.
    pub fn new() -> Self {
        Self::new_with_label_and_values_and_range("Key", 0.0, 0.0, None)
    }

    /// Creates one key node with custom label and default position/value.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        Self::new_with_label_and_values_and_range(label, 0.0, 0.0, None)
    }

    /// Creates one key node with custom label and optional range constraint.
    pub fn new_with_label_and_range(label: impl Into<String>, range_constraint: Option<AnimationCurveRangeConstraint>) -> Self {
        Self::new_with_label_and_values_and_range(label, 0.0, 0.0, range_constraint)
    }

    /// Creates one key node with explicit initial position/value.
    pub fn new_with_values(position: f64, value: f64) -> Self {
        Self::new_with_label_and_values_and_range("Key", position, value, None)
    }

    /// Creates one key node with explicit initial position/value and optional range constraint.
    pub fn new_with_values_and_range(position: f64, value: f64, range_constraint: Option<AnimationCurveRangeConstraint>) -> Self {
        Self::new_with_label_and_values_and_range("Key", position, value, range_constraint)
    }

    fn new_with_label_and_values_and_range(label: impl Into<String>, position: f64, value: f64, range_constraint: Option<AnimationCurveRangeConstraint>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        let (default_position, default_value) = if let Some(range_constraint) = range_constraint {
            (range_constraint.clamp_position(position), range_constraint.clamp_value(value))
        } else {
            (position, value)
        };
        Self {
            node_data,
            default_position,
            default_value,
            range_constraint,
        }
    }
}

impl Node for AnimationCurveKeyNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_KEY_NODE_TYPE
    }

    fn user_item_kind(&self) -> &str {
        PARAMETER_ANIMATION_KEY_ITEM_KIND
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_KEY_POSITION_DECL_ID) {
            let mut position_param = make_float_parameter("Position", PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, self.default_position);
            if let Some(range_constraint) = self.range_constraint {
                position_param.constraints.range = Some(RangeConstraint::Uniform {
                    min: Some(range_constraint.x_min),
                    max: Some(range_constraint.x_max),
                });
            }
            ctx.add_child_boxed(self.id(), Box::new(position_param), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_KEY_VALUE_DECL_ID) {
            let mut value_param = make_float_parameter("Value", PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, self.default_value);
            if let Some(range_constraint) = self.range_constraint {
                value_param.constraints.range = Some(RangeConstraint::Uniform {
                    min: Some(range_constraint.y_min),
                    max: Some(range_constraint.y_max),
                });
            }
            ctx.add_child_boxed(self.id(), Box::new(value_param), None);
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_EASING_DECL_ID) {
            ctx.add_child_boxed(self.id(), Box::new(AnimationCurveEasingNode::new("Easing")), None);
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

/// Internal node storing one key-to-next easing specification.
pub struct AnimationCurveEasingNode {
    node_data: NodeData,
    current_kind: &'static str,
    kind_param: Option<NodeId>,
    managed_children: HashMap<String, Option<NodeId>>,
}

impl AnimationCurveEasingNode {
    /// Creates one easing node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_EASING_DECL_ID.to_string());
        Self {
            node_data,
            current_kind: "bezier",
            kind_param: None,
            managed_children: HashMap::new(),
        }
    }

    fn normalize_kind(kind: &str) -> &'static str {
        match kind.trim().to_ascii_lowercase().as_str() {
            "bezier" => "bezier",
            "hold" => "hold",
            "steps" => "steps",
            "shape" => "shape",
            "perlinnoise" => "perlinnoise",
            "random" => "random",
            "script" => "script",
            _ => "linear",
        }
    }

    fn required_decl_ids_for_kind(kind: &str) -> &'static [&'static str] {
        match Self::normalize_kind(kind) {
            "bezier" => &EASING_REQUIRED_BEZIER_DECL_IDS,
            "hold" => &EASING_REQUIRED_HOLD_DECL_IDS,
            "steps" => &EASING_REQUIRED_STEPS_DECL_IDS,
            "shape" => &EASING_REQUIRED_SHAPE_DECL_IDS,
            "perlinnoise" => &EASING_REQUIRED_PERLIN_NOISE_DECL_IDS,
            "random" => &EASING_REQUIRED_RANDOM_DECL_IDS,
            "script" => &EASING_REQUIRED_SCRIPT_DECL_IDS,
            _ => &EASING_REQUIRED_LINEAR_DECL_IDS,
        }
    }

    fn is_managed_decl_id(decl_id: &str) -> bool {
        EASING_MANAGED_DECL_IDS.contains(&decl_id)
    }

    fn is_required_decl_id_for_kind(kind: &str, decl_id: &str) -> bool {
        Self::required_decl_ids_for_kind(kind).contains(&decl_id)
    }

    fn kind_from_param_value(value: &ParamValue) -> &'static str {
        if let Some(kind) = value.as_enum() {
            return Self::normalize_kind(kind.as_str());
        }
        if let Some(kind) = value.as_str() {
            return Self::normalize_kind(kind.as_str());
        }
        "linear"
    }

    fn kind_from_snapshot(snapshot: &ProcessTreeSnapshot, easing_node: NodeId) -> &'static str {
        let kind = read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID, "linear");
        Self::normalize_kind(kind.as_str())
    }

    fn bind_decl_child(&mut self, decl_id: &str, child: NodeId) {
        if !Self::is_managed_decl_id(decl_id) {
            return;
        }
        self.managed_children.insert(decl_id.to_string(), Some(child));
        if decl_id == PARAMETER_ANIMATION_EASING_KIND_DECL_ID {
            self.kind_param = Some(child);
        }
    }

    fn mark_decl_child_pending_addition(&mut self, decl_id: &str) {
        if !Self::is_managed_decl_id(decl_id) {
            return;
        }
        self.managed_children.entry(decl_id.to_string()).or_insert(None);
    }

    fn unbind_child(&mut self, child: NodeId) {
        let removed_decl = self.managed_children.iter().find_map(|(decl_id, node_id)| (*node_id == Some(child)).then_some(decl_id.clone()));

        if let Some(decl_id) = removed_decl {
            self.managed_children.remove(decl_id.as_str());
        }
        if self.kind_param == Some(child) {
            self.kind_param = None;
        }
    }

    fn make_parameter_node(decl_id: &str) -> Option<Box<dyn Node>> {
        match decl_id {
            PARAMETER_ANIMATION_EASING_KIND_DECL_ID => Some(Box::new(make_easing_kind_parameter())),
            PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID => Some(Box::new(make_float_parameter("Out Handle Position", PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID, 1.0 / 3.0))),
            PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID => Some(Box::new(make_float_parameter("Out Handle Value", PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID, 0.0))),
            PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID => Some(Box::new(make_float_parameter("In Handle Position", PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID, -1.0 / 3.0))),
            PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID => Some(Box::new(make_float_parameter("In Handle Value", PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, 0.0))),
            PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID => Some(Box::new(make_step_mode_parameter())),
            PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID => Some(Box::new(make_non_negative_float_parameter("Step Size", PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID, 0.1))),
            PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID => {
                let mut parameter = make_int_parameter("Number of Steps", PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID, 8);
                parameter.constraints.range = Some(RangeConstraint::Uniform { min: Some(1.0), max: None });
                Some(Box::new(parameter))
            }
            PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID => Some(Box::new(make_shape_parameter())),
            PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID => Some(Box::new(make_float_parameter("Amplitude", PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, 1.0))),
            PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID => Some(Box::new(make_phase_mode_parameter())),
            PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID => Some(Box::new(make_non_negative_float_parameter("Frequency", PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 1.0))),
            PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID => Some(Box::new(make_non_negative_float_parameter("Number of Phases", PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID, 1.0))),
            PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID => Some(Box::new(make_non_negative_float_parameter("Fade In", PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0))),
            PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID => Some(Box::new(make_non_negative_float_parameter("Fade Out", PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0))),
            PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID => {
                let mut parameter = make_int_parameter("Octaves", PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, 4);
                parameter.constraints.range = Some(RangeConstraint::Uniform { min: Some(1.0), max: None });
                Some(Box::new(parameter))
            }
            PARAMETER_ANIMATION_EASING_PHASE_DECL_ID => Some(Box::new(make_float_parameter("Phase", PARAMETER_ANIMATION_EASING_PHASE_DECL_ID, 0.0))),
            PARAMETER_ANIMATION_EASING_SEED_DECL_ID => Some(Box::new(make_int_parameter("Seed", PARAMETER_ANIMATION_EASING_SEED_DECL_ID, 0))),
            PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID => Some(Box::new(make_string_parameter("Script Source", PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID, ""))),
            _ => None,
        }
    }

    fn sync_parameter_children_for_kind(&mut self, ctx: &mut ProcessCtx, kind: &str) {
        let normalized_kind = Self::normalize_kind(kind);
        if let Some(snapshot) = ctx.tree_snapshot() {
            self.managed_children.clear();
            self.kind_param = None;
            let mut duplicate_children = Vec::new();
            for child_id in snapshot.child_ids(self.id()) {
                let Some(child_snapshot) = snapshot.node(child_id) else {
                    continue;
                };
                if child_snapshot.param_value.is_none() {
                    continue;
                }
                let decl_id = child_snapshot.decl_id.as_str();
                if !Self::is_managed_decl_id(decl_id) {
                    continue;
                }
                if self.managed_children.contains_key(decl_id) {
                    duplicate_children.push(child_id);
                    continue;
                }
                self.bind_decl_child(decl_id, child_id);
            }

            for duplicate_child in duplicate_children {
                self.remove_child(ctx, duplicate_child);
            }
        }

        for required_decl_id in Self::required_decl_ids_for_kind(normalized_kind) {
            if self.managed_children.contains_key(*required_decl_id) {
                continue;
            }
            if let Some(parameter_node) = Self::make_parameter_node(required_decl_id) {
                self.mark_decl_child_pending_addition(required_decl_id);
                self.add_child_boxed(ctx, parameter_node, None);
            }
        }

        let existing_children_by_decl = self.managed_children.clone();
        for (decl_id, maybe_child_id) in existing_children_by_decl {
            if Self::is_required_decl_id_for_kind(normalized_kind, decl_id.as_str()) {
                continue;
            }
            self.managed_children.remove(decl_id.as_str());
            if let Some(child_id) = maybe_child_id {
                self.remove_child(ctx, child_id);
                if self.kind_param == Some(child_id) {
                    self.kind_param = None;
                }
            } else if decl_id == PARAMETER_ANIMATION_EASING_KIND_DECL_ID {
                self.kind_param = None;
            }
        }
    }
}

impl Node for AnimationCurveEasingNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        PARAMETER_ANIMATION_EASING_NODE_TYPE
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }

    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        let current_kind = ctx.tree_snapshot().map(|snapshot| Self::kind_from_snapshot(snapshot, self.id())).unwrap_or(self.current_kind);
        self.current_kind = current_kind;
        self.sync_parameter_children_for_kind(ctx, self.current_kind);
    }

    fn engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        let mut next_kind_from_event: Option<&'static str> = None;
        let mut should_sync = !self.managed_children.contains_key(PARAMETER_ANIMATION_EASING_KIND_DECL_ID);
        let mut duplicate_children_to_remove = Vec::new();

        for event in &ctx.events {
            match &event.kind {
                EventKind::ParamChanged { param, new_value, .. } => {
                    if Some(*param) == self.kind_param {
                        next_kind_from_event = Some(Self::kind_from_param_value(new_value));
                        should_sync = true;
                    }
                }
                EventKind::ChildAdded { parent, child, decl_id } => {
                    if *parent == self.id() {
                        let child_decl_id = decl_id.0.as_str();
                        if let Some(Some(existing_child)) = self.managed_children.get(child_decl_id) {
                            if *existing_child != *child {
                                duplicate_children_to_remove.push(*child);
                                should_sync = true;
                                continue;
                            }
                        }
                        self.bind_decl_child(child_decl_id, *child);
                        should_sync = true;
                    }
                }
                EventKind::ChildRemoved { parent, child } => {
                    if *parent == self.id() {
                        self.unbind_child(*child);
                        should_sync = true;
                    }
                }
                EventKind::ChildReplaced { parent, old, new, decl_id } => {
                    if *parent == self.id() {
                        let child_decl_id = decl_id.0.as_str();
                        self.unbind_child(*old);
                        if let Some(Some(existing_child)) = self.managed_children.get(child_decl_id) {
                            if *existing_child != *new {
                                duplicate_children_to_remove.push(*new);
                                should_sync = true;
                                continue;
                            }
                        }
                        self.bind_decl_child(child_decl_id, *new);
                        should_sync = true;
                    }
                }
                EventKind::ChildReordered { parent, .. } => {
                    if *parent == self.id() {
                        should_sync = true;
                    }
                }
                EventKind::ChildMoved { old_parent, new_parent, .. } => {
                    if *old_parent == self.id() || *new_parent == self.id() {
                        should_sync = true;
                    }
                }
                _ => {}
            }
        }

        for duplicate_child in duplicate_children_to_remove {
            self.remove_child(ctx, duplicate_child);
        }

        if !should_sync {
            return;
        }

        if let Some(next_kind) = next_kind_from_event {
            self.current_kind = next_kind;
        }
        self.sync_parameter_children_for_kind(ctx, self.current_kind);
    }
}

fn parse_easing_from_snapshot(snapshot: &ProcessTreeSnapshot, easing_node: NodeId) -> CurveEasing {
    let kind = read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID, "linear");
    match kind.trim().to_ascii_lowercase().as_str() {
        "bezier" => CurveEasing::Bezier {
            out_handle: CurveHandle::new(
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID, 1.0 / 3.0),
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID, 0.0),
            ),
            in_handle: CurveHandle::new(
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID, -1.0 / 3.0),
                read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, 0.0),
            ),
        },
        "hold" => CurveEasing::Hold,
        "steps" => CurveEasing::Steps {
            step_mode: parse_step_mode(read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID, "numSteps").as_str()),
            step_size: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID, 0.1),
            num_steps: read_child_param_u32(snapshot, easing_node, PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID, 8).max(1),
        },
        "shape" => CurveEasing::Shape {
            shape: parse_shape(read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID, "sine").as_str()),
            amplitude: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, 1.0),
            phase_mode: parse_phase_mode(read_child_param_enum(snapshot, easing_node, PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID, "frequency").as_str()),
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 1.0),
            num_phases: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID, 1.0),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
        },
        "perlinnoise" => CurveEasing::PerlinNoise {
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 1.0),
            amplitude: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, 1.0),
            octaves: read_child_param_u32(snapshot, easing_node, PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, 4).max(1),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
            phase: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_PHASE_DECL_ID, 0.0),
        },
        "random" => CurveEasing::Random {
            frequency: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, 6.0),
            fade_in: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, 0.0),
            fade_out: read_child_param_f64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, 0.0),
            seed: read_child_param_u64(snapshot, easing_node, PARAMETER_ANIMATION_EASING_SEED_DECL_ID, 0),
        },
        "script" => CurveEasing::Script {
            source: read_child_param_string(snapshot, easing_node, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID, ""),
        },
        _ => CurveEasing::Linear,
    }
}

fn parse_key_from_snapshot(snapshot: &ProcessTreeSnapshot, key_node: NodeId, range_constraint: Option<AnimationCurveRangeConstraint>) -> Option<AnimationCurveKey> {
    let key = snapshot.node(key_node)?;
    if key.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
        return None;
    }

    let mut position = read_child_param_f64(snapshot, key_node, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, 0.0);
    let mut value = read_child_param_f64(snapshot, key_node, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, 0.0);
    if let Some(range_constraint) = range_constraint {
        position = range_constraint.clamp_position(position);
        value = range_constraint.clamp_value(value);
    }
    let easing = snapshot.find_child(key_node, PARAMETER_ANIMATION_EASING_DECL_ID).map(|easing_node| parse_easing_from_snapshot(snapshot, easing_node)).unwrap_or(CurveEasing::Linear);

    Some(AnimationCurveKey::new(position, value, easing))
}

/// Builds an [`AnimationCurve`] from one curve-node subtree in a processing snapshot.
pub fn curve_from_snapshot(snapshot: &ProcessTreeSnapshot, curve_node: NodeId) -> Option<AnimationCurve> {
    let curve_snapshot = snapshot.node(curve_node)?;
    if curve_snapshot.node_type != PARAMETER_ANIMATION_CURVE_NODE_TYPE {
        return None;
    }

    let range_constraint = if let Some(range_node) = snapshot.find_child(curve_node, PARAMETER_ANIMATION_RANGE_DECL_ID) {
        read_range_constraint_from_range_node(snapshot, range_node)
    } else {
        read_range_constraint_from_key_param_constraints(snapshot, curve_node)
    };

    let keys = snapshot.child_ids(curve_node).into_iter().filter_map(|child_id| parse_key_from_snapshot(snapshot, child_id, range_constraint)).collect::<Vec<_>>();

    let mut curve = AnimationCurve::new(keys);
    if let Some(range_constraint) = range_constraint {
        curve.set_value_range_constraint(Some(range_constraint.y_min), Some(range_constraint.y_max));
    }
    Some(curve)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_helpers_map_variants() {
        assert_eq!(parse_step_mode("stepSize"), CurveStepMode::StepSize);
        assert_eq!(parse_shape("reverseSaw"), CurveShape::ReverseSaw);
        assert_eq!(parse_phase_mode("numPhases"), CurvePhaseMode::NumPhases);
    }
}
