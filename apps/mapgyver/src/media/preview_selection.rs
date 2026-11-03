//! Backend ownership/default resolution for a bounded, immutable panel read.
use super::protocol::*;
use crate::app::{AppEngine, AppNode};
use golden_core::node::Node;

pub(super) fn inspect(engine: &AppEngine, query: MediaPreviewQuery) -> Result<MediaPreviewPage, String> {
    if query.count == 0 || query.count > 128 {
        return Err("media preview pages require 1..128 results".into());
    }
    let document = engine
        .nodes
        .get(engine.root)
        .ok_or("project root is missing")?
        .node_data()
        .meta
        .uuid;
    let mut result = MediaPreviewPage {
        document,
        document_matches: query.document.is_none_or(|expected| expected == document),
        media: None,
        label: None,
        channels: None,
        selected: None,
    };
    if !result.document_matches {
        return Ok(result);
    }
    let mut next = query.selected.and_then(|uuid| engine.node_id_by_uuid(uuid));
    let mut descendant_channel = None;
    while let Some(id) = next {
        let node = engine.nodes.get(id).ok_or("selected ancestor is missing")?;
        if matches!(node, AppNode::MediaChannel(_)) && descendant_channel.is_none() {
            descendant_channel = Some(ChannelId(node.node_data().meta.uuid));
        }
        if matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_)) {
            let media = MediaId(node.node_data().meta.uuid);
            super::authoring::media(engine, media)?;
            let channel = |channel| super::authoring::channel(engine, ChannelReference { media, channel }).ok();
            result.selected = query
                .channel
                .and_then(channel)
                .or_else(|| descendant_channel.and_then(channel))
                .or_else(|| {
                    super::reference::Resolver::new(engine)
                        .channels(media)
                        .ok()?
                        .next()
                        .and_then(|first| channel(ChannelId(first.node_data().meta.uuid)))
                });
            result.media = Some(media);
            result.label = Some(node.node_data().meta.label.clone());
            result.channels = Some(super::catalog::channels(
                engine,
                MediaChannelsQuery {
                    media,
                    start: query.start,
                    count: query.count,
                },
            )?);
            break;
        }
        next = node.node_data().parent;
    }
    Ok(result)
}
