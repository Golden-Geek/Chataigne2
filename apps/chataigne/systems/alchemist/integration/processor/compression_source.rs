use golden_core::{
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::ProcessTreeSnapshot,
};

use super::{FrozenMappingSource, PROCESSOR_COMPRESSION_ENABLED_DECL_ID, PROCESSOR_FROZEN_SOURCE_DECL_ID};
use crate::app::AppEngine;

pub(crate) fn processor_frozen_source(
    snapshot: &ProcessTreeSnapshot,
    processor: NodeId,
) -> Result<Option<FrozenMappingSource>, String> {
    if !super::processor_compression_enabled(snapshot, processor) {
        return Ok(None);
    }
    let value = snapshot
        .find_child_by_decl_id(processor, PROCESSOR_FROZEN_SOURCE_DECL_ID)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref());
    decode(value).map(Some)
}

/// Settlement reads canonical live controls, including edits whose callbacks have not run.
pub(super) fn live_frozen_source(
    engine: &AppEngine,
    processor: NodeId,
) -> Result<Option<(NodeId, FrozenMappingSource)>, String> {
    let enabled = live_child(engine, processor, PROCESSOR_COMPRESSION_ENABLED_DECL_ID)
        .and_then(|id| engine.nodes.get(id))
        .and_then(Node::engine_param_snapshot);
    if !enabled.is_some_and(|state| matches!(state.value, ParamValue::Bool(true))) {
        return Ok(None);
    }
    let control = live_child(engine, processor, PROCESSOR_FROZEN_SOURCE_DECL_ID).ok_or_else(missing_source)?;
    let state = engine.nodes.get(control).and_then(Node::engine_param_snapshot);
    decode(state.as_ref().map(|state| &state.value)).map(|source| Some((control, source)))
}

fn live_child(engine: &AppEngine, parent: NodeId, declaration: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id)?;
        let data = node.node_data();
        if data.meta.decl_id.0 == declaration || data.meta.decl_id.0.rsplit('/').next() == Some(declaration) {
            return Some(id);
        }
        child = data.next_sibling;
    }
    None
}

fn missing_source() -> String {
    "compressed Mapping has no frozen source".to_owned()
}

fn decode(value: Option<&ParamValue>) -> Result<FrozenMappingSource, String> {
    let encoded = value
        .and_then(|value| match value {
            ParamValue::Str(value) => Some(value.as_str()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(missing_source)?;
    serde_json::from_str(encoded).map_err(|error| format!("compressed Mapping source is invalid: {error}"))
}
