//! Pure media target resolution for atomic Parameter proposals; never UI-owned defaults.
use super::{ChannelNode, MediaNode, ReferenceNode};
use golden_core::node::{NodeId, NodeUuid, ParameterValidationContext};
use std::collections::HashSet;

pub(crate) fn leaf(change: &ParameterValidationContext<'_>, mut media: NodeUuid) -> Result<Option<NodeId>, String> {
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(media) {
            return Err("Reference media dependency cycle".into());
        }
        let Some(id) = change.resolve(media) else {
            return Ok(None);
        };
        let Some(node) = change.node(id) else { return Ok(None) };
        if let Some(reference) = node.as_any().downcast_ref::<ReferenceNode>() {
            let Some(target) = reference.target_for_validation(change) else {
                return Ok(None);
            };
            media = target;
        } else {
            return Ok(node.as_any().is::<MediaNode>().then_some(id));
        }
    }
}

/// Select only a currently exposed output. An empty result stays unconfigured instead of
/// inventing an identity; subsequent channel loss never rewrites a pinned authored choice.
pub(crate) fn first_output(
    change: &ParameterValidationContext<'_>,
    media: NodeUuid,
) -> Result<Option<NodeUuid>, String> {
    let Some(owner) = leaf(change, media)? else {
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
    if let Some(owner) = leaf(change, media)?
        && let Some(id) = change.resolve(output)
        && change
            .node(id)
            .is_some_and(|node| node.node_data().parent != Some(owner))
    {
        return Err("Output belongs to another media".into());
    }
    Ok(())
}
