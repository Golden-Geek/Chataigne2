use super::*;

pub(super) fn unresolved_warning(
    node: NodeId,
    field: &str,
    message: impl Into<String>,
) -> RuntimeWarning {
    RuntimeWarning {
        node,
        id: format!("sound-card-unresolved-{field}"),
        message: message.into(),
        detail: None,
    }
}

pub(super) fn configuration_error(
    node: NodeId,
    message: impl Into<String>,
) -> ConfigurationError {
    ConfigurationError {
        node,
        message: message.into(),
    }
}

pub(super) fn required_path(
    snapshot: &ProcessTreeSnapshot,
    start: NodeId,
    path: &str,
) -> Result<NodeId, ConfigurationError> {
    find_path(snapshot, start, path)
        .ok_or_else(|| configuration_error(start, format!("missing Sound Card path '{path}'")))
}

pub(super) fn typed_children(
    snapshot: &ProcessTreeSnapshot,
    parent: Option<NodeId>,
    node_type: &str,
) -> Vec<NodeId> {
    parent
        .map(|parent| {
            snapshot
                .child_ids_slice(parent)
                .iter()
                .copied()
                .filter(|child| {
                    snapshot
                        .node(*child)
                        .is_some_and(|node| node.node_type == node_type)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn matching_profile(
    snapshot: &ProcessTreeSnapshot,
    parent: Option<NodeId>,
    node_type: &str,
    profile_key: Option<&str>,
) -> Option<NodeId> {
    let profiles = typed_children(snapshot, parent, node_type);
    profile_key
        .and_then(|expected| {
            profiles
                .iter()
                .copied()
                .find(|profile| child_string(snapshot, *profile, "profile_key", "") == expected)
        })
        .or_else(|| profiles.first().copied())
}

pub(super) fn child_id(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
) -> NodeId {
    find_child_by_key(snapshot, parent, key).unwrap_or(NodeId(0))
}

fn child_param<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
) -> Option<&'a ParamValue> {
    find_child_by_key(snapshot, parent, key)
        .and_then(|node| snapshot.node(node))
        .and_then(|node| node.param_value.as_ref())
}

pub(super) fn child_bool(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
    default: bool,
) -> bool {
    child_param(snapshot, parent, key)
        .and_then(ParamValue::as_bool)
        .unwrap_or(default)
}

pub(super) fn child_int(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
    default: i32,
) -> i32 {
    child_param(snapshot, parent, key)
        .and_then(ParamValue::as_int)
        .unwrap_or(default)
}

pub(super) fn child_float(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
    default: f64,
) -> f64 {
    child_param(snapshot, parent, key)
        .and_then(ParamValue::as_float)
        .unwrap_or(default)
}

pub(super) fn child_string(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
    default: &str,
) -> String {
    child_param(snapshot, parent, key)
        .and_then(ParamValue::as_str)
        .unwrap_or_else(|| default.to_owned())
}

pub(super) fn child_enum(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
    default: &str,
) -> String {
    child_param(snapshot, parent, key)
        .and_then(ParamValue::as_enum)
        .unwrap_or_else(|| default.to_owned())
}

pub(super) fn child_reference(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
) -> Option<NodeReference> {
    match child_param(snapshot, parent, key)? {
        ParamValue::Reference(reference) => Some(reference.clone()),
        _ => None,
    }
}

pub(super) fn child_gain(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
) -> Result<GainDb, ConfigurationError> {
    GainDb::new(child_float(snapshot, parent, key, 0.0) as f32)
        .map_err(|error| configuration_error(parent, error.to_string()))
}

pub(super) fn gain(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    path: &str,
) -> Result<GainDb, ConfigurationError> {
    let node = required_path(snapshot, module, path)?;
    let value = snapshot
        .node(node)
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_float)
        .unwrap_or(0.0);
    GainDb::new(value as f32).map_err(|error| configuration_error(node, error.to_string()))
}

pub(super) fn route_id(
    snapshot: &ProcessTreeSnapshot,
    route: NodeId,
) -> AudioRouteId {
    AudioRouteId::from_uuid(
        snapshot
            .node(route)
            .map(|node| node.uuid.0)
            .unwrap_or_else(uuid::Uuid::nil),
    )
}

pub(super) fn child_by_uuid(
    snapshot: &ProcessTreeSnapshot,
    parent: Option<NodeId>,
    uuid: NodeUuid,
) -> Option<NodeId> {
    let parent = parent?;
    snapshot
        .child_ids_slice(parent)
        .iter()
        .copied()
        .find(|node| snapshot.node(*node).is_some_and(|node| node.uuid == uuid))
}
