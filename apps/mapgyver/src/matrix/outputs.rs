//! Export policy is separate from the stable internal channels used by Group/Final rendering.
use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::ChannelNode,
};
use golden_core::node::Node;
use golden_io::RevisionGuard;

fn element<'a>(engine: &'a AppEngine, channel: &ChannelNode) -> Option<&'a super::ElementNode> {
    let uuid = match channel.matrix_item()? {
        MatrixItemId::Layer { id } => id.0,
        MatrixItemId::Group { id } => id.0,
    };
    match engine.nodes.get(engine.node_id_by_uuid(uuid)?)? {
        AppNode::MatrixElement(node) => Some(node),
        _ => None,
    }
}

pub(crate) fn exposed(engine: &AppEngine, channel: &ChannelNode) -> bool {
    if channel.matrix_item().is_none() {
        return true;
    }
    element(engine, channel).is_some_and(|node| {
        let (ElementDefinition::Layer { settings } | ElementDefinition::Group { settings }) = node.definition();
        settings.expose_channel
    })
}

pub(crate) fn exposed_for_validation(
    change: &golden_core::node::ParameterValidationContext<'_>,
    channel: &ChannelNode,
) -> bool {
    let Some(item) = channel.matrix_item() else { return true };
    let uuid = match item {
        MatrixItemId::Layer { id } => id.0,
        MatrixItemId::Group { id } => id.0,
    };
    change
        .resolve(uuid)
        .and_then(|id| change.node(id))
        .and_then(|node| node.as_any().downcast_ref::<super::ElementNode>())
        .is_some_and(|node| node.exposed_for_validation(change))
}

pub(crate) fn guard(engine: &AppEngine, channel: &ChannelNode) -> Option<RevisionGuard> {
    let element = element(engine, channel)?;
    let parent = engine.nodes.get(channel.node_data().parent?)?;
    let mut next = parent.node_data().first_child;
    while let Some(id) = next {
        let node = engine.nodes.get(id)?;
        next = node.node_data().next_sibling;
        if let AppNode::MediaChannel(root) = node
            && root.is_matrix_final()
            && let Some(control) = root.matrix_playback()
        {
            element.bind_runtime(control.binding());
            // Settle an already committed launch before fencing its visibility. Matrix capture
            // settles the same receipt; taking this guard earlier would invalidate the capture.
            control.snapshot();
            return Some(control.guard());
        }
    }
    None
}
