//! Document identity resolution shared by every source drop intent and public caller.
use super::protocol::{ChannelId, ChannelReference, MediaId};
use crate::app::{AppEngine, AppNode};
use golden_core::node::{Node, NodeUuid};

/// A real media identity can exist before its first output becomes available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MediaSource {
    pub media: MediaId,
    pub channel: Option<ChannelId>,
}
impl MediaSource {
    /// Resolve only selected Video media (including Reference chains), never independent
    /// clocks for mirrors or transport actions on a Composition/Matrix's descendants.
    pub fn video_owner(
        self,
        engine: &AppEngine,
    ) -> Result<Option<(MediaId, std::sync::Arc<super::video::VideoControl>)>, String> {
        let Some(output) = self.capture(engine, &mut Vec::new())? else {
            return Ok(None);
        };
        let reference = super::reference::Resolver::new(engine).canonical(output)?;
        let Some(AppNode::Media(node)) = engine
            .node_id_by_uuid(reference.media.0)
            .and_then(|id| engine.nodes.get(id))
        else {
            return Ok(None);
        };
        if !node.node_data().effective_enabled {
            return Ok(None);
        }
        Ok(node.video_control().map(|control| (reference.media, control)))
    }
    pub fn output(self, engine: &AppEngine) -> Result<Option<ChannelReference>, String> {
        let channel = match self.channel {
            Some(channel) => Some(channel),
            None => super::reference::Resolver::new(engine)
                .channels(self.media)?
                .next()
                .map(|channel| ChannelId(channel.node_data().meta.uuid)),
        };
        Ok(channel.map(|channel| ChannelReference {
            media: self.media,
            channel,
        }))
    }
    pub fn capture(
        self,
        engine: &AppEngine,
        guards: &mut Vec<golden_io::RevisionGuard>,
    ) -> Result<Option<ChannelReference>, String> {
        if let Some(channel) = self.channel
            && let Some(guard) = super::visibility::guard(
                engine,
                ChannelReference {
                    media: self.media,
                    channel,
                },
            )
        {
            guards.push(guard);
        }
        let output = self
            .output(engine)?
            .filter(|reference| super::visibility::exposed(engine, *reference));
        if output.is_some() {
            return Ok(output);
        }
        let manager = super::authoring::manager(engine)?;
        let AppNode::MediaManager(manager) = engine.nodes.get(manager).unwrap() else {
            unreachable!()
        };
        guards.push(manager.membership_guard());
        let mut next = Some(self.media);
        let mut seen = std::collections::HashSet::new();
        while let Some(media) = next {
            if !seen.insert(media) {
                return Err("Reference media dependency cycle".into());
            }
            match engine.node_id_by_uuid(media.0).and_then(|id| engine.nodes.get(id)) {
                Some(AppNode::ReferenceMedia(node)) => {
                    guards.push(node.guard());
                    next = node.target();
                }
                Some(AppNode::Media(node)) => {
                    guards.push(node.guard());
                    break;
                }
                _ => break,
            }
        }
        Ok(None)
    }
}

/// Resolves an actual Media or Channel node. Media selects its first ordered channel;
/// Parameters, folders and other nodes never act as implicit source identities.
pub fn resolve_source_node(engine: &AppEngine, source: NodeUuid) -> Result<ChannelReference, String> {
    source_selection(engine, source)?
        .output(engine)?
        .ok_or("source media has no available output".into())
}

/// Drop policy pins an available first channel; unavailable media keep an automatic choice.
pub(crate) fn source_selection(engine: &AppEngine, source: NodeUuid) -> Result<MediaSource, String> {
    let id = engine.node_id_by_uuid(source).ok_or("source node is missing")?;
    let node = engine.nodes.get(id).ok_or("source node is missing")?;
    let reference = match node {
        AppNode::ReferenceMedia(_) => {
            super::authoring::media(engine, MediaId(source))?;
            let mut resolver = super::reference::Resolver::new(engine);
            let channel = resolver.channels(MediaId(source))?.next();
            return Ok(MediaSource {
                media: MediaId(source),
                channel: channel.map(|channel| ChannelId(channel.node_data().meta.uuid)),
            });
        }
        AppNode::Media(_) => {
            let media = MediaId(source);
            super::authoring::media(engine, media)?;
            let mut next = node.node_data().first_child;
            loop {
                let child = next.ok_or("source media has no channel")?;
                let node = engine.nodes.get(child).ok_or("source media child is missing")?;
                if matches!(node, AppNode::MediaChannel(_)) {
                    break ChannelReference {
                        media,
                        channel: ChannelId(node.node_data().meta.uuid),
                    };
                }
                next = node.node_data().next_sibling;
            }
        }
        AppNode::MediaChannel(_) | AppNode::ReferenceOutput(_) => {
            let parent = node.node_data().parent.ok_or("source channel has no owner")?;
            let owner = engine
                .nodes
                .get(parent)
                .filter(|node| matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_)))
                .ok_or("source channel owner is not media")?;
            ChannelReference {
                media: MediaId(owner.node_data().meta.uuid),
                channel: ChannelId(source),
            }
        }
        _ => return Err("source must be a Media or Channel node".into()),
    };
    super::authoring::channel(engine, reference)?;
    Ok(MediaSource {
        media: reference.media,
        channel: Some(reference.channel),
    })
}
