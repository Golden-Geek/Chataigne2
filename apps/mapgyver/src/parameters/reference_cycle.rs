//! Reference validation follows reachable authored/retained channels without a tree snapshot.
use crate::{
    matrix::{
        node::ElementNode,
        protocol::{ElementDefinition, MatrixItemId},
    },
    media::{ChannelNode, composition::InstanceNode, protocol::MediaKind},
};
use golden_core::node::{NodeId, NodeUuid, ParameterValidationContext};
use std::collections::HashSet;

pub(crate) fn validate(change: &ParameterValidationContext<'_>, owner: NodeId, source: NodeUuid) -> Result<(), String> {
    let Some(owner) = change.node(owner) else { return Ok(()) };
    let owner = owner.node_data().meta.uuid;
    let mut pending = vec![source];
    let mut seen = HashSet::new();
    while let Some(source) = pending.pop() {
        if source == owner {
            return Err("media channel dependency cycle".into());
        }
        if !seen.insert(source) {
            continue;
        }
        let Some(id) = change.resolve(source) else { continue };
        let Some(node) = change.node(id) else { continue };
        let Some(channel) = node.as_any().downcast_ref::<ChannelNode>() else {
            continue;
        };
        match channel.kind() {
            MediaKind::Composition => {
                for child in children(change, id) {
                    if let Some(instance) = change
                        .node(child)
                        .and_then(|node| node.as_any().downcast_ref::<InstanceNode>())
                        && let Some(source) = instance.source_for_validation(change)
                    {
                        pending.push(source.channel.0);
                    }
                }
            }
            MediaKind::Matrix => {
                let output = channel.matrix_item();
                let root = if channel.is_matrix_final() {
                    Some(id)
                } else {
                    node.node_data().parent.and_then(|parent| {
                        children(change, parent).into_iter().find(|child| {
                            change
                                .node(*child)
                                .and_then(|node| node.as_any().downcast_ref::<ChannelNode>())
                                .is_some_and(ChannelNode::is_matrix_final)
                        })
                    })
                };
                let rows = match output {
                    None => root.map(|root| children(change, root)).unwrap_or_default(),
                    Some(MatrixItemId::Layer { id }) => change.resolve(id.0).into_iter().collect(),
                    Some(MatrixItemId::Group { id }) => change.resolve(id.0).into_iter().collect(),
                };
                matrix_sources(change, rows, &mut pending);
                if let Some(control) = root
                    .and_then(|root| change.node(root))
                    .and_then(|node| node.as_any().downcast_ref::<ChannelNode>())
                    .and_then(ChannelNode::matrix_playback)
                {
                    pending.extend(
                        control
                            .retained_edges()
                            .into_iter()
                            .filter(|(owner, _)| owner.channel.0 == source)
                            .map(|(_, source)| source.channel.0),
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn children(change: &ParameterValidationContext<'_>, parent: NodeId) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut next = change.node(parent).and_then(|node| node.node_data().first_child);
    while let Some(id) = next {
        result.push(id);
        next = change.node(id).and_then(|node| node.node_data().next_sibling);
    }
    result
}

fn matrix_sources(change: &ParameterValidationContext<'_>, mut rows: Vec<NodeId>, pending: &mut Vec<NodeUuid>) {
    while let Some(id) = rows.pop() {
        let Some(element) = change
            .node(id)
            .and_then(|node| node.as_any().downcast_ref::<ElementNode>())
        else {
            continue;
        };
        match element.definition() {
            ElementDefinition::Cell { definition } => {
                if let Some(source) = definition.source {
                    pending.push(source.channel.0);
                }
            }
            ElementDefinition::Layer { .. } | ElementDefinition::Group { .. } => rows.extend(children(change, id)),
            ElementDefinition::Column { .. } => {}
        }
    }
}
