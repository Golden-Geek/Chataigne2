//! Ordered authored read model. Inspection allocates no identities and changes no history.

use golden_core::node::Node;

use super::protocol::*;
use crate::app::{AppEngine, AppNode};

pub(super) fn choices(engine: &AppEngine, query: ChannelChoiceQuery) -> Result<ChannelChoicePage, String> {
    if query.count == 0 || query.count > 128 || query.search.len() > 256 {
        return Err("channel choices require 1..128 results and at most 256 search bytes".into());
    }
    let search = query.search.to_lowercase();
    let mut resolver = super::reference::Resolver::new(engine);
    let mut result = ChannelChoicePage {
        choices: Vec::new(),
        total: 0,
        start: query.start,
    };
    for id in super::folders::entries(engine)? {
        let node = engine.nodes.get(id).ok_or("missing media child")?;
        if !matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_)) {
            continue;
        }
        let media_id = MediaId(node.node_data().meta.uuid);
        for channel in resolver.channels(media_id)? {
            if matches!(query.scope, ChannelChoiceScope::MatrixFinal) && !channel.is_matrix_final() {
                continue;
            }
            let label = format!("{} / {}", node.node_data().meta.label, channel.node_data().meta.label);
            if !label.to_lowercase().contains(&search) {
                continue;
            }
            let position = result.total;
            result.total = result.total.checked_add(1).ok_or("too many channel choices")?;
            if position >= query.start && result.choices.len() < usize::from(query.count) {
                result.choices.push(ChannelChoice {
                    reference: ChannelReference {
                        media: media_id,
                        channel: ChannelId(channel.node_data().meta.uuid),
                    },
                    label,
                });
            }
        }
    }
    Ok(result)
}

pub(super) fn inspect(engine: &AppEngine) -> Result<MediaManagerSnapshot, String> {
    let mut resolver = super::reference::Resolver::new(engine);
    let mut result = MediaManagerSnapshot {
        media: Vec::new(),
        assets: Vec::new(),
    };
    for id in super::folders::entries(engine)? {
        let node = engine.nodes.get(id).ok_or("missing manager child")?;
        if let AppNode::MediaAsset(asset) = node {
            result.assets.push(asset.snapshot());
            continue;
        }
        let kind = match node {
            AppNode::Media(media) => media.kind(),
            AppNode::ReferenceMedia(_) => MediaKind::Reference,
            _ => continue,
        };
        let data = node.node_data();
        let media_id = MediaId(data.meta.uuid);
        let mut snapshot = MediaSnapshot {
            id: media_id,
            label: data.meta.label.clone(),
            media_kind: kind,
            channels: Vec::new(),
        };
        for channel in resolver.channels(media_id)? {
            let data = channel.node_data();
            snapshot.channels.push(ChannelSnapshot {
                reference: ChannelReference {
                    media: media_id,
                    channel: ChannelId(data.meta.uuid),
                },
                label: data.meta.label.clone(),
                definition: match node {
                    AppNode::ReferenceMedia(reference) => reference.definition(engine, ChannelId(data.meta.uuid)),
                    _ => super::authoring::effective_definition(engine, channel)?,
                },
            });
        }
        result.media.push(snapshot);
    }
    Ok(result)
}
