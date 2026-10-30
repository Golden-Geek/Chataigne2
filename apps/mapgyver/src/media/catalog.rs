//! Ordered authored read model. Inspection allocates no identities and changes no history.

use golden_core::node::Node;

use super::protocol::*;
use crate::app::{AppEngine, AppNode};

pub(super) fn inspect(engine: &AppEngine) -> Result<MediaManagerSnapshot, String> {
    let manager = super::authoring::manager(engine)?;
    let mut result = MediaManagerSnapshot {
        media: Vec::new(),
        assets: Vec::new(),
    };
    let mut child = engine
        .nodes
        .get(manager)
        .ok_or("missing manager")?
        .node_data()
        .first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("missing manager child")?;
        child = node.node_data().next_sibling;
        if let AppNode::MediaAsset(asset) = node {
            result.assets.push(asset.snapshot());
            continue;
        }
        let AppNode::Media(media) = node else {
            continue;
        };
        let data = media.node_data();
        let media_id = MediaId(data.meta.uuid);
        let mut snapshot = MediaSnapshot {
            id: media_id,
            label: data.meta.label.clone(),
            media_kind: media.kind,
            channels: Vec::new(),
        };
        let mut channel = data.first_child;
        while let Some(id) = channel {
            let Some(AppNode::MediaChannel(node)) = engine.nodes.get(id) else {
                return Err("media children must be channels".into());
            };
            let data = node.node_data();
            snapshot.channels.push(ChannelSnapshot {
                reference: ChannelReference {
                    media: media_id,
                    channel: ChannelId(data.meta.uuid),
                },
                label: data.meta.label.clone(),
                definition: node.definition().clone(),
            });
            channel = data.next_sibling;
        }
        result.media.push(snapshot);
    }
    Ok(result)
}
