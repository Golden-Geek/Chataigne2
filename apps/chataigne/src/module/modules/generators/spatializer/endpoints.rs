use super::*;

pub(super) fn spatializer_endpoint_config(
    item: &ProcessTreeNodeSnapshot,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    dimension: SpatializerDimension,
    value_prefix: &str,
) -> Option<SpatializerEndpointConfig> {
    Some(SpatializerEndpointConfig {
        item_id,
        item_uuid: item.uuid,
        value_decl_id: value_decl_id(value_prefix, item.uuid),
        label: item.label.clone(),
        enabled: item.enabled,
        position: child_position(snapshot, item_id, dimension, SpatialPoint::new(0.0, 0.0, 0.0)),
        radius: child_float(snapshot, item_id, RADIUS_DECL_ID, 1.0).max(0.0),
        freeze_radius: child_float(snapshot, item_id, FREEZE_RADIUS_DECL_ID, 0.0).max(0.0),
    })
}

pub(super) fn sync_position_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    dimension: SpatializerDimension,
) -> bool {
    let desired_decl = dimension.position_decl_id();
    let stale_decl = dimension.stale_position_decl_id();
    let desired_ids = direct_child_ids_by_decl(snapshot, item_id, desired_decl);
    let stale_ids = direct_child_ids_by_decl(snapshot, item_id, stale_decl);
    let desired_id = desired_ids.first().copied();
    let stale_id = stale_ids.first().copied();
    let position = desired_id
        .and_then(|node_id| child_param_value(snapshot, node_id))
        .and_then(param_value_to_point)
        .or_else(|| {
            stale_id
                .and_then(|node_id| child_param_value(snapshot, node_id))
                .and_then(param_value_to_point)
        })
        .unwrap_or_else(|| SpatialPoint::new(0.0, 0.0, 0.0));

    let mut changed = false;
    for stale_id in stale_ids {
        NodeHandle::new(stale_id).remove(ctx);
        changed = true;
    }
    for duplicate_id in desired_ids.into_iter().skip(1) {
        NodeHandle::new(duplicate_id).remove(ctx);
        changed = true;
    }

    match desired_id {
        Some(node_id) => {
            let needs_replace = snapshot
                .node(node_id)
                .and_then(|node| node.param_value.as_ref())
                .is_none_or(|value| !position_value_matches_dimension(value, dimension));
            if needs_replace {
                let mut replacement = position_param(dimension, position);
                replacement.node_data_mut().meta.uuid = snapshot.node(node_id).unwrap().uuid;
                NodeHandle::new(node_id).replace_with(ctx, replacement);
                changed = true;
            }
        }
        None => {
            ctx.add_child_tree(item_id, NodeTree::new(position_param(dimension, position)), None);
            changed = true;
        }
    }

    changed
}

pub(super) fn sync_radius_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    required: bool,
) -> bool {
    sync_float_child(
        ctx,
        snapshot,
        item_id,
        required,
        FloatChildSpec {
            decl_id: RADIUS_DECL_ID,
            default_value: 1.0,
            parameter: radius_param,
        },
    )
}

pub(super) fn sync_freeze_radius_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    required: bool,
) -> bool {
    sync_float_child(
        ctx,
        snapshot,
        item_id,
        required,
        FloatChildSpec {
            decl_id: FREEZE_RADIUS_DECL_ID,
            default_value: 0.0,
            parameter: freeze_radius_param,
        },
    )
}

pub(super) fn sync_float_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    required: bool,
    spec: FloatChildSpec,
) -> bool {
    let existing_ids = direct_child_ids_by_decl(snapshot, item_id, spec.decl_id);
    let existing_id = existing_ids.first().copied();
    let mut changed = false;

    for duplicate_id in existing_ids.into_iter().skip(1) {
        NodeHandle::new(duplicate_id).remove(ctx);
        changed = true;
    }

    if !required {
        if let Some(existing_id) = existing_id {
            NodeHandle::new(existing_id).remove(ctx);
            changed = true;
        }
        return changed;
    }

    let radius = existing_id
        .and_then(|node_id| child_param_value(snapshot, node_id))
        .and_then(|value| value.as_float())
        .filter(|value| value.is_finite())
        .unwrap_or(spec.default_value)
        .max(0.0);

    match existing_id {
        Some(node_id) => {
            let needs_replace = snapshot
                .node(node_id)
                .and_then(|node| node.param_value.as_ref())
                .is_none_or(|value| !matches!(value, ParamValue::Float(_)));
            if needs_replace {
                let mut replacement = (spec.parameter)(radius);
                replacement.node_data_mut().meta.uuid = snapshot.node(node_id).unwrap().uuid;
                NodeHandle::new(node_id).replace_with(ctx, replacement);
                true
            } else {
                changed
            }
        }
        None => {
            ctx.add_child_tree(item_id, NodeTree::new((spec.parameter)(radius)), None);
            true
        }
    }
}
