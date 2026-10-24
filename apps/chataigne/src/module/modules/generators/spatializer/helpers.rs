use super::*;

pub(super) fn direct_child_ids_by_decl(
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    decl_id: &str,
) -> Vec<NodeId> {
    snapshot
        .child_ids_slice(parent_id)
        .iter()
        .copied()
        .filter(|child_id| snapshot.node(*child_id).is_some_and(|child| child.decl_id == decl_id))
        .collect()
}

pub(super) fn direct_child_ids_by_decl_tail(
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    decl_tail: &str,
) -> Vec<NodeId> {
    snapshot
        .child_ids_slice(parent_id)
        .iter()
        .copied()
        .filter(|child_id| {
            snapshot
                .node(*child_id)
                .is_some_and(|child| child.decl_id == decl_tail || child.decl_id.rsplit('/').next() == Some(decl_tail))
        })
        .collect()
}

pub(super) fn value_decl_id(prefix: &str, item_uuid: NodeUuid) -> String {
    format!("{}_{}", prefix, item_uuid.0.simple())
}

pub(super) fn target_folder_uuid(target_uuid: NodeUuid) -> NodeUuid {
    const TARGET_FOLDER_UUID_MASK: u128 = 0x7370_6174_6961_6c69_7a65_725f_7467_0000;
    NodeUuid(uuid::Uuid::from_u128(target_uuid.0.as_u128() ^ TARGET_FOLDER_UUID_MASK))
}

pub(super) fn source_folder_uuid(source_uuid: NodeUuid) -> NodeUuid {
    const SOURCE_FOLDER_UUID_MASK: u128 = 0x7370_6174_6961_6c69_7a65_725f_7366_0000;
    NodeUuid(uuid::Uuid::from_u128(source_uuid.0.as_u128() ^ SOURCE_FOLDER_UUID_MASK))
}

pub(super) fn source_value_uuid(target_uuid: NodeUuid, source_uuid: NodeUuid) -> NodeUuid {
    const SOURCE_VALUE_UUID_MASK: u128 = 0x7370_6174_6961_6c69_7a65_725f_7372_0000;
    NodeUuid(uuid::Uuid::from_u128(
        target_uuid.0.as_u128().rotate_left(1) ^ source_uuid.0.as_u128().rotate_right(1) ^ SOURCE_VALUE_UUID_MASK,
    ))
}

pub(super) fn target_value_uuid(source_uuid: NodeUuid, target_uuid: NodeUuid) -> NodeUuid {
    const TARGET_VALUE_UUID_MASK: u128 = 0x7370_6174_6961_6c69_7a65_725f_7476_0000;
    NodeUuid(uuid::Uuid::from_u128(
        source_uuid.0.as_u128().rotate_left(1) ^ target_uuid.0.as_u128().rotate_right(1) ^ TARGET_VALUE_UUID_MASK,
    ))
}

pub(super) fn child_param_value(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> Option<&ParamValue> {
    snapshot.node(node_id).and_then(|node| node.param_value.as_ref())
}

pub(super) fn child_value<'a>(snapshot: &'a ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<&'a ParamValue> {
    snapshot
        .find_child(parent, key)
        .and_then(|node_id| snapshot.node(node_id))
        .and_then(|node| node.param_value.as_ref())
}

pub(super) fn child_float(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str, default: f64) -> f64 {
    child_value(snapshot, parent, key)
        .and_then(|value| value.as_float())
        .filter(|value| value.is_finite())
        .unwrap_or(default)
}

pub(super) fn child_position(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    dimension: SpatializerDimension,
    default: SpatialPoint,
) -> SpatialPoint {
    child_value(snapshot, parent, dimension.position_decl_id())
        .and_then(param_value_to_point)
        .unwrap_or(default)
}

pub(super) fn param_value_to_point(value: &ParamValue) -> Option<SpatialPoint> {
    match value {
        ParamValue::Vec2(x, y) if x.is_finite() && y.is_finite() => Some(SpatialPoint::new(*x, *y, 0.0)),
        ParamValue::Vec3(x, y, z) if x.is_finite() && y.is_finite() && z.is_finite() => {
            Some(SpatialPoint::new(*x, *y, *z))
        }
        _ => None,
    }
}

pub(super) fn position_value_matches_dimension(value: &ParamValue, dimension: SpatializerDimension) -> bool {
    matches!(
        (dimension, value),
        (SpatializerDimension::Two, ParamValue::Vec2(_, _)) | (SpatializerDimension::Three, ParamValue::Vec3(_, _, _))
    )
}
