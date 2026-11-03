//! App-owned structural invariants apply equally to generic UI, transport and headless intents.
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::MediaKind,
};
use golden_core::{
    node::{Node, NodeId},
    ui_sync::UiEditIntent,
};
use std::collections::{HashMap, HashSet};

fn owned(engine: &AppEngine, mut id: NodeId) -> bool {
    // Engine parent links are acyclic. Matrix membership may be nested beneath a group/layer.
    while let Some(node) = engine.nodes.get(id) {
        if matches!(node, AppNode::Media(media) if media.kind() == MediaKind::Matrix) {
            return true;
        }
        let Some(parent) = node.node_data().parent else {
            return false;
        };
        id = parent;
    }
    false
}

fn removes_protected_children(engine: &AppEngine, nodes: &[NodeId]) -> bool {
    let selected = nodes.iter().copied().collect::<HashSet<_>>();
    let mut ancestors = HashMap::new();
    nodes.iter().copied().any(|id| {
        if !owned(engine, id)
            || matches!(engine.nodes.get(id), Some(AppNode::Media(media)) if media.kind() == MediaKind::Matrix)
        {
            return false;
        }
        let mut cursor = engine.nodes.get(id).and_then(|node| node.node_data().parent);
        let mut path = Vec::new();
        let mut removed_by_ancestor = false;
        while let Some(parent) = cursor {
            if selected.contains(&parent) {
                removed_by_ancestor = true;
                break;
            }
            if let Some(cached) = ancestors.get(&parent) {
                removed_by_ancestor = *cached;
                break;
            }
            path.push(parent);
            cursor = engine.nodes.get(parent).and_then(|node| node.node_data().parent);
        }
        for parent in path {
            ancestors.insert(parent, removed_by_ancestor);
        }
        !removed_by_ancestor
    })
}

pub(crate) fn validate_ui_intent(engine: &AppEngine, intent: &UiEditIntent) -> Result<(), String> {
    // App-control entry collections use Golden's ordinary child operations. The declared
    // settings root and paired Matrix structure remain protected by their owning policy.
    let setting = |id| {
        let mut next = Some(id);
        while let Some(id) = next {
            let Some(node) = engine.nodes.get(id) else { return false };
            if matches!(node, AppNode::Application(_)) {
                return true;
            }
            if matches!(node, AppNode::MatrixCell(_)) {
                return false;
            }
            next = node.node_data().parent;
        }
        false
    };
    let entries = match intent {
        UiEditIntent::CreateUserItem { parent, .. } => setting(*parent),
        UiEditIntent::RemoveNode { node } => setting(*node),
        UiEditIntent::RemoveNodes { nodes } => nodes.iter().all(|node| setting(*node)),
        UiEditIntent::MoveNode { node, new_parent, .. } => setting(*node) && setting(*new_parent),
        UiEditIntent::DuplicateNode { source, new_parent, .. } => setting(*source) && setting(*new_parent),
        UiEditIntent::DuplicateNodes {
            nodes,
            created_items,
            dependent_items,
        } => {
            nodes
                .iter()
                .all(|node| setting(node.source) && setting(node.new_parent))
                && created_items.iter().all(|item| setting(item.parent))
                && dependent_items.iter().all(|item| setting(item.parent))
        }
        _ => false,
    };
    if entries {
        return Ok(());
    }
    let protected = match intent {
        // Removing the owner removes its paired exports and all Matrix structure together.
        UiEditIntent::RemoveNode { node } => removes_protected_children(engine, &[*node]),
        UiEditIntent::RemoveNodes { nodes } => removes_protected_children(engine, nodes),
        UiEditIntent::MoveNode { node, new_parent, .. } => {
            let owner =
                matches!(engine.nodes.get(*node), Some(AppNode::Media(media)) if media.kind() == MediaKind::Matrix);
            (!owner && owned(engine, *node)) || owned(engine, *new_parent)
        }
        UiEditIntent::DuplicateNode { source, new_parent, .. } => {
            let owner = matches!(engine.nodes.get(*source), Some(AppNode::Media(media)) if media.kind() == MediaKind::Matrix);
            (!owner && owned(engine, *source)) || owned(engine, *new_parent)
        },
        UiEditIntent::DuplicateNodes {
            nodes,
            created_items,
            dependent_items,
        } => {
            nodes
                .iter()
                .any(|node| {
                    let owner = matches!(engine.nodes.get(node.source), Some(AppNode::Media(media)) if media.kind() == MediaKind::Matrix);
                    (!owner && owned(engine, node.source)) || owned(engine, node.new_parent)
                })
                || created_items.iter().any(|item| owned(engine, item.parent))
                || dependent_items.iter().any(|item| owned(engine, item.parent))
        }
        UiEditIntent::CreateUserItem { parent, .. } => owned(engine, *parent),
        _ => false,
    };
    if protected {
        Err("Use Matrix operations to edit Matrix structure and its paired exports".into())
    } else {
        Ok(())
    }
}
