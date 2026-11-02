//! Document identity resolution shared by every source drop intent and public caller.
use super::protocol::{ChannelId, ChannelReference, MediaId};
use crate::app::{AppEngine, AppNode};
use golden_core::node::{Node, NodeUuid};

/// Resolves an actual Media or Channel node. Media selects its first ordered channel;
/// Parameters, folders and other nodes never act as implicit source identities.
pub fn resolve_source_node(engine: &AppEngine, source: NodeUuid) -> Result<ChannelReference, String> {
    let id = engine.node_id_by_uuid(source).ok_or("source node is missing")?;
    let node = engine.nodes.get(id).ok_or("source node is missing")?;
    let reference = match node {
        AppNode::ReferenceMedia(_) => {
            let mut resolver = super::reference::Resolver::new(engine);
            let channel = resolver
                .channels(MediaId(source))?
                .next()
                .ok_or("Reference target has no channels")?;
            ChannelReference {
                media: MediaId(source),
                channel: ChannelId(channel.node_data().meta.uuid),
            }
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
        AppNode::MediaChannel(_) => {
            let parent = node.node_data().parent.ok_or("source channel has no owner")?;
            let Some(AppNode::Media(owner)) = engine.nodes.get(parent) else {
                return Err("source channel owner is not media".into());
            };
            ChannelReference {
                media: MediaId(owner.node_data().meta.uuid),
                channel: ChannelId(source),
            }
        }
        _ => return Err("source must be a Media or Channel node".into()),
    };
    super::authoring::channel(engine, reference)?;
    Ok(reference)
}
