//! Typed configuration snapshots; capture/recall never copy Cell contents or playback requests.
mod nodes;
pub(crate) mod protocol;
use super::{document::Document, protocol::*};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::protocol::{MediaEditResult, MediaId},
};
use golden_core::{
    edit::{Edit, NodeTree, UserItemTreeInsertion},
    node::{Node, NodeId, NodeMetaPatch},
};
pub(crate) use nodes::{PresetCollectionNode, PresetEntryNode, PresetNode};
use std::collections::{HashMap, HashSet};
pub(crate) const MAX_PRESETS: usize = 128;

fn collection(engine: &AppEngine, media: MediaId) -> Result<NodeId, String> {
    let (id, kind) = crate::media::authoring::media(engine, media)?;
    if kind != crate::media::protocol::MediaKind::Matrix {
        return Err("configuration presets require a Matrix".into());
    }
    let AppNode::Media(node) = engine.nodes.get(id).unwrap() else {
        unreachable!()
    };
    node.preset_container()
        .ok_or_else(|| "Matrix configuration collection is not ready".into())
}
fn children(engine: &AppEngine, parent: NodeId) -> Vec<NodeId> {
    let mut child = engine.nodes.get(parent).unwrap().node_data().first_child;
    let mut result = Vec::new();
    while let Some(id) = child {
        let node = engine.nodes.get(id).unwrap();
        child = node.node_data().next_sibling;
        result.push(id);
    }
    result
}
pub(crate) fn summaries(engine: &AppEngine, media: MediaId) -> Result<Vec<MatrixPresetSummary>, String> {
    let parent = collection(engine, media)?;
    let ids = children(engine, parent);
    if ids.len() > MAX_PRESETS {
        return Err("Matrix supports at most128 configuration presets".into());
    }
    ids.into_iter()
        .map(|id| {
            let node = engine.nodes.get(id).unwrap();
            if !matches!(node, AppNode::MatrixPreset(_)) {
                return Err("invalid configuration child".into());
            }
            Ok(MatrixPresetSummary {
                id: node.node_data().meta.uuid,
                label: node.node_data().meta.label.clone(),
            })
        })
        .collect()
}
fn resolve(engine: &AppEngine, parent: NodeId, preset: golden_core::node::NodeUuid) -> Result<NodeId, String> {
    let id = engine
        .node_id_by_uuid(preset)
        .ok_or("configuration preset is missing")?;
    let node = engine.nodes.get(id).unwrap();
    if node.node_data().parent != Some(parent) || !matches!(node, AppNode::MatrixPreset(_)) {
        return Err("configuration belongs to another Matrix".into());
    }
    Ok(id)
}
fn capture(engine: &AppEngine, document: &Document) -> Vec<NodeTree> {
    let rows = document.snapshot.items.iter().filter_map(|item| match item.id {
        MatrixItemId::Layer { id } => Some(document.items[&MatrixItemId::Layer { id }]),
        _ => None,
    });
    rows.chain(
        document
            .snapshot
            .columns
            .iter()
            .map(|column| document.columns[&column.id]),
    )
    .enumerate()
    .map(|(order, id)| {
        let meta = &engine.nodes.get(id).unwrap().node_data().meta;
        NodeTree::new(PresetEntryNode::from_member(
            meta.uuid,
            order,
            meta.enabled,
            meta.label.clone(),
        ))
        .as_user_item()
    })
    .collect()
}
pub(crate) fn apply(engine: &mut AppEngine, media: MediaId, edit: MatrixPresetEdit) -> Result<MediaEditResult, String> {
    let parent = collection(engine, media)?;
    if let MatrixPresetEdit::Capture { label } = edit {
        if children(engine, parent).len() >= MAX_PRESETS {
            return Err("Matrix supports at most128 configuration presets".into());
        }
        let document = Document::load(engine, media)?;
        let mut preset = PresetNode::new();
        preset.node_data_mut().meta.label = labels::unique(engine, parent, label, "Configuration", &[])?;
        let uuid = preset.node_data().meta.uuid;
        let mut tree = NodeTree::new(preset);
        tree.children = capture(engine, &document);
        engine.edits.push(Edit::AddUserItemTree {
            parent,
            prev_sibling: engine.nodes.get(parent).unwrap().node_data().last_child,
            tree,
        });
        return Ok(MediaEditResult::MatrixPresetCreated { preset: uuid });
    }
    let preset = match &edit {
        MatrixPresetEdit::Update { preset }
        | MatrixPresetEdit::Recall { preset }
        | MatrixPresetEdit::Rename { preset, .. }
        | MatrixPresetEdit::Delete { preset } => *preset,
        _ => unreachable!(),
    };
    let id = resolve(engine, parent, preset)?;
    match edit {
        MatrixPresetEdit::Update { .. } => {
            let document = Document::load(engine, media)?;
            let entries = capture(engine, &document);
            for child in children(engine, id) {
                if matches!(engine.nodes.get(child), Some(AppNode::MatrixPresetEntry(_))) {
                    engine.edits.push(Edit::RemoveNode { node: child });
                }
            }
            engine.edits.push(Edit::AddUserItemTrees {
                items: entries
                    .into_iter()
                    .map(|tree| UserItemTreeInsertion { parent: id, tree })
                    .collect(),
            });
        }
        MatrixPresetEdit::Recall { .. } => recall(engine, media, id)?,
        MatrixPresetEdit::Rename { label, .. } => labels::rename(engine, id, label)?,
        MatrixPresetEdit::Delete { .. } => engine.edits.push(Edit::RemoveNode { node: id }),
        _ => unreachable!(),
    }
    Ok(MediaEditResult::Changed)
}
fn recall(engine: &mut AppEngine, media: MediaId, preset: NodeId) -> Result<(), String> {
    let document = Document::load(engine, media)?;
    let allowed: HashSet<_> = document
        .snapshot
        .items
        .iter()
        .filter_map(|item| match item.id {
            MatrixItemId::Layer { .. } => Some(document.items[&item.id]),
            _ => None,
        })
        .chain(document.columns.values().copied())
        .collect();
    let mut members = HashMap::new();
    for id in children(engine, preset) {
        let Some(AppNode::MatrixPresetEntry(node)) = engine.nodes.get(id) else {
            return Err("invalid configuration entry".into());
        };
        let (target, order, enabled) = node.values();
        let Some(id) = engine.node_id_by_uuid(target) else {
            continue;
        };
        if !allowed.contains(&id) {
            return Err("configuration member belongs to another Matrix or is not a Layer/Column".into());
        }
        if members.insert(id, (order, enabled)).is_some() {
            return Err("configuration contains a duplicate member".into());
        }
    }
    let parents: HashSet<_> = members
        .keys()
        .map(|id| engine.nodes.get(*id).unwrap().node_data().parent.unwrap())
        .collect();
    let mut edits = Vec::new();
    for parent in parents {
        let siblings = children(engine, parent);
        let mut ordered: Vec<_> = siblings.iter().filter(|id| members.contains_key(id)).copied().collect();
        ordered.sort_by_key(|id| (members[id].0, engine.nodes.get(*id).unwrap().node_data().meta.uuid.0));
        let mut ordered = ordered.into_iter();
        let mut previous = None;
        for current in siblings {
            let node = if members.contains_key(&current) {
                ordered.next().unwrap()
            } else {
                current
            };
            if members.contains_key(&node) {
                edits.push(Edit::MoveNode {
                    node,
                    new_parent: parent,
                    new_prev_sibling: previous,
                });
                let enabled = members[&node].1;
                if engine.nodes.get(node).unwrap().node_data().meta.enabled != enabled {
                    edits.push(Edit::PatchMeta {
                        node,
                        patch: NodeMetaPatch {
                            enabled: Some(enabled),
                            ..Default::default()
                        },
                    });
                }
            }
            previous = Some(node);
        }
    }
    for edit in edits {
        engine.edits.push(edit);
    }
    Ok(())
}
