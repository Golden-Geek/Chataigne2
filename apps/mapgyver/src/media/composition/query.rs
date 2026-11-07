//! Immutable bounded editor projections. Selection ownership/defaults stay in the backend.
use super::{authoring, children, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::*,
};
use golden_core::node::{Node, NodeId, NodeUuid};

pub(in crate::media) fn output(engine: &AppEngine, media: NodeId) -> Option<ChannelReference> {
    let node = engine.nodes.get(media)?;
    if !matches!(node, AppNode::Media(node) if node.kind() == MediaKind::Composition) {
        return None;
    }
    let mut child = node.node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id)?;
        if matches!(node, AppNode::MediaChannel(_)) {
            return Some(ChannelReference {
                media: MediaId(engine.nodes.get(media)?.node_data().meta.uuid),
                channel: ChannelId(node.node_data().meta.uuid),
            });
        }
        child = node.node_data().next_sibling;
    }
    None
}

fn owner(engine: &AppEngine, selected: Option<NodeUuid>) -> Option<(ChannelReference, Option<InstanceId>)> {
    let mut next = selected.and_then(|uuid| engine.node_id_by_uuid(uuid));
    while let Some(id) = next {
        let node = engine.nodes.get(id)?;
        let parent = node.node_data().parent;
        if matches!(node, AppNode::MediaChannel(node) if node.kind() == MediaKind::Composition) {
            return output(engine, parent?).map(|reference| (reference, None));
        }
        if matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_)) {
            if let Some(reference) = parent.and_then(|parent| output(engine, parent)) {
                return Some((reference, Some(InstanceId(node.node_data().meta.uuid))));
            }
            if let Some(reference) = output(engine, id) {
                return Some((reference, None));
            }
        }
        next = parent;
    }
    None
}

fn snapshot(engine: &AppEngine, reference: ChannelReference, id: NodeId) -> Result<InstanceSnapshot, String> {
    let node = engine.nodes.get(id).ok_or("Composition child is missing")?;
    Ok(InstanceSnapshot {
        reference: InstanceReference {
            composition: reference,
            instance: InstanceId(node.node_data().meta.uuid),
        },
        label: node.node_data().meta.label.clone(),
        definition: children::definition(engine, id)?,
    })
}

pub(in crate::media) fn inspect(
    engine: &AppEngine,
    query: CompositionEditorQuery,
) -> Result<CompositionEditorPage, String> {
    if query.count == 0 || query.count > 128 {
        return Err("Composition editor pages require 1..128 results".into());
    }
    if (query.locked.is_some() || query.retained.is_some()) && query.document.is_none() {
        return Err("Composition bindings require a document identity".into());
    }
    let document = engine
        .nodes
        .get(engine.root)
        .ok_or("project root is missing")?
        .node_data()
        .meta
        .uuid;
    let followed = owner(engine, query.selected);
    let document_matches = query.document.is_none_or(|expected| expected == document);
    let mut page = CompositionEditorPage {
        at: engine.time,
        parameters: Vec::new(),
        label: None,
        media_types: crate::media::creation::media_choices(),
        document,
        document_matches: document_matches || (query.locked.is_none() && followed.is_some()),
        channel: None,
        composition: None,
        start: query.start,
        total: 0,
        selected: None,
    };
    if !page.document_matches {
        return Ok(page);
    }
    let Some(reference) = query
        .locked
        .or_else(|| followed.map(|(reference, _)| reference))
        .or(query.retained.filter(|_| document_matches))
    else {
        return Ok(page);
    };
    let Some(channel) = crate::media::queries::inspect_channel(engine, reference)? else {
        return Ok(page);
    };
    let (parent, canvas) = authoring::channel(engine, reference)?;
    page.label = engine
        .node_id_by_uuid(reference.media.0)
        .and_then(|id| engine.nodes.get(id))
        .map(|node| node.node_data().meta.label.clone());
    let mut first = None;
    let mut instances = Vec::with_capacity(usize::from(query.count));
    for id in authoring::instance_nodes(engine, parent)? {
        let id = id?;
        first.get_or_insert(id);
        let index = page.total;
        page.total = page.total.checked_add(1).ok_or("too many Composition children")?;
        if index >= query.start && instances.len() < usize::from(query.count) {
            instances.push(snapshot(engine, reference, id)?);
        }
    }
    let selected = followed
        .filter(|(owner, _)| *owner == reference)
        .and_then(|(_, instance)| instance)
        .or(query.instance)
        .and_then(|instance| authoring::instance(engine, parent, instance).ok())
        .or(first);
    page.selected = selected.map(|id| snapshot(engine, reference, id)).transpose()?;
    let mut bound = instances
        .iter()
        .map(|instance| instance.reference.instance)
        .collect::<Vec<_>>();
    if let Some(selected) = &page.selected
        && !bound.contains(&selected.reference.instance)
    {
        bound.push(selected.reference.instance);
    }
    for instance in bound {
        let id = authoring::instance(engine, parent, instance)?;
        page.parameters
            .push(children::placement(engine, id)?.parameter_bindings(engine, instance));
    }
    page.channel = Some(channel);
    page.composition = Some(CompositionSnapshot {
        reference,
        canvas,
        instances,
    });
    Ok(page)
}
