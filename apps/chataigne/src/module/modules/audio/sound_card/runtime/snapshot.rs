use super::*;
use crate::app::module_modules_audio_sound_card::{find_child_by_key, structure::channel_name};

#[derive(Clone, Debug)]
pub(super) struct InputChannel {
    pub(super) node: NodeId,
    pub(super) id: AudioChannelId,
    pub(super) label: String,
}

#[derive(Clone, Debug)]
pub(super) struct OutputChannel {
    pub(super) node: NodeId,
    pub(super) uuid: NodeUuid,
    pub(super) id: AudioChannelId,
    pub(super) label: String,
    pub(super) gain: GainDb,
}

pub(super) fn collect_input_channels(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
) -> Result<Vec<InputChannel>, ConfigurationError> {
    parameter_children(snapshot, find_path(snapshot, module, INPUT_CHANNELS_PATH))
    .into_iter()
    .map(|node| {
        let state = snapshot.node(node).expect("typed child exists");
        Ok(InputChannel {
            node,
            id: AudioChannelId::from_uuid(state.uuid.0),
            label: channel_name(state),
        })
    })
    .collect()
}

pub(super) fn collect_output_channels(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
) -> Result<Vec<OutputChannel>, ConfigurationError> {
    parameter_children(snapshot, find_path(snapshot, module, OUTPUT_CHANNELS_PATH))
    .into_iter()
    .map(|node| {
        let state = snapshot.node(node).expect("typed child exists");
        Ok(OutputChannel {
            node,
            uuid: state.uuid,
            id: AudioChannelId::from_uuid(state.uuid.0),
            label: channel_name(state),
            gain: gain_at_node(snapshot, node)?,
        })
    })
    .collect()
}

fn parameter_children(snapshot: &ProcessTreeSnapshot, parent: Option<NodeId>) -> Vec<NodeId> {
    parent
        .map(|parent| {
            snapshot
                .child_ids_slice(parent)
                .iter()
                .copied()
                .filter(|child| {
                    snapshot
                        .node(*child)
                        .and_then(|node| node.param_value.as_ref())
                        .is_some_and(|value| matches!(value, ParamValue::Float(_)))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn unresolved_warning(node: NodeId, field: &str, message: impl Into<String>) -> RuntimeWarning {
    RuntimeWarning {
        node,
        id: format!("sound-card-unresolved-{field}"),
        message: message.into(),
        detail: None,
    }
}

pub(super) fn configuration_error(node: NodeId, message: impl Into<String>) -> ConfigurationError {
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

pub(super) fn typed_children(snapshot: &ProcessTreeSnapshot, parent: Option<NodeId>, node_type: &str) -> Vec<NodeId> {
    parent
        .map(|parent| {
            snapshot
                .child_ids_slice(parent)
                .iter()
                .copied()
                .filter(|child| snapshot.node(*child).is_some_and(|node| node.node_type == node_type))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn child_id(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str) -> NodeId {
    find_child_by_key(snapshot, parent, key).unwrap_or(NodeId(0))
}

fn child_param<'a>(snapshot: &'a ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<&'a ParamValue> {
    find_child_by_key(snapshot, parent, key)
        .and_then(|node| snapshot.node(node))
        .and_then(|node| node.param_value.as_ref())
}

pub(super) fn child_string(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str, default: &str) -> String {
    child_param(snapshot, parent, key)
        .and_then(ParamValue::as_str)
        .unwrap_or_else(|| default.to_owned())
}

pub(super) fn child_enum(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str, default: &str) -> String {
    child_param(snapshot, parent, key)
        .and_then(ParamValue::as_enum)
        .unwrap_or_else(|| default.to_owned())
}

pub(super) fn required_child_enum(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
) -> Result<(NodeId, String), ConfigurationError> {
    let node = required_path(snapshot, parent, key)?;
    let value = snapshot
        .node(node)
        .and_then(|state| state.param_value.as_ref())
        .and_then(ParamValue::as_enum)
        .ok_or_else(|| {
            configuration_error(
                node,
                format!("Sound Card selector '{key}' is missing an enum value"),
            )
        })?;
    Ok((node, value))
}

pub(super) fn child_reference(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<NodeReference> {
    match child_param(snapshot, parent, key)? {
        ParamValue::Reference(reference) => Some(reference.clone()),
        _ => None,
    }
}

pub(super) fn gain(snapshot: &ProcessTreeSnapshot, module: NodeId, path: &str) -> Result<GainDb, ConfigurationError> {
    let node = required_path(snapshot, module, path)?;
    let value = snapshot
        .node(node)
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_float)
        .unwrap_or(0.0);
    GainDb::new(value as f32).map_err(|error| configuration_error(node, error.to_string()))
}

pub(super) fn gain_at_node(
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
) -> Result<GainDb, ConfigurationError> {
    let value = snapshot
        .node(node)
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_float)
        .unwrap_or(0.0);
    GainDb::new(value as f32).map_err(|error| configuration_error(node, error.to_string()))
}

pub(super) fn route_id(snapshot: &ProcessTreeSnapshot, route: NodeId) -> AudioRouteId {
    AudioRouteId::from_uuid(
        snapshot
            .node(route)
            .map(|node| node.uuid.0)
            .unwrap_or_else(uuid::Uuid::nil),
    )
}
