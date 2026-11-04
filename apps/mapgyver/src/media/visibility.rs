//! Public channel admission, including virtual Reference identities, never changes internal IDs.
use super::protocol::ChannelReference;
use crate::app::{AppEngine, AppNode};
use golden_io::RevisionGuard;

pub(super) fn exposed(engine: &AppEngine, reference: ChannelReference) -> bool {
    match engine
        .node_id_by_uuid(reference.channel.0)
        .and_then(|id| engine.nodes.get(id))
    {
        Some(AppNode::MediaChannel(channel)) => crate::matrix::outputs::exposed(engine, channel),
        _ => true,
    }
}
pub(super) fn guard(engine: &AppEngine, reference: ChannelReference) -> Option<RevisionGuard> {
    match engine
        .node_id_by_uuid(reference.channel.0)
        .and_then(|id| engine.nodes.get(id))
    {
        Some(AppNode::MediaChannel(channel)) => crate::matrix::outputs::guard(engine, channel),
        _ => None,
    }
}
