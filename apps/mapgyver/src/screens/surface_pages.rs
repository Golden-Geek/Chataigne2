//! Shared bounded-query cursor over actual Screen and Surface siblings, never their Parameter trees.
use super::{authoring, protocol::ScreenId};
use crate::app::{AppEngine, AppNode};
use golden_core::node::{Node, NodeId, NodeUuid};

pub(super) fn start(engine: &AppEngine, cursor: Option<NodeUuid>) -> Result<Option<NodeId>, String> {
    let manager = authoring::manager(engine)?;
    let Some(uuid) = cursor else {
        return Ok(engine.nodes.get(manager).and_then(|node| node.node_data().first_child));
    };
    let Some(id) = engine.node_id_by_uuid(uuid) else {
        return start(engine, None);
    };
    let node = engine.nodes.get(id).ok_or("Surface cursor is missing")?;
    let parent = node.node_data().parent.ok_or("Surface cursor has no owner")?;
    let valid = (parent == manager && matches!(node, AppNode::Screen(_)))
        || (matches!(engine.nodes.get(parent), Some(AppNode::Screen(_)))
            && authoring::screen_id(
                engine,
                ScreenId(engine.nodes.get(parent).unwrap().node_data().meta.uuid),
            )
            .is_ok());
    if !valid {
        return Err("Surface cursor is outside Screens".into());
    }
    Ok(Some(id))
}

pub(super) fn advance(engine: &AppEngine, id: NodeId) -> Option<NodeId> {
    let node = engine.nodes.get(id)?;
    if matches!(node, AppNode::Screen(_)) {
        node.node_data().first_child.or(node.node_data().next_sibling)
    } else {
        node.node_data().next_sibling.or_else(|| {
            engine
                .nodes
                .get(node.node_data().parent?)
                .and_then(|node| node.node_data().next_sibling)
        })
    }
}
