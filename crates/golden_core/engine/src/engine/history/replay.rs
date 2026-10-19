use super::*;

impl<T: Node> HistoryStep<T> {
    /// Applies this step's inverse operation.
    pub(super) fn undo(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        match self {
            Self::AuthoredNode(step) => engine.restore_authored_node_state(step.node, &step.old)?,
            Self::SetParam(step) => {
                let _ = engine.apply_set_param(0, step.node, step.old_value.clone())?;
            }
            Self::SetParamControlState(step) => {
                engine.apply_set_param_control_state_for_history(
                    "UndoSetParamControlState",
                    step.node,
                    step.old_state.clone(),
                )?;
            }
            Self::SetParamConstraints(step) => {
                engine.apply_restore_param_state_for_history(
                    "UndoSetParamConstraints",
                    step.node,
                    step.old_value.clone(),
                    step.old_constraints.clone(),
                )?;
            }
            Self::PatchMeta(step) => {
                replay_patch_meta(engine, step.node, &step.old_meta, "UndoPatchMeta")?;
            }
            Self::SetScriptConfig(step) => {
                engine.apply_script_config_for_history(
                    "UndoSetScriptConfig",
                    step.node,
                    step.old_config.clone(),
                    step.force_reload,
                )?;
            }
            Self::AddNode(step) => {
                undo_add_node(engine, step, true)?;
            }
            Self::RemoveNode(step) => {
                if let Some(ready_ids) = restore_remove_node(engine, step)? {
                    push_history_subtree_inserted_ui_event(
                        engine,
                        "UndoRemoveNode",
                        step.node,
                        step.parent,
                        &ready_ids,
                    )?;
                    emit_restored_remove_node_events(engine, step, &ready_ids)?;
                    engine.run_node_ready_for_subtree(
                        ready_ids.as_slice(),
                        crate::node::NodeCreationContext::HistoryRestore,
                    )?;
                }
            }
            Self::MoveNode(step) => {
                const OP: &str = "UndoMoveNode";

                if !step.at_new_position {
                    return Ok(());
                }

                validate_history_move(engine, OP, step.node, step.old_parent)?;
                let current_parent = engine.detach_node(0, OP, step.node)?;
                attach_node_for_history(
                    engine,
                    OP,
                    step.node,
                    step.old_parent,
                    step.old_prev_sibling,
                    step.old_next_sibling,
                )?;

                step.at_new_position = false;

                if current_parent == step.old_parent {
                    engine.emit_event(EventKind::ChildReordered {
                        parent: step.old_parent,
                        child: step.node,
                    });
                } else {
                    engine.emit_event(EventKind::ChildMoved {
                        child: step.node,
                        old_parent: current_parent,
                        new_parent: step.old_parent,
                    });
                    let changes = engine.subtree_effective_enabled_changes(step.node);
                    engine.queue_effective_enabled_callbacks(&changes)?;
                }
            }
            Self::ReplaceNode(step) => {
                swap_replacement(engine, step, true)?;
            }
        }

        Ok(())
    }

    /// Reapplies this step's forward operation.
    pub(super) fn redo(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        match self {
            Self::AuthoredNode(step) => engine.restore_authored_node_state(step.node, &step.new)?,
            Self::SetParam(step) => {
                let _ = engine.apply_set_param(0, step.node, step.new_value.clone())?;
            }
            Self::SetParamControlState(step) => {
                engine.apply_set_param_control_state_for_history(
                    "RedoSetParamControlState",
                    step.node,
                    step.new_state.clone(),
                )?;
            }
            Self::SetParamConstraints(step) => {
                engine.apply_restore_param_state_for_history(
                    "RedoSetParamConstraints",
                    step.node,
                    step.new_value.clone(),
                    step.new_constraints.clone(),
                )?;
            }
            Self::PatchMeta(step) => {
                replay_patch_meta(engine, step.node, &step.new_meta, "RedoPatchMeta")?;
            }
            Self::SetScriptConfig(step) => {
                engine.apply_script_config_for_history(
                    "RedoSetScriptConfig",
                    step.node,
                    step.new_config.clone(),
                    step.force_reload,
                )?;
            }
            Self::AddNode(step) => {
                if let Some(ready_ids) = restore_add_node(engine, step)? {
                    push_history_subtree_inserted_ui_event(engine, "RedoAddNode", step.node, step.parent, &ready_ids)?;
                    emit_added_node_events(engine, step, &ready_ids)?;
                    engine.run_node_ready_for_subtree(
                        ready_ids.as_slice(),
                        crate::node::NodeCreationContext::HistoryRestore,
                    )?;
                }
            }
            Self::RemoveNode(step) => {
                redo_remove_node(engine, step, true)?;
            }
            Self::MoveNode(step) => {
                const OP: &str = "RedoMoveNode";

                if step.at_new_position {
                    return Ok(());
                }

                validate_history_move(engine, OP, step.node, step.new_parent)?;
                let current_parent = engine.detach_node(0, OP, step.node)?;
                attach_node_for_history(
                    engine,
                    OP,
                    step.node,
                    step.new_parent,
                    step.new_prev_sibling,
                    step.new_next_sibling,
                )?;

                step.at_new_position = true;

                if current_parent == step.new_parent {
                    engine.emit_event(EventKind::ChildReordered {
                        parent: step.new_parent,
                        child: step.node,
                    });
                } else {
                    engine.emit_event(EventKind::ChildMoved {
                        child: step.node,
                        old_parent: current_parent,
                        new_parent: step.new_parent,
                    });
                    let changes = engine.subtree_effective_enabled_changes(step.node);
                    engine.queue_effective_enabled_callbacks(&changes)?;
                }
            }
            Self::ReplaceNode(step) => {
                swap_replacement(engine, step, false)?;
            }
        }

        Ok(())
    }

    /// Releases detached node payloads owned by this step, if any.
    pub(super) fn dispose(&mut self, engine: &mut Engine<T>) {
        match self {
            Self::AuthoredNode(_)
            | Self::SetParam(_)
            | Self::SetParamControlState(_)
            | Self::SetParamConstraints(_)
            | Self::PatchMeta(_)
            | Self::SetScriptConfig(_)
            | Self::MoveNode(_) => {}
            Self::AddNode(step) => {
                if let Some(nodes) = step.detached_nodes.take() {
                    for (id, node) in nodes {
                        purge_detached_node(engine, id, node);
                    }
                }
            }
            Self::RemoveNode(step) => {
                if let Some(nodes) = step.detached_nodes.take() {
                    for (id, node) in nodes {
                        purge_detached_node(engine, id, node);
                    }
                }
            }
            Self::ReplaceNode(step) => {
                if let Some(node) = step.old_node.take() {
                    purge_detached_node(engine, step.node, node);
                }
                if let Some(node) = step.new_node.take() {
                    purge_detached_node(engine, step.node, node);
                }
            }
        }
    }
}

pub(super) fn attach_node_for_history<T: Node>(
    engine: &mut Engine<T>,
    operation: &'static str,
    node: NodeId,
    parent: NodeId,
    prev_sibling: Option<NodeId>,
    next_sibling: Option<NodeId>,
) -> Result<(), EngineEditError> {
    let (prev_sibling, next_sibling) = historical_sibling_bounds(engine, parent, prev_sibling, next_sibling);
    engine.attach_node_between(0, operation, node, parent, prev_sibling, next_sibling)
}

fn validate_history_move<T: Node>(
    engine: &Engine<T>,
    operation: &'static str,
    node: NodeId,
    parent: NodeId,
) -> Result<(), EngineEditError> {
    if !engine.nodes.contains(parent) {
        return Err(EngineEditError::ParentNotFound {
            edit_index: 0,
            operation,
            parent,
        });
    }
    if node == parent || engine.is_descendant_of(parent, node) {
        return Err(EngineEditError::CycleDetected {
            edit_index: 0,
            operation,
            node,
            new_parent: parent,
        });
    }
    Ok(())
}

fn validate_detached_restore<T: Node>(
    engine: &Engine<T>,
    operation: &'static str,
    root: NodeId,
    parent: NodeId,
    nodes: &[(NodeId, T)],
) -> Result<(), EngineEditError> {
    if !engine.nodes.contains(parent) {
        return Err(EngineEditError::ParentNotFound {
            edit_index: 0,
            operation,
            parent,
        });
    }
    if nodes.iter().any(|(id, _)| engine.nodes.contains(*id)) {
        return Err(EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation,
            node: root,
            node_type: "history subtree".into(),
            message: "a detached history identity is already live".into(),
        });
    }
    Ok(())
}

fn swap_replacement<T: Node>(
    engine: &mut Engine<T>,
    step: &mut ReplaceNodeHistory<T>,
    undo: bool,
) -> Result<(), EngineEditError> {
    let incoming = if undo { &mut step.old_node } else { &mut step.new_node };
    if incoming.is_none() {
        return Ok(());
    }
    const OP: &str = "HistoryReplaceNode";
    let live = engine
        .nodes
        .get(step.node)
        .ok_or(EngineEditError::NodeNotFound {
            edit_index: 0,
            operation: OP,
            node: step.node,
        })?
        .node_data()
        .clone();
    engine.run_destroy_for_subtree(&[step.node])?;
    let mut replacement = incoming.take().expect("detached replacement");
    let data = replacement.node_data_mut();
    data.id = step.node;
    data.parent = live.parent;
    data.first_child = live.first_child;
    data.last_child = live.last_child;
    data.prev_sibling = live.prev_sibling;
    data.next_sibling = live.next_sibling;
    data.effective_enabled = live.effective_enabled;
    engine.unregister_node_uuid(step.node);
    let detached = engine.nodes.detach(step.node).expect("validated live replacement");
    engine.purge_param_cache_entry(step.node);
    engine.node_wakes.source.close(step.node);
    engine.event_listeners.remove_subscriber(step.node);
    engine.nodes.reattach(step.node, replacement);
    engine.register_node_uuid(step.node);
    engine.populate_param_cache_entry(step.node);
    swap_detached_blueprint(engine, step.node, step.node, &mut step.detached_blueprint);
    // Retain ownership before any fallible callback or projection operation.
    if undo {
        step.new_node = Some(detached);
    } else {
        step.old_node = Some(detached);
    }
    engine.mark_schedule_dirty();
    let changes = engine.subtree_effective_enabled_changes(step.node);
    engine.queue_effective_enabled_callbacks(&changes)?;
    engine.run_node_ready(step.node, crate::node::NodeCreationContext::HistoryRestore)?;
    if let Some(parent) = live.parent {
        let decl_id = child_decl_id(engine, 0, OP, step.node)?;
        engine.emit_event(EventKind::ChildReplaced {
            parent,
            old: step.node,
            new: step.node,
            decl_id,
        });
    }
    if let Some(snapshot) = engine.ui_node_dto_for_event(step.node) {
        let index = live.parent.and_then(|parent| engine.ui_child_index(parent, step.node));
        engine.push_ui_graph_transaction(vec![UiGraphOp::NodeCreated {
            snapshot: Box::new(snapshot),
            parent: live.parent,
            index,
        }]);
    }
    Ok(())
}

fn historical_sibling_bounds<T: Node>(
    engine: &Engine<T>,
    parent: NodeId,
    prev_sibling: Option<NodeId>,
    next_sibling: Option<NodeId>,
) -> (Option<NodeId>, Option<NodeId>) {
    let live_child = |candidate: NodeId| {
        engine
            .nodes
            .get(candidate)
            .is_some_and(|node| node.node_data().parent == Some(parent))
    };

    let prev_live = prev_sibling.filter(|sibling| live_child(*sibling));
    let next_live = next_sibling.filter(|sibling| live_child(*sibling));

    if let (Some(prev), Some(next)) = (prev_live, next_live) {
        let adjacent = engine
            .nodes
            .get(prev)
            .is_some_and(|node| node.node_data().next_sibling == Some(next));
        if adjacent {
            return (Some(prev), Some(next));
        }
    }

    if let Some(prev) = prev_live {
        let next = engine.nodes.get(prev).and_then(|node| node.node_data().next_sibling);
        return (Some(prev), next);
    }

    if let Some(next) = next_live {
        let prev = engine.nodes.get(next).and_then(|node| node.node_data().prev_sibling);
        return (prev, Some(next));
    }

    (
        engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
        None,
    )
}

/// Temporarily reattaches then permanently removes a previously detached node payload.
fn purge_detached_node<T: Node>(engine: &mut Engine<T>, id: NodeId, node: T) {
    if engine.nodes.contains(id) {
        return;
    }
    engine.nodes.reattach(id, node);
    let _ = engine.nodes.remove(id);
}

fn swap_detached_blueprint<T: Node>(
    engine: &mut Engine<T>,
    live: NodeId,
    restored: NodeId,
    detached: &mut Option<BlueprintInstanceMeta>,
) {
    let active = engine.blueprints.unregister_instance(live);
    if let Some(meta) = detached.take() {
        engine.blueprints.register_instance(restored, meta);
    }
    *detached = active;
}

fn child_decl_id<T: Node>(
    engine: &Engine<T>,
    edit_index: usize,
    operation: &'static str,
    node: NodeId,
) -> Result<crate::node::DeclId, EngineEditError> {
    Ok(engine
        .nodes
        .get(node)
        .ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation,
            node,
        })?
        .node_data()
        .meta
        .decl_id
        .clone())
}

pub(super) fn undo_add_node<T: Node>(
    engine: &mut Engine<T>,
    step: &mut AddNodeHistory<T>,
    run_destroy: bool,
) -> Result<(), EngineEditError> {
    const OP: &str = "UndoAddNode";

    if step.detached_nodes.is_some() {
        return Ok(());
    }

    let subtree = match engine.collect_subtree(0, OP, step.node) {
        Ok(subtree) => subtree,
        Err(EngineEditError::NodeNotFound { node, .. }) if node == step.node => return Ok(()),
        Err(err) => return Err(err),
    };
    if run_destroy {
        engine.run_destroy_for_subtree(subtree.as_slice())?;
    }
    let (parent, prev_sibling, next_sibling) = engine.node_position(0, OP, step.node)?;
    engine.detach_node(0, OP, step.node)?;

    let mut detached_nodes = Vec::with_capacity(subtree.len());
    let removed_ids = subtree.clone();
    for removed in subtree.into_iter().rev() {
        engine.unregister_node_uuid(removed);
        let detached = engine.nodes.detach(removed).ok_or(EngineEditError::NodeNotFound {
            edit_index: 0,
            operation: OP,
            node: removed,
        })?;
        detached_nodes.push((removed, detached));
        if let Some(meta) = engine.blueprints.unregister_instance(removed) {
            step.detached_blueprints.push((removed, meta));
        }
        engine.purge_param_cache_entry(removed);
        engine.emit_inbox_event(EventKind::NodeDeleted { node: removed });
    }

    step.parent = parent;
    step.prev_sibling = prev_sibling;
    step.next_sibling = next_sibling;
    step.detached_nodes = Some(detached_nodes);

    engine.emit_inbox_event(EventKind::ChildRemoved {
        parent,
        child: step.node,
    });
    push_history_subtree_removed_ui_event(engine, step.node, removed_ids, parent);
    Ok(())
}

pub(super) fn restore_add_node<T: Node>(
    engine: &mut Engine<T>,
    step: &mut AddNodeHistory<T>,
) -> Result<Option<Vec<NodeId>>, EngineEditError> {
    const OP: &str = "RedoAddNode";
    if let Some(nodes) = &step.detached_nodes {
        validate_detached_restore(engine, OP, step.node, step.parent, nodes)?;
    }
    let Some(detached_nodes) = step.detached_nodes.take() else {
        return Ok(None);
    };

    let created_ids: Vec<NodeId> = detached_nodes.iter().map(|(id, _)| *id).collect();
    for (id, node) in detached_nodes {
        engine.nodes.reattach(id, node);
        engine.register_node_uuid(id);
        engine.populate_param_cache_entry(id);
    }
    attach_node_for_history(engine, OP, step.node, step.parent, step.prev_sibling, step.next_sibling)?;

    for (root, meta) in step.detached_blueprints.drain(..) {
        engine.blueprints.register_instance(root, meta);
    }

    let mut ready_ids = created_ids;
    ready_ids.reverse();
    Ok(Some(ready_ids))
}

pub(super) fn emit_added_node_events<T: Node>(
    engine: &mut Engine<T>,
    step: &AddNodeHistory<T>,
    ready_ids: &[NodeId],
) -> Result<(), EngineEditError> {
    for node in ready_ids.iter().copied() {
        engine.emit_inbox_event(EventKind::NodeCreated { node });
    }
    let decl_id = child_decl_id(engine, 0, "RedoAddNode", step.node)?;
    engine.emit_inbox_event(EventKind::ChildAdded {
        parent: step.parent,
        child: step.node,
        decl_id,
    });
    Ok(())
}

pub(super) fn restore_remove_node<T: Node>(
    engine: &mut Engine<T>,
    step: &mut RemoveNodeHistory<T>,
) -> Result<Option<Vec<NodeId>>, EngineEditError> {
    const OP: &str = "UndoRemoveNode";
    if let Some(nodes) = &step.detached_nodes {
        validate_detached_restore(engine, OP, step.node, step.parent, nodes)?;
    }
    let Some(detached_nodes) = step.detached_nodes.take() else {
        return Ok(None);
    };

    let created_ids: Vec<NodeId> = detached_nodes.iter().map(|(id, _)| *id).collect();
    for (id, node) in detached_nodes {
        engine.nodes.reattach(id, node);
        engine.register_node_uuid(id);
        engine.populate_param_cache_entry(id);
    }
    attach_node_for_history(engine, OP, step.node, step.parent, step.prev_sibling, step.next_sibling)?;

    for (root, meta) in step.detached_blueprints.drain(..) {
        engine.blueprints.register_instance(root, meta);
    }

    let mut ready_ids = created_ids;
    ready_ids.reverse();
    Ok(Some(ready_ids))
}

pub(super) fn emit_restored_remove_node_events<T: Node>(
    engine: &mut Engine<T>,
    step: &RemoveNodeHistory<T>,
    ready_ids: &[NodeId],
) -> Result<(), EngineEditError> {
    for node in ready_ids.iter().copied() {
        engine.emit_inbox_event(EventKind::NodeCreated { node });
    }
    let decl_id = child_decl_id(engine, 0, "UndoRemoveNode", step.node)?;
    engine.emit_inbox_event(EventKind::ChildAdded {
        parent: step.parent,
        child: step.node,
        decl_id,
    });
    Ok(())
}

pub(super) fn redo_remove_node<T: Node>(
    engine: &mut Engine<T>,
    step: &mut RemoveNodeHistory<T>,
    run_destroy: bool,
) -> Result<Option<(NodeId, Vec<NodeId>, NodeId)>, EngineEditError> {
    const OP: &str = "RedoRemoveNode";
    if step.detached_nodes.is_some() {
        return Ok(None);
    }

    let subtree = engine.collect_subtree(0, OP, step.node)?;
    if run_destroy {
        engine.run_destroy_for_subtree(subtree.as_slice())?;
    }
    let (parent, prev_sibling, next_sibling) = engine.node_position(0, OP, step.node)?;
    engine.detach_node(0, OP, step.node)?;

    let mut detached_nodes = Vec::with_capacity(subtree.len());
    let removed_ids = subtree.clone();
    for removed in subtree.into_iter().rev() {
        engine.unregister_node_uuid(removed);
        let detached = engine.nodes.detach(removed).ok_or(EngineEditError::NodeNotFound {
            edit_index: 0,
            operation: OP,
            node: removed,
        })?;
        detached_nodes.push((removed, detached));
        if let Some(meta) = engine.blueprints.unregister_instance(removed) {
            step.detached_blueprints.push((removed, meta));
        }
        engine.purge_param_cache_entry(removed);
        engine.emit_inbox_event(EventKind::NodeDeleted { node: removed });
    }

    engine.emit_inbox_event(EventKind::ChildRemoved {
        parent,
        child: step.node,
    });
    let removed = if run_destroy {
        push_history_subtree_removed_ui_event(engine, step.node, removed_ids, parent);
        None
    } else {
        Some((step.node, removed_ids, parent))
    };

    step.parent = parent;
    step.prev_sibling = prev_sibling;
    step.next_sibling = next_sibling;
    step.detached_nodes = Some(detached_nodes);
    Ok(removed)
}

fn push_history_subtree_inserted_ui_event<T: Node>(
    engine: &mut Engine<T>,
    operation: &'static str,
    root: NodeId,
    parent: NodeId,
    node_ids: &[NodeId],
) -> Result<(), EngineEditError> {
    let catalog_snapshot = engine.build_process_tree_snapshot();
    let mut nodes = Vec::with_capacity(node_ids.len());

    for node_id in node_ids {
        let snapshot = engine
            .ui_node_dto_for_event_with_catalog_snapshot(*node_id, catalog_snapshot.as_ref())
            .ok_or(EngineEditError::NodeNotFound {
                edit_index: 0,
                operation,
                node: *node_id,
            })?;
        nodes.push(snapshot);
    }

    engine.push_ui_graph_transaction(vec![UiGraphOp::SubtreeInserted {
        root,
        parent,
        nodes,
        parent_children_after: Some(engine.ui_direct_children(parent).unwrap_or_default()),
    }]);

    Ok(())
}

fn push_history_subtree_removed_ui_event<T: Node>(
    engine: &mut Engine<T>,
    root: NodeId,
    removed_ids: Vec<NodeId>,
    parent: NodeId,
) {
    engine.push_ui_graph_transaction(vec![UiGraphOp::SubtreeRemoved {
        root,
        removed_ids,
        parent_after: engine.ui_children_order_patch(parent),
    }]);
}

fn replay_patch_meta<T: Node>(
    engine: &mut Engine<T>,
    node: NodeId,
    restored_meta: &NodeMeta,
    operation: &'static str,
) -> Result<(), EngineEditError> {
    let enabled_changed = engine
        .nodes
        .get(node)
        .ok_or(EngineEditError::NodeNotFound {
            edit_index: 0,
            operation,
            node,
        })?
        .node_data()
        .meta
        .enabled
        != restored_meta.enabled;
    let previous_enabled = enabled_changed.then(|| {
        engine
            .collect_subtree_node_ids(node)
            .into_iter()
            .map(|child| (child, engine.is_effectively_enabled(child)))
            .collect::<Vec<_>>()
    });

    engine
        .nodes
        .get_mut(node)
        .expect("node was validated")
        .node_data_mut()
        .meta = restored_meta.clone();
    if let Some(previous_enabled) = previous_enabled {
        engine.mark_schedule_dirty();
        let changes = previous_enabled
            .into_iter()
            .filter_map(|(child, was_enabled)| {
                let enabled = engine.is_effectively_enabled(child);
                (enabled != was_enabled).then_some((child, enabled))
            })
            .collect::<Vec<_>>();
        engine.queue_effective_enabled_callbacks(&changes)?;
    }
    engine.emit_event(EventKind::MetaChanged {
        node,
        patch: meta_to_patch(restored_meta),
    });
    Ok(())
}

fn meta_to_patch(meta: &NodeMeta) -> NodeMetaPatch {
    NodeMetaPatch {
        short_name: Some(meta.short_name.clone()),
        enabled: Some(meta.enabled),
        can_be_disabled: Some(meta.can_be_disabled),
        label: Some(meta.label.clone()),
        description: Some(meta.description.clone()),
        tags: Some(meta.tags.clone()),
        user_permissions: Some(meta.user_permissions.clone()),
        semantics: Some(meta.semantics.clone()),
        presentation: Some(meta.presentation.clone()),
    }
}
