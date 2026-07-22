use super::easing::parse_easing_from_snapshot;
use super::helpers::read_child_param_f64;
use super::prelude::*;
use super::range::{
    CurveRangeConstraint, read_range_constraint_from_key_param_constraints, read_range_constraint_from_range_node,
};

pub(super) fn parse_key_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    key_node: NodeId,
    range_constraint: Option<CurveRangeConstraint>,
) -> Option<CurveKey> {
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
    let easing = snapshot
        .find_child(key_node, PARAMETER_ANIMATION_EASING_DECL_ID)
        .map(|easing_node| parse_easing_from_snapshot(snapshot, easing_node))
        .unwrap_or(CurveEasing::Linear);

    Some(CurveKey::new(position, value, easing))
}

/// Builds a [`Curve`] from one curve-node subtree in a processing snapshot.
pub fn curve_from_snapshot(snapshot: &ProcessTreeSnapshot, curve_node: NodeId) -> Option<Curve> {
    let curve_snapshot = snapshot.node(curve_node)?;
    if curve_snapshot.node_type != PARAMETER_ANIMATION_CURVE_NODE_TYPE {
        return None;
    }

    let range_constraint = if let Some(range_node) = snapshot.find_child(curve_node, PARAMETER_ANIMATION_RANGE_DECL_ID)
    {
        read_range_constraint_from_range_node(snapshot, range_node)
    } else {
        read_range_constraint_from_key_param_constraints(snapshot, curve_node)
    };

    let keys = snapshot
        .child_ids(curve_node)
        .into_iter()
        .filter_map(|child_id| parse_key_from_snapshot(snapshot, child_id, range_constraint))
        .collect::<Vec<_>>();

    let mut curve = Curve::new(keys);
    if let Some(range_constraint) = range_constraint {
        curve.set_value_range_constraint(Some(range_constraint.y_min), Some(range_constraint.y_max));
    }
    Some(curve)
}
