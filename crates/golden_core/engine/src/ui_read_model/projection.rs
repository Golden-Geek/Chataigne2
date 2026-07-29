use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::contexts::UiUserContextsDto;
use crate::engine::EngineTime;
use crate::node::NodeId;
use crate::ui_sync::{
    UI_PROTOCOL_VERSION, UiChildrenOrderPatch, UiEventDto, UiEventKind, UiGraphOp, UiHistoryState, UiLoggerState,
    UiNodeDataDto, UiNodeDto, UiNodeMetaPatch, UiProjectFileSpec, UiSchemaView, UiSnapshot, UiSubscriptionScope,
};

pub(super) struct SnapshotHeader {
    pub(super) at: EngineTime,
    pub(super) history: UiHistoryState,
    pub(super) user_contexts: UiUserContextsDto,
    pub(super) project_file: UiProjectFileSpec,
}

pub(super) struct ProjectionState {
    pub(super) nodes: HashMap<NodeId, UiNodeDto>,
    pub(super) parents: HashMap<NodeId, NodeId>,
    pub(super) header: SnapshotHeader,
    pub(super) schema: UiSchemaView,
    pub(super) cached_snapshot: Arc<UiSnapshot>,
    pub(super) snapshot_dirty: bool,
}

pub(super) fn nodes_to_store(nodes: &[UiNodeDto]) -> HashMap<NodeId, UiNodeDto> {
    nodes.iter().map(|dto| (dto.node_id, dto.clone())).collect()
}

pub(super) fn parents_from_nodes<'a>(nodes: impl IntoIterator<Item = &'a UiNodeDto>) -> HashMap<NodeId, NodeId> {
    let mut parents = HashMap::new();
    for node in nodes {
        for child in &node.children {
            parents.insert(*child, node.node_id);
        }
    }
    parents
}

fn snapshot_header_from_projection(projection: &ProjectionState, scope: UiSubscriptionScope) -> UiSnapshot {
    UiSnapshot {
        protocol_version: UI_PROTOCOL_VERSION.to_string(),
        scope,
        at: projection.header.at,
        nodes: Vec::new(),
        schema: projection.schema.clone(),
        history: projection.header.history.clone(),
        logger: UiLoggerState {
            max_entries: crate::logger::max_entries(),
            records: crate::logger::records(),
        },
        project_file: projection.header.project_file.clone(),
        user_contexts: projection.header.user_contexts.clone(),
    }
}

pub(super) fn snapshot_from_projection(projection: &ProjectionState) -> UiSnapshot {
    let mut snapshot = snapshot_header_from_projection(projection, UiSubscriptionScope::WholeGraph);
    snapshot.nodes = projection.nodes.values().cloned().collect();
    snapshot
}

pub(super) fn scoped_snapshot(projection: &ProjectionState, scope: UiSubscriptionScope) -> UiSnapshot {
    let mut snapshot = snapshot_header_from_projection(projection, scope.clone());
    snapshot.nodes = nodes_for_scope(&projection.nodes, scope);
    snapshot
}

/// Applies graph transactions and standalone changes to the incremental projection.
pub(super) fn apply_events(projection: &mut ProjectionState, events: &[UiEventDto]) {
    for event in events {
        match &event.kind {
            UiEventKind::GraphTransaction { transaction } => {
                for op in transaction.ops.iter() {
                    apply_graph_op(&mut projection.nodes, &mut projection.parents, op);
                }
            }
            UiEventKind::ParamChanged {
                param,
                old_value,
                new_value,
            } => {
                if let Some(UiNodeDto {
                    data: UiNodeDataDto::Parameter { param: param_dto },
                    ..
                }) = projection.nodes.get_mut(param)
                {
                    if param_dto.default_value.is_none() && old_value != new_value {
                        param_dto.default_value = Some(old_value.clone());
                    }
                    param_dto.value.clone_from(new_value);
                    if param_dto.default_value.as_ref() == Some(new_value) {
                        param_dto.default_value = None;
                    }
                }
            }
            UiEventKind::ParamControlChanged { param, new_state, .. } => {
                if let Some(UiNodeDto {
                    data: UiNodeDataDto::Parameter { param: param_dto },
                    ..
                }) = projection.nodes.get_mut(param)
                {
                    param_dto.control.clone_from(new_state);
                }
            }
            UiEventKind::ParamConstraintsChanged {
                param, new_constraints, ..
            } => {
                if let Some(UiNodeDto {
                    data: UiNodeDataDto::Parameter { param: param_dto },
                    ..
                }) = projection.nodes.get_mut(param)
                {
                    param_dto.constraints.clone_from(new_constraints);
                }
            }
            UiEventKind::ChildAdded {
                parent,
                child,
                parent_children,
                ..
            } => {
                projection.parents.insert(*child, *parent);
                if let Some(parent_dto) = projection.nodes.get_mut(parent) {
                    if let Some(children) = parent_children {
                        parent_dto.children.clone_from(children);
                    } else if !parent_dto.children.contains(child) {
                        parent_dto.children.push(*child);
                    }
                }
            }
            UiEventKind::ChildRemoved { parent, child } => {
                if projection.parents.get(child) == Some(parent) {
                    projection.parents.remove(child);
                }
                if let Some(parent_dto) = projection.nodes.get_mut(parent) {
                    parent_dto.children.retain(|candidate| candidate != child);
                }
            }
            UiEventKind::ChildReplaced { parent, old, new, .. } => {
                if projection.parents.get(old) == Some(parent) {
                    projection.parents.remove(old);
                }
                projection.parents.insert(*new, *parent);
                if let Some(parent_dto) = projection.nodes.get_mut(parent)
                    && let Some(index) = parent_dto.children.iter().position(|child| child == old)
                {
                    parent_dto.children[index] = *new;
                }
            }
            UiEventKind::ChildMoved {
                child,
                old_parent,
                new_parent,
                old_parent_children,
                new_parent_children,
            } => {
                projection.parents.insert(*child, *new_parent);
                if let Some(children) = old_parent_children
                    && let Some(parent_dto) = projection.nodes.get_mut(old_parent)
                {
                    parent_dto.children.clone_from(children);
                }
                if let Some(children) = new_parent_children
                    && let Some(parent_dto) = projection.nodes.get_mut(new_parent)
                {
                    parent_dto.children.clone_from(children);
                }
            }
            UiEventKind::ChildReordered {
                parent,
                child,
                parent_children,
            } => {
                projection.parents.insert(*child, *parent);
                if let Some(children) = parent_children
                    && let Some(parent_dto) = projection.nodes.get_mut(parent)
                {
                    parent_dto.children.clone_from(children);
                }
            }
            UiEventKind::NodeCreated { node, snapshot } => {
                if let Some(snapshot) = snapshot {
                    record_node_children_parents(&mut projection.parents, snapshot);
                    projection.nodes.insert(*node, snapshot.as_ref().clone());
                }
            }
            UiEventKind::NodeDeleted { node } => {
                projection.parents.remove(node);
                if let Some(removed) = projection.nodes.remove(node) {
                    for child in removed.children {
                        if projection.parents.get(&child) == Some(node) {
                            projection.parents.remove(&child);
                        }
                    }
                }
            }
            UiEventKind::MetaChanged { node, patch } => {
                if let Some(dto) = projection.nodes.get_mut(node) {
                    apply_meta_patch(&mut dto.meta, &UiNodeMetaPatch::from(patch));
                }
            }
            UiEventKind::Custom { .. } => {}
        }
    }
}

fn apply_graph_op(store: &mut HashMap<NodeId, UiNodeDto>, parents: &mut HashMap<NodeId, NodeId>, op: &UiGraphOp) {
    match op {
        UiGraphOp::NodeCreated { snapshot, parent, .. } => {
            record_node_children_parents(parents, snapshot);
            if let Some(parent) = parent {
                parents.insert(snapshot.node_id, *parent);
            }
            store.insert(snapshot.node_id, snapshot.as_ref().clone());
        }
        UiGraphOp::SubtreeInserted {
            root,
            nodes,
            parent,
            parent_children_after,
            ..
        } => {
            for node in nodes {
                record_node_children_parents(parents, node);
                store.insert(node.node_id, node.clone());
            }
            parents.insert(*root, *parent);
            if let Some(parent_dto) = store.get_mut(parent) {
                parent_dto.children.clone_from(parent_children_after);
            }
        }
        UiGraphOp::SubtreeRemoved {
            removed_ids,
            parent_after,
            ..
        } => {
            for id in removed_ids {
                store.remove(id);
                parents.remove(id);
            }
            apply_children_order(store, parent_after.as_ref());
        }
        UiGraphOp::NodeMoved {
            node,
            new_parent,
            old_parent_after,
            new_parent_after,
            ..
        } => {
            if let Some(new_parent) = new_parent {
                parents.insert(*node, *new_parent);
            } else {
                parents.remove(node);
            }
            apply_children_order(store, old_parent_after.as_ref());
            apply_children_order(store, new_parent_after.as_ref());
        }
        UiGraphOp::ChildrenReordered { parent, children } => {
            if let Some(node) = store.get_mut(parent) {
                node.children.clone_from(children);
            }
            for child in children {
                parents.insert(*child, *parent);
            }
        }
        UiGraphOp::NodeMetaPatched { node, patch } => {
            if let Some(dto) = store.get_mut(node) {
                apply_meta_patch(&mut dto.meta, patch);
            }
        }
        UiGraphOp::ParamPatched { param, patch, .. } => {
            if let Some(dto) = store.get_mut(param)
                && let UiNodeDataDto::Parameter { param: param_dto } = &mut dto.data
            {
                if let Some(value) = &patch.value {
                    param_dto.value = value.clone();
                }
                if let Some(control) = &patch.control {
                    param_dto.control = control.clone();
                }
                if let Some(constraints) = &patch.constraints {
                    param_dto.constraints = constraints.clone();
                }
            }
        }
        UiGraphOp::HistoryPatched { .. } | UiGraphOp::LoggerPatched { .. } => {}
    }
}

fn record_node_children_parents(parents: &mut HashMap<NodeId, NodeId>, node: &UiNodeDto) {
    for child in &node.children {
        parents.insert(*child, node.node_id);
    }
}

fn apply_children_order(store: &mut HashMap<NodeId, UiNodeDto>, patch: Option<&UiChildrenOrderPatch>) {
    if let Some(patch) = patch
        && let Some(node) = store.get_mut(&patch.parent)
    {
        node.children.clone_from(&patch.children);
    }
}

fn apply_meta_patch(meta: &mut crate::ui_sync::UiNodeMetaDto, patch: &UiNodeMetaPatch) {
    if let Some(label) = &patch.label {
        meta.label.clone_from(label);
    }
    if let Some(short_name) = &patch.short_name {
        meta.short_name.clone_from(short_name);
    }
    if let Some(enabled) = patch.enabled {
        meta.enabled = enabled;
    }
    if let Some(can_be_disabled) = patch.can_be_disabled {
        meta.can_be_disabled = can_be_disabled;
    }
    if let Some(description) = &patch.description {
        meta.description.clone_from(description);
    }
    if let Some(user_permissions) = &patch.user_permissions {
        meta.user_permissions.clone_from(user_permissions);
    }
    if let Some(tags) = &patch.tags {
        meta.tags.clone_from(tags);
    }
    if let Some(presentation) = &patch.presentation {
        meta.presentation.clone_from(presentation);
    }
}

fn nodes_for_scope(store: &HashMap<NodeId, UiNodeDto>, scope: UiSubscriptionScope) -> Vec<UiNodeDto> {
    match scope {
        UiSubscriptionScope::WholeGraph => store.values().cloned().collect(),
        UiSubscriptionScope::Subtree { root, max_depth } => {
            let mut out = Vec::new();
            let mut stack = vec![(root, 0u32)];
            let mut visited = HashSet::new();
            while let Some((node_id, depth)) = stack.pop() {
                if !visited.insert(node_id) {
                    continue;
                }
                let Some(node) = store.get(&node_id) else {
                    continue;
                };
                out.push(node.clone());
                if depth >= max_depth {
                    continue;
                }
                for child in node.children.iter().rev() {
                    stack.push((*child, depth.saturating_add(1)));
                }
            }
            out
        }
    }
}
