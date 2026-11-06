//! Detached owned media and Placement are admitted as one authored subtree.
use super::{PlacementNode, authoring, children, protocol::*};
use crate::{
    app::{AppEngine, labels},
    media::{creation, protocol::*},
};
use golden_core::{
    edit::{Edit, NodeTree},
    node::{Node, NodeId},
};
use golden_video::Extent2D;

pub(super) fn apply(
    engine: &mut AppEngine,
    reference: ChannelReference,
    channel: NodeId,
    canvas: Extent2D,
    kind: MediaKind,
    label: Option<String>,
    position: Point,
) -> Result<MediaEditResult, String> {
    if !position.x.is_finite() || !position.y.is_finite() {
        return Err("Composition creation requires finite canvas coordinates".into());
    }
    let count = authoring::instance_nodes(engine, channel)?.try_fold(0usize, |count, node| node.map(|_| count + 1))?;
    if count >= authoring::MAX_INSTANCES {
        return Err("Composition exceeds 100000 authored instances".into());
    }
    let parent = children::owner(engine, channel)?;
    let label = labels::unique(engine, parent, label, kind.default_label(), &[])?;
    let mut definition = InstanceDefinition::for_canvas(canvas);
    definition.transform.position = [position.x, position.y];
    let tree = creation::owned_tree(engine, kind, label).with_child(NodeTree::new(PlacementNode::from_definition(
        "Placement".into(),
        definition,
    )));
    let instance = InstanceId(tree.node.node_data().meta.uuid);
    engine.edits.push(Edit::AddUserItemTree {
        parent,
        prev_sibling: engine.nodes.get(parent).unwrap().node_data().last_child,
        tree,
    });
    Ok(MediaEditResult::InstancesCreated {
        instances: vec![InstanceReference {
            composition: reference,
            instance,
        }],
    })
}
