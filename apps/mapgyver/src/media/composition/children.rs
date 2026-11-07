//! Actual media own placement Parameters; canvas instances are read projections of those children.
use super::{PlacementNode, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::{ReferenceNode, protocol::*},
};
use golden_core::{
    edit::NodeTree,
    node::{Node, NodeId},
};

pub(crate) fn owner(engine: &AppEngine, channel: NodeId) -> Result<NodeId, String> {
    engine
        .nodes
        .get(channel)
        .and_then(|node| node.node_data().parent)
        .ok_or("Composition media is missing".into())
}
pub(crate) fn placement(engine: &AppEngine, media: NodeId) -> Result<&PlacementNode, String> {
    let mut child = engine
        .nodes
        .get(media)
        .ok_or("media child is missing")?
        .node_data()
        .first_child;
    let mut result = None;
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("media child is missing")?;
        child = node.node_data().next_sibling;
        if let AppNode::CompositionPlacement(node) = node
            && result.replace(node.as_ref()).is_some()
        {
            return Err("media has duplicate Placement children".into());
        }
    }
    result.ok_or("Composition child media requires Placement".into())
}
pub(crate) fn definition(engine: &AppEngine, media: NodeId) -> Result<InstanceDefinition, String> {
    let uuid = engine
        .nodes
        .get(media)
        .ok_or("media child is missing")?
        .node_data()
        .meta
        .uuid;
    let placement = placement(engine, media)?;
    let mut definition = placement.definition(MediaId(uuid));
    definition.source = crate::media::MediaSource {
        media: MediaId(uuid),
        channel: placement.output(),
    }
    .output(engine)?;
    Ok(definition)
}
pub(crate) fn reference_tree(
    label: String,
    source: Option<crate::media::MediaSource>,
    definition: InstanceDefinition,
) -> (NodeTree, InstanceId) {
    let node = ReferenceNode::from_source(label, source);
    let id = InstanceId(node.node_data().meta.uuid);
    let mut definition = definition;
    definition.source = None;
    let tree = NodeTree::new(node).with_child(NodeTree::new(PlacementNode::from_definition(
        "Placement".into(),
        definition,
    )));
    (tree, id)
}
