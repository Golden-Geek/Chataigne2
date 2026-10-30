use std::collections::HashSet;

use golden_core::app::add_default_project_nodes;
use golden_core::edit::{Edit, NodeTree};
use golden_core::node::{Node, NodeId, NodeMetaPatch};
use serde_json::Value;

use super::nodes::{ChannelNode, MediaManagerNode, MediaNode};
use super::protocol::*;
use crate::app::{AppEngine, AppNode};

const MANAGER_DECL: &str = "mapgyver_media_manager";

pub fn initialize(engine: &mut AppEngine) {
    add_default_project_nodes(engine);
    engine.add_node(MediaManagerNode::new().into(), None);
}

pub(super) fn manager(engine: &AppEngine) -> Result<NodeId, String> {
    let mut child = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child);
    let mut found = None;
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("missing root child")?;
        if node.node_data().meta.decl_id.0 == MANAGER_DECL && matches!(node, AppNode::MediaManager(_)) {
            if found.is_some() {
                return Err("duplicate Media Manager".into());
            }
            found = Some(id);
        }
        child = node.node_data().next_sibling;
    }
    found.ok_or_else(|| "Media Manager is missing".into())
}

fn media(engine: &AppEngine, media: MediaId) -> Result<(NodeId, MediaKind), String> {
    let id = engine.node_id_by_uuid(media.0).ok_or("media reference is missing")?;
    let Some(AppNode::Media(node)) = engine.nodes.get(id) else {
        return Err("reference is not media".into());
    };
    if node.node_data().parent != Some(manager(engine)?) {
        return Err("media is outside its manager".into());
    }
    Ok((id, node.kind))
}

fn channel_id(engine: &AppEngine, reference: ChannelReference) -> Result<NodeId, String> {
    let (parent, kind) = media(engine, reference.media)?;
    let id = engine
        .node_id_by_uuid(reference.channel.0)
        .ok_or("channel reference is missing")?;
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(id) else {
        return Err("reference is not a channel".into());
    };
    if node.node_data().parent != Some(parent) {
        return Err("channel belongs to another media".into());
    }
    if node.definition().media_kind() != kind {
        return Err("channel source does not match its media kind".into());
    }
    Ok(id)
}

pub fn channel(engine: &AppEngine, reference: ChannelReference) -> Result<ChannelSnapshot, String> {
    let id = channel_id(engine, reference)?;
    let AppNode::MediaChannel(node) = engine.nodes.get(id).expect("validated channel") else {
        unreachable!()
    };
    Ok(ChannelSnapshot {
        reference,
        label: node.node_data().meta.label.clone(),
        definition: node.definition().clone(),
    })
}

fn label(engine: &AppEngine, parent: NodeId, requested: Option<String>, fallback: &str) -> Result<String, String> {
    label_with_reserved(engine, parent, requested, fallback, &[])
}

pub(super) fn label_with_reserved(
    engine: &AppEngine,
    parent: NodeId,
    requested: Option<String>,
    fallback: &str,
    reserved: &[&str],
) -> Result<String, String> {
    let requested = requested.unwrap_or_else(|| fallback.into());
    let requested = requested.trim();
    if requested.is_empty() || requested.len() > 255 || requested.contains('\0') {
        return Err("label must contain 1..255 UTF-8 bytes without NUL".into());
    }
    let mut labels = HashSet::new();
    labels.extend(reserved.iter().copied());
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("missing sibling")?;
        labels.insert(node.node_data().meta.label.as_str());
        child = node.node_data().next_sibling;
    }
    if !labels.contains(requested) {
        return Ok(requested.into());
    }
    for index in 2..=labels.len() + 2 {
        let candidate = format!("{requested} {index}");
        if !labels.contains(candidate.as_str()) && candidate.len() <= 255 {
            return Ok(candidate);
        }
    }
    Err("cannot allocate a unique label within the byte limit".into())
}

fn rename(engine: &mut AppEngine, node: NodeId, label: String) -> Result<(), String> {
    let data = engine.nodes.get(node).ok_or("rename target is missing")?.node_data();
    if label.trim() == data.meta.label {
        return Ok(());
    }
    let parent = data.parent.ok_or("rename target is detached")?;
    let label = self::label(engine, parent, Some(label), "")?;
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            label: Some(label),
            ..Default::default()
        },
    });
    Ok(())
}

pub fn apply_operation(engine: &mut AppEngine, operation: &str, payload: Value) -> Result<Value, String> {
    if operation != MEDIA_OPERATION {
        return Err("unsupported media operation".into());
    }
    let edit: MediaEdit = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let pending_before = engine.edits.pending.len();
    let result = match edit {
        MediaEdit::ImportPicture { locator, target, label } => {
            super::importing::picture(engine, locator, target, label)?
        }
        MediaEdit::InspectManager => MediaEditResult::Manager {
            snapshot: super::catalog::inspect(engine)?,
        },
        MediaEdit::ImportAsset {
            locator,
            label: requested,
        } => {
            let parent = manager(engine)?;
            let fallback = locator_label(&locator);
            let node = super::assets::AssetNode::new(label(engine, parent, requested, fallback)?, Some(locator));
            let asset = node.id();
            engine.edits.push(Edit::AddNodeTree {
                parent,
                prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
                tree: NodeTree::new(node),
            });
            MediaEditResult::AssetCreated { asset }
        }
        MediaEdit::RelinkAsset { asset, locator } => {
            let id = super::assets::node_id(engine, asset)?;
            engine.edits.push(Edit::CallNodeMutation {
                node: id,
                needs_tree_snapshot: false,
                callback: Box::new(move |node, _| {
                    let node = node
                        .as_any_mut()
                        .downcast_mut::<super::AssetNode>()
                        .ok_or("asset type changed")?;
                    node.set_locator(Some(locator));
                    Ok(())
                }),
            });
            MediaEditResult::Changed
        }
        MediaEdit::RemoveAsset { asset } => {
            let id = super::assets::node_id(engine, asset)?;
            engine.edits.push(Edit::RemoveNode { node: id });
            MediaEditResult::Changed
        }
        MediaEdit::InspectAsset { asset } => MediaEditResult::Asset {
            record: super::assets::asset(engine, asset)?,
        },
        MediaEdit::CreateMedia {
            media_kind,
            label: requested,
        } => {
            let parent = manager(engine)?;
            let fallback = match media_kind {
                MediaKind::Picture => "Picture",
                MediaKind::SolidColor => "Solid Color",
                MediaKind::SharedTexture => "Shared Texture",
            };
            let (tree, reference) = media_tree(
                label(engine, parent, requested, fallback)?,
                ChannelDefinition::for_media(media_kind),
            );
            engine.edits.push(Edit::AddNodeTree {
                tree,
                parent,
                prev_sibling: engine.nodes.get(parent).and_then(|n| n.node_data().last_child),
            });
            MediaEditResult::Created { reference }
        }
        MediaEdit::AddChannel {
            media: owner,
            label: requested,
        } => {
            let (parent, kind) = media(engine, owner)?;
            let channel = ChannelNode::new(
                label(engine, parent, requested, "Channel")?,
                ChannelDefinition::for_media(kind),
            );
            let reference = ChannelReference {
                media: owner,
                channel: ChannelId(channel.node_data().meta.uuid),
            };
            engine.edits.push(Edit::AddNodeTree {
                tree: NodeTree::new(channel),
                parent,
                prev_sibling: engine.nodes.get(parent).and_then(|n| n.node_data().last_child),
            });
            MediaEditResult::Created { reference }
        }
        MediaEdit::ConfigureChannel { reference, definition } => {
            configure_channel(engine, reference, definition)?;
            MediaEditResult::Changed
        }
        MediaEdit::RenameMedia { media: owner, label } => {
            let (id, _) = media(engine, owner)?;
            rename(engine, id, label)?;
            MediaEditResult::Changed
        }
        MediaEdit::RenameChannel { reference, label } => {
            let id = channel_id(engine, reference)?;
            rename(engine, id, label)?;
            MediaEditResult::Changed
        }
        MediaEdit::ReorderChannel { reference, after } => {
            let node = channel_id(engine, reference)?;
            let (parent, _) = media(engine, reference.media)?;
            let previous = after
                .map(|channel| channel_id(engine, ChannelReference { channel, ..reference }))
                .transpose()?;
            if previous == Some(node) {
                return Err("channel cannot follow itself".into());
            }
            engine.edits.push(Edit::MoveNode {
                node,
                new_parent: parent,
                new_prev_sibling: previous,
            });
            MediaEditResult::Changed
        }
        MediaEdit::RemoveChannel { reference } => {
            let node = channel_id(engine, reference)?;
            let (parent, _) = media(engine, reference.media)?;
            let data = engine.nodes.get(parent).expect("validated media").node_data();
            if data.first_child == Some(node) && data.last_child == Some(node) {
                return Err("media must retain at least one channel".into());
            }
            engine.edits.push(Edit::RemoveNode { node });
            MediaEditResult::Changed
        }
        MediaEdit::RemoveMedia { media: owner } => {
            let (node, _) = media(engine, owner)?;
            engine.edits.push(Edit::RemoveNode { node });
            MediaEditResult::Changed
        }
        MediaEdit::InspectChannel { reference } => MediaEditResult::Channel {
            snapshot: channel(engine, reference)?,
        },
    };
    if engine.edits.pending.len() != pending_before {
        // Published inside the atomic edit boundary, including script/headless callers.
        // The app namespace is also its retained authored-media notification topic.
        let event = golden_core::events::CustomEvent::from_latest_payload(MEDIA_NAMESPACE, None, &())
            .map_err(|error| error.to_string())?;
        engine.edits.push(Edit::EmitCustomEvent { event });
    }
    serde_json::to_value(result).map_err(|error| error.to_string())
}

pub(super) fn locator_label(locator: &AssetLocator) -> &str {
    match locator {
        AssetLocator::ProjectRelative { path } => path.as_str(),
        AssetLocator::Absolute { path } => path.as_str(),
    }
    .rsplit(['/', '\\'])
    .find(|segment| !segment.is_empty())
    .unwrap_or("Asset")
}

pub(super) fn media_tree(label: String, definition: ChannelDefinition) -> (NodeTree, ChannelReference) {
    let media = MediaNode::new(label, definition.media_kind());
    let channel = ChannelNode::new("Channel 1".into(), definition);
    let reference = ChannelReference {
        media: MediaId(media.node_data().meta.uuid),
        channel: ChannelId(channel.node_data().meta.uuid),
    };
    (NodeTree::new(media).with_child(NodeTree::new(channel)), reference)
}

pub(super) fn configure_channel(
    engine: &mut AppEngine,
    reference: ChannelReference,
    definition: ChannelDefinition,
) -> Result<(), String> {
    let id = channel_id(engine, reference)?;
    let (_, kind) = media(engine, reference.media)?;
    definition.validate()?;
    if definition.media_kind() != kind {
        return Err("channel source does not match its media kind".into());
    }
    engine.edits.push(Edit::CallNodeMutation {
        node: id,
        needs_tree_snapshot: false,
        callback: Box::new(move |node, _| {
            let channel = node
                .as_any_mut()
                .downcast_mut::<ChannelNode>()
                .ok_or("channel type changed")?;
            channel.set_definition(definition);
            Ok(())
        }),
    });
    Ok(())
}

pub fn validate_project(engine: &AppEngine) -> Result<(), String> {
    // Whole-project validation belongs to detached load/preparation, never the frame loop.
    let manager = manager(engine)?;
    for (_, node) in engine.nodes.iter() {
        match node {
            AppNode::MediaAsset(node) => {
                if node.node_data().parent != Some(manager) {
                    return Err("asset is outside its catalog".into());
                }
            }
            AppNode::Media(node) => {
                if node.node_data().parent != Some(manager) {
                    return Err("media is outside its manager".into());
                }
                let mut child = node.node_data().first_child;
                let mut count = 0;
                while let Some(id) = child {
                    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
                        return Err("media children must be channels".into());
                    };
                    count += 1;
                    child = channel.node_data().next_sibling;
                }
                if count == 0 {
                    return Err("media must retain at least one channel".into());
                }
            }
            AppNode::MediaChannel(node) => {
                node.definition().validate()?;
                let parent = node
                    .node_data()
                    .parent
                    .and_then(|id| engine.nodes.get(id))
                    .ok_or("channel owner is missing")?;
                let AppNode::Media(parent) = parent else {
                    return Err("channel owner is not media".into());
                };
                if parent.kind != node.definition().media_kind() {
                    return Err("channel source does not match its media kind".into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}
