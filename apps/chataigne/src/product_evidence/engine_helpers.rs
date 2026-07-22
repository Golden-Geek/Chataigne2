use golden_core::edit::Edit;
use golden_core::node::{Node, NodeId};
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
use golden_core::process_ctx::ExecutionPhase;

use crate::app::AppEngine;

pub(super) fn find_node_by_type(engine: &AppEngine, node_type: &str) -> Option<NodeId> {
    engine
        .nodes
        .iter()
        .find_map(|(id, node)| (node.get_type() == node_type).then_some(id))
}

pub(super) fn find_child_by_type(engine: &AppEngine, parent: NodeId, node_type: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        if node.get_type() == node_type {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }
    None
}

pub(super) fn find_child_by_key(engine: &AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        let meta = &node.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }
    None
}

pub(super) fn find_path(engine: &AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    let mut remaining = path.trim_matches('/');
    loop {
        if remaining.is_empty() {
            return Some(current);
        }
        if let Some(found) = find_child_by_key(engine, current, remaining) {
            return Some(found);
        }
        let Some((segment, tail)) = remaining.split_once('/') else {
            return find_child_by_key(engine, current, remaining);
        };
        current = find_child_by_key(engine, current, segment)?;
        remaining = tail;
    }
}

pub(super) fn set_param(engine: &mut AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

pub(super) fn materialize(engine: &mut AppEngine, passes: usize) -> Result<(), String> {
    for _ in 0..passes {
        engine.apply_edits().map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) fn drive_effects(engine: &mut AppEngine) -> Result<(), String> {
    engine.apply_edits().map_err(|error| error.to_string())?;
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .map_err(|error| error.to_string())?;
    engine.apply_edits().map_err(|error| error.to_string())?;
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .map_err(|error| error.to_string())?;
    engine.apply_edits().map_err(|error| error.to_string())?;
    engine
        .run_tick(std::time::Duration::from_millis(20))
        .map_err(|error| error.to_string())?;
    engine.apply_edits().map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn param_value(engine: &AppEngine, node: NodeId) -> Option<ParamValue> {
    engine
        .nodes
        .get(node)
        .and_then(|node| node.engine_param_snapshot())
        .map(|snapshot| snapshot.value)
}
