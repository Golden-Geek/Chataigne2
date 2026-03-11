use super::helpers::{CURVE_RANGE_EPSILON, clamp_f64, make_vec2_parameter, read_child_param_value};
use super::prelude::*;

/// Axis-aligned curve bounds used to constrain key positions, key values, and sampled output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveRangeConstraint {
    /// Minimum key position.
    pub x_min: f64,
    /// Maximum key position.
    pub x_max: f64,
    /// Minimum key/sample value.
    pub y_min: f64,
    /// Maximum key/sample value.
    pub y_max: f64,
}

impl CurveRangeConstraint {
    /// Default editable range.
    pub const DEFAULT: Self = Self {
        x_min: 0.0,
        x_max: 1.0,
        y_min: 0.0,
        y_max: 1.0,
    };

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

        Some(Self {
            x_min,
            x_max,
            y_min,
            y_max,
        })
    }

    pub(super) fn clamp_position(self, value: f64) -> f64 {
        clamp_f64(value, self.x_min, self.x_max)
    }

    pub(super) fn clamp_value(self, value: f64) -> f64 {
        clamp_f64(value, self.y_min, self.y_max)
    }
}

pub(super) fn read_range_constraint_from_range_node(
    snapshot: &ProcessTreeSnapshot,
    range_node: NodeId,
) -> Option<CurveRangeConstraint> {
    let node = snapshot.node(range_node)?;
    if node.node_type != PARAMETER_ANIMATION_RANGE_NODE_TYPE || !node.enabled {
        return None;
    }

    let x_bounds = read_child_param_value(snapshot, range_node, PARAMETER_ANIMATION_RANGE_X_DECL_ID)
        .and_then(ParamValue::as_vec2)?;
    let y_bounds = read_child_param_value(snapshot, range_node, PARAMETER_ANIMATION_RANGE_Y_DECL_ID)
        .and_then(ParamValue::as_vec2)?;
    CurveRangeConstraint::new(x_bounds.0, x_bounds.1, y_bounds.0, y_bounds.1)
}

pub(super) fn read_range_axis_bounds(
    snapshot: &ProcessTreeSnapshot,
    range_node: NodeId,
    decl_id: &str,
) -> Option<(f64, f64)> {
    let bounds = read_child_param_value(snapshot, range_node, decl_id).and_then(ParamValue::as_vec2)?;
    Some((bounds.0, bounds.1))
}

pub(super) fn read_uniform_constraint_bounds(range: Option<&RangeConstraint>) -> Option<(f64, f64)> {
    match range {
        Some(RangeConstraint::Uniform {
            min: Some(min),
            max: Some(max),
        }) if min.is_finite() && max.is_finite() => {
            if min <= max {
                Some((*min, *max))
            } else {
                Some((*max, *min))
            }
        }
        _ => None,
    }
}

pub(super) fn read_range_constraint_from_key_param_constraints(
    snapshot: &ProcessTreeSnapshot,
    curve_node: NodeId,
) -> Option<CurveRangeConstraint> {
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
                x_bounds = read_uniform_constraint_bounds(
                    snapshot
                        .node(position_param)
                        .and_then(|entry| entry.param_constraints.as_ref())
                        .and_then(|constraints| constraints.range.as_ref()),
                );
            }
        }

        if y_bounds.is_none() {
            if let Some(value_param) = snapshot.find_child(child_id, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID) {
                y_bounds = read_uniform_constraint_bounds(
                    snapshot
                        .node(value_param)
                        .and_then(|entry| entry.param_constraints.as_ref())
                        .and_then(|constraints| constraints.range.as_ref()),
                );
            }
        }

        if x_bounds.is_some() && y_bounds.is_some() {
            break;
        }
    }

    let (x_min, x_max) = x_bounds?;
    let (y_min, y_max) = y_bounds?;
    CurveRangeConstraint::new(x_min, x_max, y_min, y_max)
}

/// Internal node storing editable X/Y curve range bounds.
pub struct CurveRangeNode {
    node_data: NodeData,
    default_range: CurveRangeConstraint,
}

impl CurveRangeNode {
    /// Creates one range node with optional initial bounds and enable state.
    pub fn new(initial_range: Option<CurveRangeConstraint>, enabled: bool) -> Self {
        let mut node_data = NodeData::new("Range".to_string());
        node_data.meta.can_be_disabled = true;
        node_data.meta.enabled = enabled;
        node_data.meta.decl_id = DeclId(PARAMETER_ANIMATION_RANGE_DECL_ID.to_string());
        Self {
            node_data,
            default_range: initial_range.unwrap_or(CurveRangeConstraint::DEFAULT),
        }
    }
}

impl Node for CurveRangeNode {
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
            ctx.add_child_boxed(
                self.id(),
                Box::new(make_vec2_parameter(
                    "X",
                    PARAMETER_ANIMATION_RANGE_X_DECL_ID,
                    self.default_range.x_min,
                    self.default_range.x_max,
                )),
                None,
            );
        }
        if !parameter_child_exists(ctx, self.id(), PARAMETER_ANIMATION_RANGE_Y_DECL_ID) {
            ctx.add_child_boxed(
                self.id(),
                Box::new(make_vec2_parameter(
                    "Y",
                    PARAMETER_ANIMATION_RANGE_Y_DECL_ID,
                    self.default_range.y_min,
                    self.default_range.y_max,
                )),
                None,
            );
        }
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}
