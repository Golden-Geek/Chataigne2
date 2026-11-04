//! Persistent media organization. Folder membership is document state, never UI state.
use crate::app::{AppEngine, AppNode};
use golden_core::node::{Node, NodeData, NodeId, NodeUuid, UserContainerRules};
use golden_core::process_ctx::ProcessCtx;
use golden_io::{RevisionGuard, RevisionSource};
use std::{any::Any, collections::HashSet};

pub struct MediaFolderNode {
    data: NodeData,
    revision: RevisionSource,
}
impl MediaFolderNode {
    pub(crate) fn new(label: String) -> Self {
        let mut data = NodeData::new(label);
        data.meta.can_be_disabled = false;
        data.meta.user_permissions.can_edit_name = true;
        data.meta.user_permissions.can_remove_and_duplicate = true;
        Self {
            data,
            revision: RevisionSource::default(),
        }
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
}
impl Node for MediaFolderNode {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "mapgyver.media.folder"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            "mapgyver.media.v8",
            "mapgyver.media.folder",
            super::reference::NODE_TYPE,
        ]))
    }
    fn user_creatable_items(&self) -> Vec<golden_core::node::UserCreatableItem> {
        super::creation::items()
    }
    fn create_user_item_tree(&self, node_type: &str) -> Option<golden_core::edit::NodeTree> {
        super::creation::tree(node_type)
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }
    fn project_create(kind: &str) -> Option<Self> {
        (kind == "mapgyver.media.folder").then(|| Self::new("Folder".into()))
    }
}

pub(crate) fn contains_parent(engine: &AppEngine, mut parent: Option<NodeId>, manager: NodeId) -> bool {
    let mut seen = HashSet::new();
    while let Some(id) = parent {
        if id == manager {
            return true;
        }
        if !seen.insert(id) {
            return false;
        }
        let Some(node) = engine.nodes.get(id) else { return false };
        match node {
            AppNode::MediaFolder(_) => {}
            AppNode::Media(media)
                if matches!(
                    media.kind(),
                    super::protocol::MediaKind::Composition | super::protocol::MediaKind::Matrix
                ) => {}
            AppNode::MatrixCell(_) | AppNode::MatrixElement(_) => {}
            AppNode::MediaChannel(channel) if channel.is_matrix_final() => {}
            _ => return false,
        }
        parent = node.node_data().parent;
    }
    false
}

pub(crate) fn parent(engine: &AppEngine, folder: Option<NodeUuid>) -> Result<NodeId, String> {
    let manager = super::authoring::manager(engine)?;
    let Some(uuid) = folder else {
        return Ok(manager);
    };
    let id = engine.node_id_by_uuid(uuid).ok_or("media folder is missing")?;
    if !matches!(engine.nodes.get(id), Some(AppNode::MediaFolder(_))) || !contains_parent(engine, Some(id), manager) {
        return Err("folder is outside Medias".into());
    }
    Ok(id)
}

/// Ordered iterative traversal visits folders once and leaves media/channel subtrees to their owner.
pub(crate) fn entries(engine: &AppEngine) -> Result<Vec<NodeId>, String> {
    let manager = super::authoring::manager(engine)?;
    let mut stack = engine
        .nodes
        .get(manager)
        .and_then(|node| node.node_data().first_child)
        .into_iter()
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            return Err("media hierarchy contains a cycle".into());
        }
        let node = engine.nodes.get(id).ok_or("media hierarchy node is missing")?;
        if let Some(next) = node.node_data().next_sibling {
            stack.push(next);
        }
        if (matches!(node, AppNode::MediaFolder(_))
            || matches!(node, AppNode::Media(media) if matches!(media.kind(), super::protocol::MediaKind::Composition | super::protocol::MediaKind::Matrix))
            || matches!(node, AppNode::MatrixCell(_) | AppNode::MatrixElement(_))
            || matches!(node, AppNode::MediaChannel(channel) if channel.is_matrix_final()))
            && let Some(child) = node.node_data().first_child
        {
            stack.push(child);
        }
        result.push(id);
    }
    Ok(result)
}
