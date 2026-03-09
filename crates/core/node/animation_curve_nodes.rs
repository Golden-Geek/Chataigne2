use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;

use crate::animation_curve::{AnimationCurve, AnimationCurveBezierFitOptions, AnimationCurveFitPoint, AnimationCurveKey, CurveEasing, CurveHandle, CurvePhaseMode, CurveShape, CurveStepMode, bezier_easing_from_endpoint_slopes, fit_points_to_bezier_keys};
use crate::edit::Edit;
use crate::events::{Event, EventKind};
use crate::node::NodeId;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour, RangeConstraint};
use crate::process_ctx::{ProcessCtx, ProcessTreeSnapshot};

use super::{
    DeclId, EventPropagation, Node, NodeData, PARAMETER_ANIMATION_CURVE_DECL_ID, PARAMETER_ANIMATION_CURVE_ITEM_KIND, PARAMETER_ANIMATION_CURVE_NODE_TYPE, PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, PARAMETER_ANIMATION_EASING_DECL_ID, PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID,
    PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID, PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, PARAMETER_ANIMATION_EASING_KIND_DECL_ID, PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID,
    PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID, PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID, PARAMETER_ANIMATION_EASING_PHASE_DECL_ID, PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID,
    PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID, PARAMETER_ANIMATION_EASING_SEED_DECL_ID, PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID, PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID, PARAMETER_ANIMATION_KEY_ITEM_KIND, PARAMETER_ANIMATION_KEY_NODE_TYPE,
    PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, PARAMETER_ANIMATION_RANGE_DECL_ID, PARAMETER_ANIMATION_RANGE_NODE_TYPE, PARAMETER_ANIMATION_RANGE_X_DECL_ID, PARAMETER_ANIMATION_RANGE_Y_DECL_ID, UserContainerRules, UserCreatableItem, parameter_child_exists,
};

const CURVE_RANGE_EPSILON: f64 = 1e-9;
const KEY_ORDER_POSITION_EPSILON: f64 = 1e-10;

fn make_float_parameter(label: &str, decl_id: &str, default_value: f64) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Float(default_value), ParameterChangeCheck::ValueChange);
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

fn default_curve_easing() -> CurveEasing {
    CurveEasing::Bezier {
        out_handle: CurveHandle::new(1.0 / 3.0, 0.0),
        in_handle: CurveHandle::new(-1.0 / 3.0, 0.0),
    }
}

fn curve_easing_kind_id(easing: &CurveEasing) -> &'static str {
    match easing {
        CurveEasing::Linear => "linear",
        CurveEasing::Bezier { .. } => "bezier",
        CurveEasing::Hold => "hold",
        CurveEasing::Steps { .. } => "steps",
        CurveEasing::Shape { .. } => "shape",
        CurveEasing::PerlinNoise { .. } => "perlinNoise",
        CurveEasing::Random { .. } => "random",
        CurveEasing::Script { .. } => "script",
    }
}

fn curve_step_mode_variant_id(mode: CurveStepMode) -> &'static str {
    match mode {
        CurveStepMode::StepSize => "stepSize",
        CurveStepMode::NumSteps => "numSteps",
    }
}

fn curve_shape_variant_id(shape: CurveShape) -> &'static str {
    match shape {
        CurveShape::Sine => "sine",
        CurveShape::Triangle => "triangle",
        CurveShape::Saw => "saw",
        CurveShape::ReverseSaw => "reverseSaw",
        CurveShape::Square => "square",
    }
}

fn curve_phase_mode_variant_id(mode: CurvePhaseMode) -> &'static str {
    match mode {
        CurvePhaseMode::Frequency => "frequency",
        CurvePhaseMode::NumPhases => "numPhases",
    }
}

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

fn key_secant_slope(start_position: f64, start_value: f64, end_position: f64, end_value: f64) -> Option<f64> {
    let span = end_position - start_position;
    if !span.is_finite() || span.abs() <= CURVE_RANGE_EPSILON {
        return None;
    }
    let slope = (end_value - start_value) / span;
    slope.is_finite().then_some(slope)
}

fn outgoing_segment_slope(start: &AnimationCurveKey, end: &AnimationCurveKey) -> Option<f64> {
    let secant = key_secant_slope(start.position, start.value, end.position, end.value);
    match &start.easing {
        CurveEasing::Bezier { out_handle, .. } => {
            let span = end.position - start.position;
            let delta_x = out_handle.position * span;
            if !delta_x.is_finite() || delta_x.abs() <= CURVE_RANGE_EPSILON {
                return secant;
            }
            let slope = out_handle.value / delta_x;
            if slope.is_finite() { Some(slope) } else { secant }
        }
        CurveEasing::Hold => Some(0.0),
        _ => secant,
    }
}

fn incoming_segment_slope(start: &AnimationCurveKey, end: &AnimationCurveKey) -> Option<f64> {
    let secant = key_secant_slope(start.position, start.value, end.position, end.value);
    match &start.easing {
        CurveEasing::Bezier { in_handle, .. } => {
            let span = end.position - start.position;
            let delta_x = in_handle.position * span;
            if !delta_x.is_finite() || delta_x.abs() <= CURVE_RANGE_EPSILON {
                return secant;
            }
            let slope = in_handle.value / delta_x;
            if slope.is_finite() { Some(slope) } else { secant }
        }
        CurveEasing::Hold => Some(0.0),
        _ => secant,
    }
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

    fn type_description(&self) -> Option<&str> {
        Some("Internal node storing editable X/Y curve range bounds.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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

    /// Inserts multiple keys using `(position, value)` tuples.
    ///
    /// Inserted keys default to linear easing so code-driven bulk inserts can stay lightweight.
    ///
    /// Returns created key node ids in insertion order.
    pub fn insert_keys(&mut self, ctx: &mut ProcessCtx, keys: Vec<(f64, f64)>) -> Vec<NodeId> {
        let keys_with_easing = keys.into_iter().map(|(position, value)| (position, value, CurveEasing::Linear)).collect();
        self.insert_keys_with_easing(ctx, keys_with_easing)
    }

    /// Inserts multiple keys with explicit per-key easing.
    ///
    /// Returns created key node ids in insertion order.
    pub fn insert_keys_with_easing(&mut self, ctx: &mut ProcessCtx, mut keys: Vec<(f64, f64, CurveEasing)>) -> Vec<NodeId> {
        if keys.is_empty() {
            return Vec::new();
        }

        keys.retain(|(position, value, _)| position.is_finite() && value.is_finite());
        if keys.is_empty() {
            return Vec::new();
        }
        keys.sort_by(|left, right| left.0.total_cmp(&right.0));

        let active_range = ctx.tree_snapshot().and_then(|snapshot| self.effective_range_constraint(snapshot));
        let child_range = self.initial_key_range_constraint();
        let mut created = Vec::<NodeId>::with_capacity(keys.len());
        for (position, value, easing) in keys {
            let (position, value) = if let Some(range) = active_range { (range.clamp_position(position), range.clamp_value(value)) } else { (position, value) };

            let key_node = AnimationCurveKeyNode::new_with_values_and_range_and_easing(position, value, child_range, easing);
            let key_id = key_node.id();
            self.add_child_boxed(ctx, Box::new(key_node), None);
            created.push(key_id);
        }
        created
    }

    /// Samples one value at `position` from the current curve snapshot.
    ///
    /// Returns `0` when curve data is unavailable or sampling fails.
    pub fn get_value_at(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> f64 {
        if !position.is_finite() {
            return 0.0;
        }
        curve_from_snapshot(snapshot, self.id()).and_then(|curve| curve.sample(position)).unwrap_or(0.0)
    }

    /// Returns sorted `(key_id, position, value)` tuples inside `range`.
    pub fn get_keys_between(&self, snapshot: &ProcessTreeSnapshot, range: RangeInclusive<f64>) -> Vec<(NodeId, f64, f64)> {
        let start = *range.start();
        let end = *range.end();
        if !start.is_finite() || !end.is_finite() {
            return Vec::new();
        }
        let (min_position, max_position) = if start <= end { (start, end) } else { (end, start) };
        self.collect_sorted_key_data(snapshot).into_iter().filter(|(_, position, _)| *position >= min_position && *position <= max_position).collect()
    }

    /// Returns the closest key to `position` as `(key_id, position, value)`.
    pub fn get_closest_key(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> Option<(NodeId, f64, f64)> {
        if !position.is_finite() {
            return None;
        }

        self.collect_sorted_key_data(snapshot).into_iter().min_by(|left, right| {
            let left_distance = (left.1 - position).abs();
            let right_distance = (right.1 - position).abs();
            left_distance.total_cmp(&right_distance).then(left.1.total_cmp(&right.1)).then(left.0.0.cmp(&right.0.0))
        })
    }

    /// Returns the closest strictly-previous key to `position`.
    pub fn get_prev_key(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> Option<(NodeId, f64, f64)> {
        if !position.is_finite() {
            return None;
        }

        let mut previous = None;
        for key in self.collect_sorted_key_data(snapshot) {
            if key.1 < position - KEY_ORDER_POSITION_EPSILON {
                previous = Some(key);
            } else {
                break;
            }
        }
        previous
    }

    /// Returns the closest strictly-next key to `position`.
    pub fn get_next_key(&self, snapshot: &ProcessTreeSnapshot, position: f64) -> Option<(NodeId, f64, f64)> {
        if !position.is_finite() {
            return None;
        }

        self.collect_sorted_key_data(snapshot).into_iter().find(|(_, key_position, _)| *key_position > position + KEY_ORDER_POSITION_EPSILON)
    }

    fn collect_sorted_key_records(&self, snapshot: &ProcessTreeSnapshot) -> Vec<(NodeId, AnimationCurveKey)> {
        let range_constraint = self.effective_range_constraint(snapshot).or_else(|| read_range_constraint_from_key_param_constraints(snapshot, self.id()));
        let mut key_entries = Vec::<(usize, NodeId, AnimationCurveKey)>::new();
        for (source_index, child) in snapshot.child_ids(self.id()).into_iter().enumerate() {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
                continue;
            }
            let Some(key) = parse_key_from_snapshot(snapshot, child, range_constraint) else {
                continue;
            };
            key_entries.push((source_index, child, key));
        }

        key_entries.sort_by(|left, right| {
            if (left.2.position - right.2.position).abs() <= KEY_ORDER_POSITION_EPSILON {
                left.0.cmp(&right.0)
            } else {
                left.2.position.total_cmp(&right.2.position)
            }
        });
        key_entries.into_iter().map(|(_, node_id, key)| (node_id, key)).collect()
    }

    fn queue_key_easing_replace(&self, snapshot: &ProcessTreeSnapshot, key_node: NodeId, easing: CurveEasing) -> Result<Edit, String> {
        let easing_node = snapshot.find_child(key_node, PARAMETER_ANIMATION_EASING_DECL_ID).ok_or_else(|| format!("missing easing node for animation-curve key {}", key_node.0))?;
        let label = snapshot.node(easing_node).map(|node| node.label.clone()).unwrap_or_else(|| "Easing".to_string());
        Ok(Edit::ReplaceNode {
            node: easing_node,
            new_node: Box::new(AnimationCurveEasingNode::new_with_easing(label, easing)),
        })
    }

    /// Replaces all keys inside the sampled x-range with a sparse bezier fit of `samples`.
    pub fn replace_range_with_fitted_samples(&mut self, ctx: &mut ProcessCtx, samples: &[AnimationCurveFitPoint], options: AnimationCurveBezierFitOptions) -> Result<Vec<NodeId>, String> {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return Err("animation curve fit requires an active tree snapshot".to_string());
        };

        let active_range = self.effective_range_constraint(snapshot).or_else(|| read_range_constraint_from_key_param_constraints(snapshot, self.id()));
        let normalized_samples = samples
            .iter()
            .copied()
            .map(|point| if let Some(range) = active_range { AnimationCurveFitPoint::new(range.clamp_position(point.position), range.clamp_value(point.value)) } else { point })
            .collect::<Vec<_>>();
        let mut fitted_keys = fit_points_to_bezier_keys(normalized_samples.as_slice(), options);
        if fitted_keys.len() < 2 {
            return Err("animation curve fit requires at least two unique sample positions".to_string());
        }

        let key_records = self.collect_sorted_key_records(snapshot);
        let range_start = fitted_keys.first().map(|key| key.position).unwrap_or(0.0);
        let range_end = fitted_keys.last().map(|key| key.position).unwrap_or(range_start);
        let keys_to_remove = key_records
            .iter()
            .filter(|(_, key)| key.position >= range_start - KEY_ORDER_POSITION_EPSILON && key.position <= range_end + KEY_ORDER_POSITION_EPSILON)
            .map(|(node_id, _)| *node_id)
            .collect::<Vec<_>>();
        let left_index = key_records.iter().rposition(|(_, key)| key.position < range_start - KEY_ORDER_POSITION_EPSILON);
        let right_index = key_records.iter().position(|(_, key)| key.position > range_end + KEY_ORDER_POSITION_EPSILON);

        let first_path_slope = outgoing_segment_slope(&fitted_keys[0], &fitted_keys[1]).unwrap_or_else(|| key_secant_slope(fitted_keys[0].position, fitted_keys[0].value, fitted_keys[1].position, fitted_keys[1].value).unwrap_or(0.0));
        let fitted_len = fitted_keys.len();
        let last_path_slope =
            incoming_segment_slope(&fitted_keys[fitted_len - 2], &fitted_keys[fitted_len - 1]).unwrap_or_else(|| key_secant_slope(fitted_keys[fitted_len - 2].position, fitted_keys[fitted_len - 2].value, fitted_keys[fitted_len - 1].position, fitted_keys[fitted_len - 1].value).unwrap_or(0.0));

        if let Some(right_index) = right_index {
            let (_, right_key) = &key_records[right_index];
            let right_boundary_slope = key_records
                .get(right_index + 1)
                .and_then(|(_, next_key)| outgoing_segment_slope(right_key, next_key))
                .or_else(|| key_secant_slope(fitted_keys[fitted_len - 1].position, fitted_keys[fitted_len - 1].value, right_key.position, right_key.value))
                .unwrap_or(0.0);
            if let Some(easing) = bezier_easing_from_endpoint_slopes(fitted_keys[fitted_len - 1].position, fitted_keys[fitted_len - 1].value, right_key.position, right_key.value, last_path_slope, right_boundary_slope) {
                fitted_keys[fitted_len - 1].easing = easing;
            }
        }

        let mut left_key_easing_replace = None;
        if let Some(left_index) = left_index {
            let (left_node_id, left_key) = &key_records[left_index];
            let left_boundary_slope = key_records
                .get(left_index + 1)
                .and_then(|(_, next_key)| outgoing_segment_slope(left_key, next_key))
                .or_else(|| key_secant_slope(left_key.position, left_key.value, fitted_keys[0].position, fitted_keys[0].value))
                .unwrap_or(0.0);
            if let Some(easing) = bezier_easing_from_endpoint_slopes(left_key.position, left_key.value, fitted_keys[0].position, fitted_keys[0].value, left_boundary_slope, first_path_slope) {
                left_key_easing_replace = Some(self.queue_key_easing_replace(snapshot, *left_node_id, easing)?);
            }
        }

        if let Some(edit) = left_key_easing_replace {
            ctx.edits.push(edit);
        }

        for node_id in keys_to_remove {
            ctx.edits.push(Edit::RemoveNode { node: node_id });
        }

        Ok(self.insert_keys_with_easing(ctx, fitted_keys.into_iter().map(|key| (key.position, key.value, key.easing)).collect()))
    }

    fn collect_sorted_key_data(&self, snapshot: &ProcessTreeSnapshot) -> Vec<(NodeId, f64, f64)> {
        let range_constraint = self.effective_range_constraint(snapshot).or_else(|| read_range_constraint_from_key_param_constraints(snapshot, self.id()));
        let mut key_entries = Vec::<(usize, NodeId, f64, f64)>::new();
        for (source_index, child) in snapshot.child_ids(self.id()).into_iter().enumerate() {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type != PARAMETER_ANIMATION_KEY_NODE_TYPE {
                continue;
            }

            let mut position = read_child_param_f64(snapshot, child, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID, 0.0);
            let mut value = read_child_param_f64(snapshot, child, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, 0.0);
            if let Some(range_constraint) = range_constraint {
                position = range_constraint.clamp_position(position);
                value = range_constraint.clamp_value(value);
            }
            key_entries.push((source_index, child, position, value));
        }

        key_entries.sort_by(|left, right| if (left.2 - right.2).abs() <= KEY_ORDER_POSITION_EPSILON { left.0.cmp(&right.0) } else { left.2.total_cmp(&right.2) });
        key_entries.into_iter().map(|(_, key_id, position, value)| (key_id, position, value)).collect()
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
        sorted_key_entries.sort_by(|left, right| if (left.2 - right.2).abs() <= KEY_ORDER_POSITION_EPSILON { left.0.cmp(&right.0) } else { left.2.total_cmp(&right.2) });

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

    fn type_description(&self) -> Option<&str> {
        Some("Internal node hosting one animation-curve key list.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
            let reorder_moves = if should_reorder_keys { self.collect_key_reorder_moves_by_position(snapshot) } else { Vec::new() };
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
    default_easing: CurveEasing,
}

impl AnimationCurveKeyNode {
    /// Creates one key node with default position/value set to `0`.
    pub fn new() -> Self {
        Self::new_with_label_and_values_and_range_and_easing("Key", 0.0, 0.0, None, default_curve_easing())
    }

    /// Creates one key node with custom label and default position/value.
    pub fn new_with_label(label: impl Into<String>) -> Self {
        Self::new_with_label_and_values_and_range_and_easing(label, 0.0, 0.0, None, default_curve_easing())
    }

    /// Creates one key node with custom label and optional range constraint.
    pub fn new_with_label_and_range(label: impl Into<String>, range_constraint: Option<AnimationCurveRangeConstraint>) -> Self {
        Self::new_with_label_and_values_and_range_and_easing(label, 0.0, 0.0, range_constraint, default_curve_easing())
    }

    /// Creates one key node with explicit initial position/value.
    pub fn new_with_values(position: f64, value: f64) -> Self {
        Self::new_with_label_and_values_and_range_and_easing("Key", position, value, None, default_curve_easing())
    }

    /// Creates one key node with explicit initial position/value and optional range constraint.
    pub fn new_with_values_and_range(position: f64, value: f64, range_constraint: Option<AnimationCurveRangeConstraint>) -> Self {
        Self::new_with_label_and_values_and_range_and_easing("Key", position, value, range_constraint, default_curve_easing())
    }

    /// Creates one key node with explicit initial position/value, optional range, and explicit easing.
    pub fn new_with_values_and_range_and_easing(position: f64, value: f64, range_constraint: Option<AnimationCurveRangeConstraint>, easing: CurveEasing) -> Self {
        Self::new_with_label_and_values_and_range_and_easing("Key", position, value, range_constraint, easing)
    }

    fn new_with_label_and_values_and_range_and_easing(label: impl Into<String>, position: f64, value: f64, range_constraint: Option<AnimationCurveRangeConstraint>, default_easing: CurveEasing) -> Self {
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
            default_easing,
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

    fn type_description(&self) -> Option<&str> {
        Some("Internal node representing one animation curve key.")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
            ctx.add_child_boxed(self.id(), Box::new(AnimationCurveEasingNode::new_with_easing("Easing", self.default_easing.clone())), None);
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

/// Internal node storing one key-to-next easing specification.
#[allow(missing_docs)]
#[crate::node("animation_curve_easing")]
#[children(
    kind: crate::parameter::Enum = "bezier" (
        label = "Kind",
        enum_options = ["linear", "bezier", "hold", "steps", "shape", "perlinNoise", "random", "script"],
    );
    out_position: f64 = 1.0 / 3.0 (
        label = "Out Handle Position",
        dependency = kind == "bezier",
    );
    out_value: f64 = 0.0 (
        label = "Out Handle Value",
        dependency = kind == "bezier",
    );
    in_position: f64 = -1.0 / 3.0 (
        label = "In Handle Position",
        dependency = kind == "bezier",
    );
    in_value: f64 = 0.0 (
        label = "In Handle Value",
        dependency = kind == "bezier",
    );
    step_mode: crate::parameter::Enum = "numSteps" (
        label = "Step Mode",
        enum_options = ["stepSize", "numSteps"],
        dependency = kind == "steps",
    );
    step_size: f64 = 0.1 [0.0..] (
        label = "Step Size",
        dependency = kind == "steps" && step_mode == "stepSize",
    );
    num_steps: i32 = 8 [1..] (
        label = "Number of Steps",
        dependency = kind == "steps" && step_mode == "numSteps",
    );
    shape: crate::parameter::Enum = "sine" (
        label = "Shape",
        enum_options = ["sine", "triangle", "saw", "reverseSaw", "square"],
        dependency = kind == "shape",
    );
    amplitude: f64 = 1.0 (
        label = "Amplitude",
        dependency = kind == "shape" || kind == "perlinNoise",
    );
    phase_mode: crate::parameter::Enum = "frequency" (
        label = "Phase Mode",
        enum_options = ["frequency", "numPhases"],
        dependency = kind == "shape",
    );
    frequency: f64 = 1.0 [0.0..] (
        label = "Frequency",
        dependency = kind == "shape" || kind == "perlinNoise" || kind == "random",
    );
    num_phases: f64 = 1.0 [0.0..] (
        label = "Number of Phases",
        dependency = kind == "shape" && phase_mode == "numPhases",
    );
    fade_in: f64 = 0.0 [0.0..] (
        label = "Fade In",
        dependency = kind == "shape" || kind == "perlinNoise" || kind == "random",
    );
    fade_out: f64 = 0.0 [0.0..] (
        label = "Fade Out",
        dependency = kind == "shape" || kind == "perlinNoise" || kind == "random",
    );
    octaves: i32 = 4 [1..] (
        label = "Octaves",
        dependency = kind == "perlinNoise",
    );
    phase: f64 = 0.0 (
        label = "Phase",
        dependency = kind == "perlinNoise",
    );
    seed: i32 = 0 (
        label = "Seed",
        dependency = kind == "random",
    );
    script_source: String = "".to_string() (
        label = "Script Source",
        dependency = kind == "script",
    );
)]
pub struct AnimationCurveEasingNode {}

impl AnimationCurveEasingNode {
    /// Creates one easing node with explicit default easing values.
    pub fn new_with_easing(label: impl Into<String>, default_easing: CurveEasing) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_EASING_DECL_ID.to_string());

        let (kind, out_position, out_value, in_position, in_value, step_mode, step_size, num_steps, shape, amplitude, phase_mode, frequency, num_phases, fade_in, fade_out, octaves, phase, seed, script_source) = Self::defaults_from_easing(&default_easing);

        Self {
            node_data,
            kind: crate::node::ParameterHandle::new(kind.into()),
            out_position: crate::node::ParameterHandle::new(out_position),
            out_value: crate::node::ParameterHandle::new(out_value),
            in_position: crate::node::ParameterHandle::new(in_position),
            in_value: crate::node::ParameterHandle::new(in_value),
            step_mode: crate::node::ParameterHandle::new(step_mode.into()),
            step_size: crate::node::ParameterHandle::new(step_size),
            num_steps: crate::node::ParameterHandle::new(num_steps),
            shape: crate::node::ParameterHandle::new(shape.into()),
            amplitude: crate::node::ParameterHandle::new(amplitude),
            phase_mode: crate::node::ParameterHandle::new(phase_mode.into()),
            frequency: crate::node::ParameterHandle::new(frequency),
            num_phases: crate::node::ParameterHandle::new(num_phases),
            fade_in: crate::node::ParameterHandle::new(fade_in),
            fade_out: crate::node::ParameterHandle::new(fade_out),
            octaves: crate::node::ParameterHandle::new(octaves),
            phase: crate::node::ParameterHandle::new(phase),
            seed: crate::node::ParameterHandle::new(seed),
            script_source: crate::node::ParameterHandle::new(script_source),
        }
    }

    fn defaults_from_easing(easing: &CurveEasing) -> (&'static str, f64, f64, f64, f64, &'static str, f64, i32, &'static str, f64, &'static str, f64, f64, f64, f64, i32, f64, i32, String) {
        let kind = curve_easing_kind_id(easing);
        let out_position = match easing {
            CurveEasing::Bezier { out_handle, .. } => out_handle.position,
            _ => 1.0 / 3.0,
        };
        let out_value = match easing {
            CurveEasing::Bezier { out_handle, .. } => out_handle.value,
            _ => 0.0,
        };
        let in_position = match easing {
            CurveEasing::Bezier { in_handle, .. } => in_handle.position,
            _ => -1.0 / 3.0,
        };
        let in_value = match easing {
            CurveEasing::Bezier { in_handle, .. } => in_handle.value,
            _ => 0.0,
        };
        let step_mode = match easing {
            CurveEasing::Steps { step_mode, .. } => curve_step_mode_variant_id(*step_mode),
            _ => "numSteps",
        };
        let step_size = match easing {
            CurveEasing::Steps { step_size, .. } => *step_size,
            _ => 0.1,
        };
        let num_steps = match easing {
            CurveEasing::Steps { num_steps, .. } => (*num_steps).max(1) as i32,
            _ => 8,
        };
        let shape = match easing {
            CurveEasing::Shape { shape, .. } => curve_shape_variant_id(*shape),
            _ => "sine",
        };
        let amplitude = match easing {
            CurveEasing::Shape { amplitude, .. } | CurveEasing::PerlinNoise { amplitude, .. } => *amplitude,
            _ => 1.0,
        };
        let phase_mode = match easing {
            CurveEasing::Shape { phase_mode, .. } => curve_phase_mode_variant_id(*phase_mode),
            _ => "frequency",
        };
        let frequency = match easing {
            CurveEasing::Shape { frequency, .. } | CurveEasing::PerlinNoise { frequency, .. } | CurveEasing::Random { frequency, .. } => *frequency,
            _ => 1.0,
        };
        let num_phases = match easing {
            CurveEasing::Shape { num_phases, .. } => *num_phases,
            _ => 1.0,
        };
        let fade_in = match easing {
            CurveEasing::Shape { fade_in, .. } | CurveEasing::PerlinNoise { fade_in, .. } | CurveEasing::Random { fade_in, .. } => *fade_in,
            _ => 0.0,
        };
        let fade_out = match easing {
            CurveEasing::Shape { fade_out, .. } | CurveEasing::PerlinNoise { fade_out, .. } | CurveEasing::Random { fade_out, .. } => *fade_out,
            _ => 0.0,
        };
        let octaves = match easing {
            CurveEasing::PerlinNoise { octaves, .. } => (*octaves).max(1) as i32,
            _ => 4,
        };
        let phase = match easing {
            CurveEasing::PerlinNoise { phase, .. } => *phase,
            _ => 0.0,
        };
        let seed = match easing {
            CurveEasing::Random { seed, .. } => (*seed).min(i32::MAX as u64) as i32,
            _ => 0,
        };
        let script_source = match easing {
            CurveEasing::Script { source } => source.clone(),
            _ => String::new(),
        };

        (
            kind,
            out_position,
            out_value,
            in_position,
            in_value,
            step_mode,
            step_size,
            num_steps,
            shape,
            amplitude,
            phase_mode,
            frequency,
            num_phases,
            fade_in,
            fade_out,
            octaves,
            phase,
            seed,
            script_source,
        )
    }
}

#[crate::node("animation_curve_easing", from_struct)]
impl Node for AnimationCurveEasingNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.can_be_disabled = false;
        self.node_data_mut().meta.decl_id = DeclId(PARAMETER_ANIMATION_EASING_DECL_ID.to_string());
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
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
    use crate::define_node_enum;
    use crate::edit::Edit;
    use crate::engine::Engine;
    use crate::node::{Folder, Node, NodeId};
    use crate::parameter::ParameterEventBehaviour;
    use crate::process_ctx::ExecutionPhase;

    use super::*;

    define_node_enum!(
        enum AnimationCurveTestNode {}
    );

    fn first_child<T: Node>(engine: &Engine<T>, parent: NodeId) -> NodeId {
        engine.nodes.get(parent).and_then(|node| node.node_data().first_child).expect("parent should have one child")
    }

    fn direct_child_decl_ids<T: Node>(engine: &Engine<T>, parent: NodeId) -> Vec<String> {
        let mut decl_ids = Vec::new();
        let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let child_node = engine.nodes.get(child_id).expect("child should exist");
            decl_ids.push(child_node.node_data().meta.decl_id.0.clone());
            child = child_node.node_data().next_sibling;
        }
        decl_ids
    }

    fn find_direct_child_by_decl<T: Node>(engine: &Engine<T>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
        let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let child_node = engine.nodes.get(child_id)?;
            if child_node.node_data().meta.decl_id.0 == decl_id {
                return Some(child_id);
            }
            child = child_node.node_data().next_sibling;
        }
        None
    }

    fn stabilize_dependency_updates<T: Node>(engine: &mut Engine<T>, reason: &str) {
        for _ in 0..3 {
            engine.apply_edits().expect(reason);
            engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dependency stabilization dispatch should succeed");
        }
    }

    #[test]
    fn parse_helpers_map_variants() {
        assert_eq!(parse_step_mode("stepSize"), CurveStepMode::StepSize);
        assert_eq!(parse_shape("reverseSaw"), CurveShape::ReverseSaw);
        assert_eq!(parse_phase_mode("numPhases"), CurvePhaseMode::NumPhases);
    }

    #[test]
    fn easing_node_dependencies_follow_kind_and_mode() {
        let root: AnimationCurveTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        engine.add_node(
            AnimationCurveEasingNode::new_with_easing(
                "Ease",
                CurveEasing::Steps {
                    step_mode: CurveStepMode::StepSize,
                    step_size: 0.25,
                    num_steps: 9,
                },
            )
            .into(),
            None,
        );
        stabilize_dependency_updates(&mut engine, "easing node creation should apply");

        let easing = first_child(&engine, engine.root);
        let direct_children_after_create = direct_child_decl_ids(&engine, easing);
        let step_mode = find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID).unwrap_or_else(|| panic!("step mode should exist; children were {:?}", direct_children_after_create));
        let kind = find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_KIND_DECL_ID).expect("kind should exist");

        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID).is_none(), "non-bezier easings should hide the out handle position");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID).is_none(), "non-bezier easings should hide the out handle value");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID).is_none(), "non-bezier easings should hide the in handle position");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID).is_none(), "non-bezier easings should hide the in handle value");

        assert_eq!(
            direct_child_decl_ids(&engine, easing),
            vec![PARAMETER_ANIMATION_EASING_KIND_DECL_ID.to_string(), PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID.to_string(), PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID.to_string(),],
            "step-size easings should materialize only the active step parameter",
        );

        engine.edits.push(Edit::SetParam {
            node: step_mode,
            value: ParamValue::Enum("numSteps".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        stabilize_dependency_updates(&mut engine, "switching step mode should apply");

        assert_eq!(
            direct_child_decl_ids(&engine, easing),
            vec![PARAMETER_ANIMATION_EASING_KIND_DECL_ID.to_string(), PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID.to_string(), PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID.to_string(),],
            "switching step mode should swap the dependent step parameter in place",
        );

        engine.edits.push(Edit::SetParam {
            node: kind,
            value: ParamValue::Enum("shape".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        stabilize_dependency_updates(&mut engine, "switching easing kind should apply");

        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID).is_none(), "shape easings should hide the out handle position");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID).is_none(), "shape easings should hide the out handle value");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID).is_none(), "shape easings should hide the in handle position");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID).is_none(), "shape easings should hide the in handle value");

        assert_eq!(
            direct_child_decl_ids(&engine, easing),
            vec![
                PARAMETER_ANIMATION_EASING_KIND_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID.to_string(),
            ],
            "shape easings should expose only the active shape-specific parameters",
        );

        engine.edits.push(Edit::SetParam {
            node: kind,
            value: ParamValue::Enum("bezier".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        stabilize_dependency_updates(&mut engine, "switching easing kind to bezier should apply");

        assert_eq!(
            direct_child_decl_ids(&engine, easing),
            vec![
                PARAMETER_ANIMATION_EASING_KIND_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID.to_string(),
                PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID.to_string(),
            ],
            "bezier easings should expose only the bezier handle parameters",
        );
    }

    #[test]
    fn easing_node_preserves_script_source_default() {
        let root: AnimationCurveTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);
        let expected_source = "return t * 0.5;".to_string();

        engine.add_node(AnimationCurveEasingNode::new_with_easing("Ease", CurveEasing::Script { source: expected_source.clone() }).into(), None);
        stabilize_dependency_updates(&mut engine, "script easing node creation should apply");

        let easing = first_child(&engine, engine.root);
        let direct_children_after_create = direct_child_decl_ids(&engine, easing);
        let script_source = find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID).unwrap_or_else(|| panic!("script easings should materialize script source; children were {:?}", direct_children_after_create));
        let script_source_snapshot = engine.nodes.get(script_source).and_then(Node::engine_param_snapshot).expect("script source should expose a parameter snapshot");

        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID).is_none(), "script easings should hide the out handle position");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID).is_none(), "script easings should hide the out handle value");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID).is_none(), "script easings should hide the in handle position");
        assert!(find_direct_child_by_decl(&engine, easing, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID).is_none(), "script easings should hide the in handle value");

        assert_eq!(script_source_snapshot.value, ParamValue::Str(expected_source));
    }
}
