use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::*,
};
use golden_core::node::{Node, NodeUuid};

fn owner(engine: &AppEngine, selected: Option<NodeUuid>) -> Option<SpatialEditorTarget> {
    let mut next = selected.and_then(|uuid| engine.node_id_by_uuid(uuid));
    for _ in 0..64 {
        let id = next?;
        let node = engine.nodes.get(id)?;
        let parent = node.node_data().parent;
        // An owned media is the outer Composition's editable instance. Its own channel
        // remains an explicit entry into its nested domain, matching Composition selection.
        if matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_))
            && let Some(reference) = parent.and_then(|id| super::super::composition::query::output(engine, id))
        {
            return Some(SpatialEditorTarget::Composition { reference });
        }
        if let AppNode::Media(node) = node {
            match node.kind() {
                MediaKind::Matrix => {
                    return Some(SpatialEditorTarget::Matrix {
                        media: MediaId(node.node_data().meta.uuid),
                    });
                }
                MediaKind::Composition => {
                    return super::super::composition::query::output(engine, id)
                        .map(|reference| SpatialEditorTarget::Composition { reference });
                }
                _ => {}
            }
        }
        if let AppNode::MediaChannel(node) = node {
            match node.kind() {
                MediaKind::Composition => {
                    return parent
                        .and_then(|id| super::super::composition::query::output(engine, id))
                        .map(|reference| SpatialEditorTarget::Composition { reference });
                }
                MediaKind::Matrix => {
                    return parent
                        .and_then(|id| engine.nodes.get(id))
                        .map(|node| SpatialEditorTarget::Matrix {
                            media: MediaId(node.node_data().meta.uuid),
                        });
                }
                _ => {}
            }
        }
        next = parent;
    }
    None
}

pub(in crate::media) fn inspect(engine: &AppEngine, query: SpatialEditorQuery) -> Result<SpatialEditorBinding, String> {
    if (query.locked.is_some() || query.retained.is_some()) && query.document.is_none() {
        return Err("Spatial bindings require a document identity".into());
    }
    let document = engine
        .nodes
        .get(engine.root)
        .ok_or("project root is missing")?
        .node_data()
        .meta
        .uuid;
    let followed = owner(engine, query.selected);
    let matches = query.document.is_none_or(|expected| expected == document);
    let mut binding = SpatialEditorBinding {
        document,
        document_matches: matches || (query.locked.is_none() && followed.is_some()),
        target: None,
        present: false,
        label: None,
    };
    if !binding.document_matches {
        return Ok(binding);
    }
    // A nested media also represents its outer instance. Background owner selection
    // keeps the current edited domain instead of navigating to that outer placement.
    let retained_owner = query.retained.filter(|target| {
        matches
            && query.selected.is_some_and(|selected| match target {
                SpatialEditorTarget::Composition { reference } => {
                    selected == reference.media.0 || selected == reference.channel.0
                }
                SpatialEditorTarget::Matrix { media } => selected == media.0,
            })
    });
    let Some(target) = query
        .locked
        .or(retained_owner)
        .or(followed)
        .or(query.retained.filter(|_| matches))
    else {
        return Ok(binding);
    };
    binding.target = Some(target);
    let (media, kind) = match target {
        SpatialEditorTarget::Composition { reference } => (reference.media, MediaKind::Composition),
        SpatialEditorTarget::Matrix { media } => (media, MediaKind::Matrix),
    };
    let Some(node) = engine.node_id_by_uuid(media.0).and_then(|id| engine.nodes.get(id)) else {
        return Ok(binding);
    };
    if !matches!(node, AppNode::Media(node) if node.kind() == kind) {
        return Err("Spatial binding does not name its declared media kind".into());
    }
    if let SpatialEditorTarget::Composition { reference } = target {
        let Some(channel) = crate::media::queries::inspect_channel(engine, reference)? else {
            return Ok(binding);
        };
        if !matches!(channel.definition.source, ChannelSource::Composition { .. }) {
            return Err("Spatial Composition binding must name a Composition channel".into());
        }
    }
    binding.present = true;
    binding.label = Some(node.node_data().meta.label.clone());
    Ok(binding)
}
