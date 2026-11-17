//! Immutable source equivalence for app-owned editors, independent of device availability.
use super::{protocol::ChannelReference, reference::Resolver};
use crate::app::{AppEngine, AppNode};
use golden_core::node::Node;

pub(crate) struct SourceIdentity<'a> {
    engine: &'a AppEngine,
    resolver: Resolver<'a>,
}

impl<'a> SourceIdentity<'a> {
    pub fn new(engine: &'a AppEngine) -> Self {
        Self {
            engine,
            resolver: Resolver::new(engine),
        }
    }

    /// Reference outputs share their selected leaf channel. Unassigned or missing routes
    /// have no identity; separate unassigned Surfaces must never become one source group.
    pub fn resolve(&mut self, reference: Option<ChannelReference>) -> Option<ChannelReference> {
        let reference = self.resolver.canonical(reference?).ok()?;
        let owner = self.engine.node_id_by_uuid(reference.media.0)?;
        if !matches!(self.engine.nodes.get(owner)?, AppNode::Media(_)) {
            return None;
        }
        let id = self.engine.node_id_by_uuid(reference.channel.0)?;
        let node = self.engine.nodes.get(id)?;
        (matches!(node, AppNode::MediaChannel(_)) && node.node_data().parent == Some(owner)).then_some(reference)
    }
}
