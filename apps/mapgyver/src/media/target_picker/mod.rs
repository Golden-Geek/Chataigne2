//! Bounded Inspector choices. Browsing policy is stricter than explicit nested source bindings.
pub(super) mod protocol;
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use protocol::*;

const SCAN_LIMIT: usize = 256;
struct Binding {
    kind: TargetPickerKind,
    parameter: NodeId,
    media: Option<super::protocol::MediaId>,
    automatic: bool,
}

fn binding(engine: &AppEngine, uuid: NodeUuid) -> Result<Binding, String> {
    let parameter = engine
        .node_id_by_uuid(uuid)
        .ok_or("Target Parameter no longer exists")?;
    let owner = engine
        .nodes
        .get(parameter)
        .and_then(|node| node.node_data().parent)
        .and_then(|folder| engine.nodes.get(folder))
        .and_then(|node| node.node_data().parent)
        .and_then(|owner| engine.nodes.get(owner))
        .ok_or("Target Parameter owner is missing")?;
    let (media, channel, target, automatic) = match owner {
        AppNode::ReferenceMedia(node) => (node.target_parameter(), node.channel_parameter(), node.target(), true),
        AppNode::Surface(node) => {
            let [(media, _), (channel, _)] = node.source_parameter_values(None);
            let target = match engine
                .nodes
                .get(media)
                .and_then(Node::engine_param_snapshot)
                .map(|param| param.value)
            {
                Some(ParamValue::Reference(value)) if !value.uuid.0.is_nil() => {
                    Some(super::protocol::MediaId(value.uuid))
                }
                _ => None,
            };
            (media, channel, target, false)
        }
        _ => return Err("Parameter has no Media/Channel picker".into()),
    };
    let kind = if parameter == media {
        TargetPickerKind::Media
    } else if parameter == channel {
        TargetPickerKind::Channel
    } else {
        return Err("Parameter has no Media/Channel picker".into());
    };
    Ok(Binding {
        kind,
        parameter,
        media: target,
        automatic,
    })
}

/// Only organizing folders may bridge a catalog entry to Medias. Never descend into media.
fn catalog_parent(engine: &AppEngine, mut id: NodeId, manager: NodeId) -> bool {
    for _ in 0..64 {
        if id == manager {
            return true;
        }
        let Some(AppNode::MediaFolder(folder)) = engine.nodes.get(id) else {
            return false;
        };
        let Some(parent) = folder.node_data().parent else {
            return false;
        };
        id = parent;
    }
    false
}

fn catalog_entry(engine: &AppEngine, id: NodeId, manager: NodeId) -> bool {
    engine.nodes.get(id).is_some_and(|node| {
        matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_))
            && node
                .node_data()
                .parent
                .is_some_and(|parent| catalog_parent(engine, parent, manager))
    })
}

fn current(engine: &AppEngine, binding: &Binding) -> Option<TargetPickerCurrent> {
    let ParamValue::Reference(value) = engine.nodes.get(binding.parameter)?.engine_param_snapshot()?.value else {
        return None;
    };
    if value.uuid.0.is_nil() {
        return None;
    }
    let node = engine.node_id_by_uuid(value.uuid).and_then(|id| engine.nodes.get(id));
    Some(TargetPickerCurrent {
        id: value.uuid,
        label: node
            .map(|node| node.node_data().meta.label.clone())
            .or(value.cached_name)
            .unwrap_or_else(|| value.uuid.0.to_string()),
        present: node.is_some()
            && match binding.kind {
                TargetPickerKind::Media => true,
                TargetPickerKind::Channel => binding.media.is_some_and(|media| {
                    super::authoring::channel(
                        engine,
                        super::protocol::ChannelReference {
                            media,
                            channel: super::protocol::ChannelId(value.uuid),
                        },
                    )
                    .is_ok()
                }),
            },
    })
}

pub(super) fn inspect(engine: &AppEngine, query: TargetPickerQuery) -> Result<TargetPickerPage, String> {
    if query.limit > 64 {
        return Err("Target picker pages admit at most64 choices".into());
    }
    let binding = binding(engine, query.parameter)?;
    let manager = super::authoring::manager(engine)?;
    let owner = match binding.kind {
        TargetPickerKind::Media => match query.folder {
            None => Some(manager),
            Some(folder) => {
                let id = engine.node_id_by_uuid(folder).ok_or("Catalog folder is missing")?;
                if !catalog_parent(engine, id, manager) {
                    return Err("Folder is outside the Media catalog".into());
                }
                Some(id)
            }
        },
        TargetPickerKind::Channel => {
            if query.folder.is_some() {
                return Err("Channel choices belong to the selected Media".into());
            }
            binding
                .media
                .map(|media| {
                    if engine.node_id_by_uuid(media.0).is_none() {
                        return Ok(None);
                    }
                    super::authoring::media(engine, media).map(|(id, _)| Some(id))
                })
                .transpose()?
                .flatten()
        }
    };
    let location = owner
        .and_then(|id| engine.nodes.get(id))
        .map_or("Channels", |node| node.node_data().meta.label.as_str())
        .to_owned();
    let parent = query
        .folder
        .and(owner)
        .and_then(|id| engine.nodes.get(id))
        .and_then(|node| node.node_data().parent)
        .filter(|id| *id != manager)
        .and_then(|id| engine.nodes.get(id))
        .map(|node| node.node_data().meta.uuid);
    let mut page = TargetPickerPage {
        kind: binding.kind,
        location,
        folder: query.folder,
        parent,
        before: None,
        after: None,
        current: current(engine, &binding),
        empty_label: if binding.automatic && matches!(binding.kind, TargetPickerKind::Channel) {
            "First available"
        } else {
            "None"
        }
        .into(),
        entries: vec![],
    };
    let Some(owner) = owner else {
        return Ok(page);
    };
    if query.limit == 0 {
        return Ok(page);
    }
    let owner_node = engine.nodes.get(owner).ok_or("Picker owner is missing")?;
    let mut next = if let Some(cursor) = query.cursor {
        let node = engine
            .node_id_by_uuid(cursor)
            .and_then(|id| engine.nodes.get(id))
            .ok_or("Picker cursor no longer exists")?;
        if node.node_data().parent != Some(owner) {
            return Err("Picker cursor belongs to another scope".into());
        }
        if query.backwards {
            node.node_data().prev_sibling
        } else {
            node.node_data().next_sibling
        }
    } else if query.backwards {
        owner_node.node_data().last_child
    } else {
        owner_node.node_data().first_child
    };
    let mut visited = Vec::with_capacity(SCAN_LIMIT);
    for _ in 0..SCAN_LIMIT {
        let Some(id) = next else {
            break;
        };
        let node = engine.nodes.get(id).ok_or("Picker sibling is missing")?;
        next = if query.backwards {
            node.node_data().prev_sibling
        } else {
            node.node_data().next_sibling
        };
        visited.push(id);
        let allowed = match binding.kind {
            TargetPickerKind::Media => matches!(
                node,
                AppNode::MediaFolder(_) | AppNode::Media(_) | AppNode::ReferenceMedia(_)
            ),
            TargetPickerKind::Channel => match node {
                AppNode::MediaChannel(channel) => crate::matrix::outputs::exposed(engine, channel),
                AppNode::ReferenceOutput(_) => true,
                _ => false,
            },
        };
        if allowed {
            page.entries.push(TargetPickerEntry {
                id: node.node_data().meta.uuid,
                label: node.node_data().meta.label.clone(),
                folder: matches!(node, AppNode::MediaFolder(_)),
            });
        }
        if page.entries.len() == usize::from(query.limit) {
            break;
        }
    }
    if query.backwards {
        visited.reverse();
        page.entries.reverse();
    }
    if let (Some(first), Some(last)) = (visited.first(), visited.last()) {
        let first = engine.nodes.get(*first).unwrap().node_data();
        let last = engine.nodes.get(*last).unwrap().node_data();
        page.before = first.prev_sibling.map(|_| first.meta.uuid);
        page.after = last.next_sibling.map(|_| last.meta.uuid);
    }
    Ok(page)
}

pub(super) fn select(engine: &mut AppEngine, parameter: NodeUuid, target: Option<NodeUuid>) -> Result<(), String> {
    let binding = binding(engine, parameter)?;
    if let Some(target) = target {
        let id = engine.node_id_by_uuid(target).ok_or("Choice is no longer available")?;
        match binding.kind {
            TargetPickerKind::Media => {
                if !catalog_entry(engine, id, super::authoring::manager(engine)?) {
                    return Err("Choice is outside the Media catalog".into());
                }
            }
            TargetPickerKind::Channel => {
                let media = binding.media.ok_or("Select a Media first")?;
                super::authoring::channel(
                    engine,
                    super::protocol::ChannelReference {
                        media,
                        channel: super::protocol::ChannelId(target),
                    },
                )?;
            }
        }
    }
    engine.edits.push(Edit::SetParam {
        node: binding.parameter,
        value: ParamValue::Reference(target.map_or_else(NodeReference::empty, NodeReference::new)),
        behaviour: ParameterEventBehaviour::Append,
    });
    Ok(())
}

#[cfg(test)]
mod tests;
