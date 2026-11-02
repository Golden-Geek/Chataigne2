//! Ordered authored read model. Inspection allocates no identities and changes no history.

use golden_core::node::Node;

use super::protocol::*;
use crate::app::{AppEngine, AppNode};

pub(super) fn choices(engine: &AppEngine, query: ChannelChoiceQuery) -> Result<ChannelChoicePage, String> {
    if query.count == 0 || query.count > 128 || query.search.len() > 256 {
        return Err("channel choices require 1..128 results and at most 256 search bytes".into());
    }
    let search = query.search.to_lowercase();
    let mut result = ChannelChoicePage {
        choices: Vec::new(),
        total: 0,
        start: query.start,
    };
    for id in super::folders::entries(engine)? {
        let node = engine.nodes.get(id).ok_or("missing media child")?;
        let AppNode::Media(media) = node else {
            continue;
        };
        let mut child = media.node_data().first_child;
        while let Some(id) = child {
            let node = engine.nodes.get(id).ok_or("missing media child")?;
            child = node.node_data().next_sibling;
            if crate::parameters::is_parameter_folder(node) {
                continue;
            }
            let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
                return Err("media child is not a channel".into());
            };
            child = channel.node_data().next_sibling;
            if matches!(query.scope, ChannelChoiceScope::MatrixFinal)
                && !matches!(
                    channel.definition().source,
                    ChannelSource::Matrix {
                        output: crate::matrix::protocol::MatrixOutput::Final { .. }
                    }
                )
            {
                continue;
            }
            let label = format!("{} / {}", media.node_data().meta.label, channel.node_data().meta.label);
            if !label.to_lowercase().contains(&search) {
                continue;
            }
            let position = result.total;
            result.total = result.total.checked_add(1).ok_or("too many channel choices")?;
            if position >= query.start && result.choices.len() < usize::from(query.count) {
                result.choices.push(ChannelChoice {
                    reference: ChannelReference {
                        media: MediaId(media.node_data().meta.uuid),
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
        let AppNode::Media(media) = node else {
            continue;
        };
        let data = media.node_data();
        let media_id = MediaId(data.meta.uuid);
        let mut snapshot = MediaSnapshot {
            id: media_id,
            label: data.meta.label.clone(),
            media_kind: media.kind(),
            channels: Vec::new(),
        };
        let mut channel = data.first_child;
        while let Some(id) = channel {
            let node = engine.nodes.get(id).ok_or("missing media child")?;
            channel = node.node_data().next_sibling;
            if crate::parameters::is_parameter_folder(node) {
                continue;
            }
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
                definition: super::authoring::effective_definition(engine, node)?,
            });
            channel = data.next_sibling;
        }
        result.media.push(snapshot);
    }
    Ok(result)
}
