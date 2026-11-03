//! Reference validation follows reachable authored/retained channels without a tree snapshot.
use crate::{
    matrix::{
        node::ElementNode,
        protocol::{ElementDefinition, MatrixItemId},
    },
    media::{ChannelNode, protocol::MediaKind},
};
use golden_core::node::{NodeId, NodeUuid, ParameterValidationContext};
use golden_core::parameter::ParamValue;
use std::collections::HashSet;

fn leaf(change: &ParameterValidationContext<'_>, mut media: NodeUuid) -> Result<Option<NodeId>, String> {
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(media) {
            return Err("Reference media dependency cycle".into());
        }
        let Some(id) = change.resolve(media) else {
            return Ok(None);
        };
        let Some(node) = change.node(id) else { return Ok(None) };
        if let Some(reference) = node.as_any().downcast_ref::<crate::media::ReferenceNode>() {
            let Some(target) = reference.target_for_validation(change) else {
                return Ok(None);
            };
            media = target;
        } else {
            return Ok(node.as_any().is::<crate::media::MediaNode>().then_some(id));
        }
    }
}
fn canonical(
    change: &ParameterValidationContext<'_>,
    source: crate::media::protocol::ChannelReference,
) -> Result<Option<NodeUuid>, String> {
    let Some(owner) = leaf(change, source.media.0)? else {
        return Ok(None);
    };
    let channel = if source.channel.0.0.is_nil() {
        children(change, owner)
            .into_iter()
            .filter_map(|id| change.node(id))
            .find(|node| node.as_any().is::<ChannelNode>())
            .map(|node| node.node_data().meta.uuid)
    } else {
        Some(source.channel.0)
    };
    Ok(channel
        .and_then(|channel| change.resolve(channel))
        .and_then(|id| change.node(id))
        .filter(|node| node.node_data().parent == Some(owner))
        .map(|node| node.node_data().meta.uuid))
}
pub(crate) fn validate_output(
    change: &ParameterValidationContext<'_>,
    media: NodeUuid,
    output: NodeUuid,
) -> Result<(), String> {
    if let Some(owner) = leaf(change, media)?
        && let Some(id) = change.resolve(output)
        && change
            .node(id)
            .is_some_and(|node| node.node_data().parent != Some(owner))
    {
        return Err("Output belongs to another media".into());
    }
    validate_media(change, media)
}
/// A Reference target is a whole media, but cycles remain channel-specific. Canonical aliases
/// avoid expanding every mirrored channel across every Reference in a large document.
pub(crate) fn validate_media(change: &ParameterValidationContext<'_>, target: NodeUuid) -> Result<(), String> {
    let Some(root) = leaf(change, target)? else {
        return Ok(());
    };
    let mut states = std::collections::HashMap::new();
    for root in children(change, root) {
        let Some(node) = change.node(root).filter(|node| node.as_any().is::<ChannelNode>()) else {
            continue;
        };
        let mut pending = vec![(node.node_data().meta.uuid, false)];
        while let Some((id, exiting)) = pending.pop() {
            if exiting {
                states.insert(id, 2u8);
                continue;
            }
            match states.get(&id) {
                Some(2) => continue,
                Some(1) => return Err("media channel dependency cycle".into()),
                _ => {}
            }
            states.insert(id, 1);
            pending.push((id, true));
            pending.extend(edges(change, id)?.into_iter().rev().map(|source| (source, false)));
        }
    }
    Ok(())
}
fn edges(change: &ParameterValidationContext<'_>, source: NodeUuid) -> Result<Vec<NodeUuid>, String> {
    let mut pending = Vec::new();
    let Some(id) = change.resolve(source) else {
        return Ok(pending);
    };
    let Some(node) = change.node(id) else {
        return Ok(pending);
    };
    let Some(channel) = node.as_any().downcast_ref::<ChannelNode>() else {
        return Ok(pending);
    };
    match channel.kind() {
        MediaKind::Composition => {
            if let Some(media) = node.node_data().parent {
                for child in children(change, media) {
                    let Some(owner) = change.node(child).filter(|node| {
                        node.as_any().is::<crate::media::MediaNode>()
                            || node.as_any().is::<crate::media::ReferenceNode>()
                    }) else {
                        continue;
                    };
                    for placement in children(change, child) {
                        if let Some(placement) = change
                            .node(placement)
                            .and_then(|node| node.as_any().downcast_ref::<crate::media::composition::PlacementNode>())
                            && let Some(ParamValue::Reference(output)) =
                                change.parameter_value(placement.output_parameter())
                            && let Some(source) = canonical(
                                change,
                                crate::media::protocol::ChannelReference {
                                    media: crate::media::protocol::MediaId(owner.node_data().meta.uuid),
                                    channel: crate::media::protocol::ChannelId(output.uuid),
                                },
                            )?
                        {
                            pending.push(source);
                        }
                    }
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
            matrix_sources(change, rows, &mut pending)?;
            if let Some(control) = root
                .and_then(|root| change.node(root))
                .and_then(|node| node.as_any().downcast_ref::<ChannelNode>())
                .and_then(ChannelNode::matrix_playback)
            {
                for (_, dependency) in control
                    .retained_edges()
                    .into_iter()
                    .filter(|(owner, _)| owner.channel.0 == source)
                {
                    if let Some(source) = canonical(change, dependency)? {
                        pending.push(source);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(pending)
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

fn matrix_sources(
    change: &ParameterValidationContext<'_>,
    mut rows: Vec<NodeId>,
    pending: &mut Vec<NodeUuid>,
) -> Result<(), String> {
    while let Some(id) = rows.pop() {
        if let Some(cell) = change
            .node(id)
            .and_then(|node| node.as_any().downcast_ref::<crate::matrix::CellNode>())
        {
            if let Some(ParamValue::Reference(output)) = change.parameter_value(cell.output_parameter()) {
                for child in children(change, id) {
                    if let Some(owner) = change.node(child).filter(|node| {
                        node.as_any().is::<crate::media::MediaNode>()
                            || node.as_any().is::<crate::media::ReferenceNode>()
                    }) && let Some(source) = canonical(
                        change,
                        crate::media::protocol::ChannelReference {
                            media: crate::media::protocol::MediaId(owner.node_data().meta.uuid),
                            channel: crate::media::protocol::ChannelId(output.uuid),
                        },
                    )? {
                        pending.push(source);
                    }
                }
            }
            continue;
        }
        let Some(element) = change
            .node(id)
            .and_then(|node| node.as_any().downcast_ref::<ElementNode>())
        else {
            continue;
        };
        match element.definition() {
            ElementDefinition::Layer { .. } | ElementDefinition::Group { .. } => rows.extend(children(change, id)),
        }
    }
    Ok(())
}
