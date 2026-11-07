//! Pure media target resolution for atomic Parameter proposals; never UI-owned defaults.
use super::{ChannelNode, MediaNode, ReferenceNode};
use golden_core::node::{NodeId, NodeUuid, ParameterValidationContext};

/// Project validation retains missing identities, while rejecting a resolved foreign output.
pub(crate) fn validate_existing_output(
    engine: &crate::app::AppEngine,
    output: super::protocol::ChannelReference,
) -> Result<(), String> {
    if engine.node_id_by_uuid(output.media.0).is_some() && engine.node_id_by_uuid(output.channel.0).is_some() {
        super::authoring::channel_internal(engine, output)?;
    }
    Ok(())
}
pub(crate) fn owner(change: &ParameterValidationContext<'_>, media: NodeUuid) -> Option<NodeId> {
    let id = change.resolve(media)?;
    let node = change.node(id)?;
    (node.as_any().is::<MediaNode>() || node.as_any().is::<ReferenceNode>()).then_some(id)
}

/// Select only a currently exposed output. An empty result stays unconfigured instead of
/// inventing an identity; subsequent channel loss never rewrites a pinned authored choice.
pub(crate) fn first_output(
    change: &ParameterValidationContext<'_>,
    media: NodeUuid,
) -> Result<Option<NodeUuid>, String> {
    let Some(owner) = owner(change, media) else {
        return Ok(None);
    };
    let mut next = change.node(owner).and_then(|node| node.node_data().first_child);
    while let Some(id) = next {
        let node = change.node(id).ok_or("media child is missing")?;
        next = node.node_data().next_sibling;
        if let Some(channel) = node.as_any().downcast_ref::<ChannelNode>()
            && crate::matrix::outputs::exposed_for_validation(change, channel)
        {
            return Ok(Some(node.node_data().meta.uuid));
        }
        if node.as_any().is::<super::ReferenceOutputNode>() {
            return Ok(Some(node.node_data().meta.uuid));
        }
    }
    Ok(None)
}

/// A Surface binding adds no media dependency edge; validating its owner must not walk
/// every channel/dependency of the source. Missing identities stay authored for recovery.
pub(crate) fn validate_output_membership(
    change: &ParameterValidationContext<'_>,
    media: NodeUuid,
    output: NodeUuid,
) -> Result<(), String> {
    if let Some(owner) = owner(change, media)
        && let Some(id) = change.resolve(output)
        && change
            .node(id)
            .is_some_and(|node| node.node_data().parent != Some(owner))
    {
        return Err("Output belongs to another media".into());
    }
    Ok(())
}
