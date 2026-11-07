//! App hierarchy policy shared by manager, generic Outliner, script and transport intents.
use crate::app::{AppEngine, AppNode};
use golden_core::{
    node::{Node, NodeId, UserNodeRole},
    ui_sync::UiEditIntent,
};
use std::collections::{HashMap, HashSet};

fn editable_item(engine: &AppEngine, id: NodeId) -> bool {
    match engine.nodes.get(id) {
        Some(AppNode::MediaChannel(node)) => !node
            .node_data()
            .parent
            .and_then(|parent| engine.nodes.get(parent))
            .is_some_and(|node| matches!(node, AppNode::Media(media) if !media.kind().has_authored_channel_variants())),
        Some(
            AppNode::Media(_)
            | AppNode::ReferenceMedia(_)
            | AppNode::MediaCommand(_)
            | AppNode::MediaFolder(_)
            | AppNode::MediaAsset(_)
            | AppNode::Application(_)
            | AppNode::Screen(_)
            | AppNode::Surface(_),
        ) => true,
        _ => false,
    }
}

fn intrinsic_application(engine: &AppEngine, id: NodeId) -> bool {
    let Some(AppNode::Application(node)) = engine.nodes.get(id) else {
        return false;
    };
    match node.node_data().parent.and_then(|parent| engine.nodes.get(parent)) {
        Some(AppNode::MatrixColumn(owner)) => owner.application_node() == Some(id),
        Some(AppNode::MatrixColumnGroup(owner)) => owner.application_node() == Some(id),
        Some(AppNode::MatrixCell(owner)) => owner.application_node() == Some(id),
        _ => false,
    }
}

/// Authored app objects are curated items by hierarchy policy, including projects saved when
/// their constructors accidentally used Regular insertion. Classify before candidate publication;
/// keep identities, declared Parameter children, settings and paired Matrix exports unchanged.
/// Embedded App Control roots remain non-curated settings, including when loaded metadata
/// incorrectly classified them as user items. Their typed owner's handle is authoritative.
pub(crate) fn classify_authored_items(engine: &mut AppEngine) {
    let ids = engine
        .nodes
        .iter()
        .filter_map(|(id, _)| {
            if intrinsic_application(engine, id) {
                Some((id, UserNodeRole::Regular))
            } else {
                editable_item(engine, id).then_some((id, UserNodeRole::ItemRoot))
            }
        })
        .collect::<Vec<_>>();
    for (id, role) in ids {
        let data = engine.nodes.get_mut(id).unwrap().node_data_mut();
        data.user_role = role;
        if role == UserNodeRole::Regular {
            data.meta.user_permissions = Default::default();
        } else {
            data.meta.user_permissions.can_edit_name = true;
            data.meta.user_permissions.can_remove_and_duplicate = true;
        }
    }
}

pub(crate) fn validate_ui_intent(engine: &AppEngine, intent: &UiEditIntent) -> Result<(), String> {
    match intent {
        UiEditIntent::MoveNode { node, new_parent, .. } => validate_move(engine, *node, *new_parent),
        UiEditIntent::RemoveNode { node } => validate_removals(engine, &[*node]),
        UiEditIntent::RemoveNodes { nodes } => validate_removals(engine, nodes),
        UiEditIntent::DuplicateNode { source, new_parent, .. } => validate_channel_copy(engine, *source, *new_parent),
        UiEditIntent::DuplicateNodes { nodes, .. } => {
            for node in nodes {
                validate_channel_copy(engine, node.source, node.new_parent)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_channel_copy(engine: &AppEngine, source: NodeId, parent: NodeId) -> Result<(), String> {
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(source) else {
        return Ok(());
    };
    let Some(AppNode::Media(owner)) = channel.node_data().parent.and_then(|id| engine.nodes.get(id)) else {
        return Err("channel owner is missing".into());
    };
    if !owner.kind().has_authored_channel_variants() || owner.node_data().id != parent {
        return Err("only source channel variants may be copied within their media".into());
    }
    Ok(())
}

pub(super) fn validate_move(engine: &AppEngine, id: NodeId, parent: NodeId) -> Result<(), String> {
    let Some(node) = engine.nodes.get(id) else {
        return Ok(());
    };
    let old_parent = node.node_data().parent;
    let destination = engine.nodes.get(parent);
    let valid = match node {
        AppNode::Media(_) | AppNode::ReferenceMedia(_) | AppNode::MediaFolder(_) => {
            let folder = match destination {
                Some(AppNode::MediaManager(_)) => None,
                Some(AppNode::MediaFolder(node)) => Some(node.node_data().meta.uuid),
                Some(AppNode::Media(media)) if media.kind() == super::protocol::MediaKind::Composition => {
                    if !matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_)) || old_parent != Some(parent) {
                        return Err("Use a Composition drop to create Reference or copied media".into());
                    }
                    return Ok(());
                }
                _ => return Err("media and folders belong in Medias or its folders".into()),
            };
            super::folders::parent(engine, folder)? == parent
        }
        AppNode::MediaChannel(_) | AppNode::CompositionPlacement(_) => old_parent == Some(parent),
        AppNode::MediaAsset(_) | AppNode::Application(_) => parent == super::authoring::manager(engine)?,
        AppNode::MediaCommand(_) => parent == super::authoring::manager(engine)?,
        AppNode::Screen(_) => {
            matches!(destination, Some(AppNode::ScreenManager(node)) if node.node_data().parent == Some(engine.root))
        }
        AppNode::Surface(_) => {
            matches!(destination, Some(AppNode::Screen(screen)) if screen.node_data().parent.and_then(|id| engine.nodes.get(id)).is_some_and(|node| matches!(node, AppNode::ScreenManager(_))))
        }
        _ => return Ok(()),
    };
    if !valid {
        return Err("this object may be sorted within its owner but cannot change document ownership".into());
    }
    if old_parent != Some(parent) {
        let mut child = destination.and_then(|node| node.node_data().first_child);
        while let Some(sibling) = child {
            let sibling_node = engine.nodes.get(sibling).ok_or("destination child is missing")?;
            if sibling != id && sibling_node.node_data().meta.label == node.node_data().meta.label {
                return Err("destination already contains that label; rename the object before moving it".into());
            }
            child = sibling_node.node_data().next_sibling;
        }
    }
    Ok(())
}

fn validate_removals(engine: &AppEngine, nodes: &[NodeId]) -> Result<(), String> {
    let selected = nodes.iter().copied().collect::<HashSet<_>>();
    let mut channels_by_owner = HashMap::<NodeId, usize>::new();
    let mut ancestor_removed = HashMap::<NodeId, bool>::new();
    for id in &selected {
        let Some(AppNode::MediaChannel(node)) = engine.nodes.get(*id) else {
            continue;
        };
        let parent = node.node_data().parent.ok_or("channel owner is missing")?;
        let mut ancestor = Some(parent);
        let mut removed_with_owner = false;
        let mut path = Vec::new();
        while let Some(id) = ancestor {
            if selected.contains(&id) {
                removed_with_owner = true;
                break;
            }
            if let Some(cached) = ancestor_removed.get(&id) {
                removed_with_owner = *cached;
                break;
            }
            path.push(id);
            ancestor = engine.nodes.get(id).and_then(|node| node.node_data().parent);
        }
        for id in path {
            ancestor_removed.insert(id, removed_with_owner);
        }
        if !removed_with_owner {
            *channels_by_owner.entry(parent).or_default() += 1;
        }
    }
    for (parent, count) in channels_by_owner {
        let total = crate::parameters::domain_children(engine, parent)?
            .into_iter()
            .filter(|id| matches!(engine.nodes.get(*id), Some(AppNode::MediaChannel(_))))
            .count();
        if count >= total {
            return Err("media must retain at least one channel".into());
        }
    }
    Ok(())
}
