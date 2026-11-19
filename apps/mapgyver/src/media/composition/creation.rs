//! Detached owned media and Placement are admitted as one authored subtree.
use super::{PlacementNode, authoring, children, protocol::*};
use crate::{
    app::{AppEngine, labels},
    media::{creation, importing::FileMedia, protocol::*},
};
use golden_core::{
    edit::{Edit, NodeTree},
    node::{Node, NodeId},
};
use golden_video::Extent2D;

pub(super) enum Child {
    Media { kind: MediaKind, label: Option<String> },
    File(FileMedia),
}

pub(super) fn apply(
    engine: &mut AppEngine,
    reference: ChannelReference,
    channel: NodeId,
    canvas: Extent2D,
    position: Point,
    child: Child,
) -> Result<MediaEditResult, String> {
    if !position.x.is_finite() || !position.y.is_finite() {
        return Err("Composition creation requires finite canvas coordinates".into());
    }
    let count = authoring::instance_nodes(engine, channel)?.try_fold(0usize, |count, node| node.map(|_| count + 1))?;
    if count >= authoring::MAX_INSTANCES {
        return Err("Composition exceeds 100000 authored instances".into());
    }
    let parent = children::owner(engine, channel)?;
    let mut definition = InstanceDefinition::for_canvas(canvas);
    let (media, position) = match child {
        Child::Media { kind, label } => {
            let label = labels::unique(engine, parent, label, kind.default_label(), &[])?;
            (creation::owned_tree(engine, kind, label), [position.x, position.y])
        }
        Child::File(file) => (
            file.tree(engine, Some(parent))?.0,
            [
                position.x.clamp(0.0, canvas.width() as f32),
                position.y.clamp(0.0, canvas.height() as f32),
            ],
        ),
    };
    definition.transform.position = position;
    let tree = media.with_child(NodeTree::new(PlacementNode::from_definition(
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
