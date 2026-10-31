//! App-owned structural invariants apply equally to generic UI, transport and headless intents.
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::MediaKind,
};
use golden_core::{
    node::{Node, NodeId},
    ui_sync::UiEditIntent,
};

fn owned(engine: &AppEngine, mut id: NodeId) -> bool {
    // Engine parent links are acyclic. Matrix membership may be nested beneath a group/layer.
    while let Some(node) = engine.nodes.get(id) {
        if matches!(node, AppNode::Media(media) if media.kind == MediaKind::Matrix) {
            return true;
        }
        let Some(parent) = node.node_data().parent else {
            return false;
        };
        id = parent;
    }
    false
}
pub(crate) fn validate_ui_intent(engine: &AppEngine, intent: &UiEditIntent) -> Result<(), String> {
    let protected = match intent {
        UiEditIntent::RemoveNode { node } => owned(engine, *node),
        UiEditIntent::RemoveNodes { nodes } => nodes.iter().any(|node| owned(engine, *node)),
        UiEditIntent::MoveNode { node, new_parent, .. } => owned(engine, *node) || owned(engine, *new_parent),
        UiEditIntent::DuplicateNode { source, new_parent, .. } => owned(engine, *source) || owned(engine, *new_parent),
        UiEditIntent::DuplicateNodes {
            nodes,
            created_items,
            dependent_items,
        } => {
            nodes
                .iter()
                .any(|node| owned(engine, node.source) || owned(engine, node.new_parent))
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
