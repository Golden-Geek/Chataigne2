use super::helpers::{read_child_param_color, read_child_param_enum, read_child_param_f64};
use super::prelude::*;

pub(super) fn parse_stop_from_snapshot(snapshot: &ProcessTreeSnapshot, stop_node: NodeId) -> Option<GradientStop> {
    let stop = snapshot.node(stop_node)?;
    if stop.node_type != GRADIENT_STOP_NODE_TYPE {
        return None;
    }

    let position = read_child_param_f64(snapshot, stop_node, GRADIENT_STOP_POSITION_DECL_ID, 0.0).clamp(0.0, 1.0);
    let color = read_child_param_color(
        snapshot,
        stop_node,
        GRADIENT_STOP_COLOR_DECL_ID,
        Color::new(0.0, 0.0, 0.0, 1.0),
    );
    let interpolation = GradientInterpolation::from_variant_id(&read_child_param_enum(
        snapshot,
        stop_node,
        GRADIENT_STOP_INTERPOLATION_DECL_ID,
        GradientInterpolation::Linear.variant_id(),
    ));

    Some(GradientStop::new(position, color, interpolation))
}

/// Builds a [`Gradient`] from one gradient-node subtree in a processing snapshot.
pub fn gradient_from_snapshot(snapshot: &ProcessTreeSnapshot, gradient_node: NodeId) -> Option<Gradient> {
    let gradient_snapshot = snapshot.node(gradient_node)?;
    if gradient_snapshot.node_type != GRADIENT_NODE_TYPE {
        return None;
    }

    let stops = snapshot
        .child_ids(gradient_node)
        .into_iter()
        .filter_map(|child_id| parse_stop_from_snapshot(snapshot, child_id))
        .collect::<Vec<_>>();

    Some(Gradient::new(stops))
}
