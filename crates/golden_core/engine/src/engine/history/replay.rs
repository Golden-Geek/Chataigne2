use super::*;

impl<T: Node> HistoryStep<T> {
    /// Applies this step's inverse operation.
    pub(super) fn undo(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        match self {
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
                let enabled_changed = {
                    let current = engine.nodes.get(step.node).ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: "UndoPatchMeta",
                        node: step.node,
                    })?;
                    current.node_data().meta.enabled != step.old_meta.enabled
                };

                let target = engine.nodes.get_mut(step.node).ok_or(EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: "UndoPatchMeta",
                    node: step.node,
                })?;
                target.node_data_mut().meta = step.old_meta.clone();
                engine.emit_event(EventKind::MetaChanged {
                    node: step.node,
                    patch: meta_to_patch(&step.old_meta),
                });

                if enabled_changed {
                    engine.mark_schedule_dirty();
                }
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
                const OP: &str = "UndoAddNode";

                if step.detached_nodes.is_some() {
                    return Ok(());
                }

                let subtree = match engine.collect_subtree(0, OP, step.node) {
                    Ok(subtree) => subtree,
                    Err(EngineEditError::NodeNotFound { node, .. }) if node == step.node => {
                        return Ok(());
                    }
                    Err(err) => return Err(err),
                };
                engine.run_destroy_for_subtree(subtree.as_slice());
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
            }
            Self::RemoveNode(step) => {
                const OP: &str = "UndoRemoveNode";

                let Some(detached_nodes) = step.detached_nodes.take() else {
                    return Ok(());
                };

                let created_ids: Vec<NodeId> = detached_nodes.iter().map(|(id, _)| *id).collect();
                for (id, node) in detached_nodes {
                    engine.nodes.reattach(id, node);
                    engine.register_node_uuid(id);
                    engine.populate_param_cache_entry(id);
                }

                attach_node_for_history(engine, OP, step.node, step.parent, step.prev_sibling, step.next_sibling)?;

                let mut ready_ids = created_ids;
                ready_ids.reverse();

                push_history_subtree_inserted_ui_event(engine, OP, step.node, step.parent, &ready_ids)?;

                for node in ready_ids.iter().copied() {
                    engine.emit_inbox_event(EventKind::NodeCreated { node });
                }
                let decl_id = child_decl_id(engine, 0, OP, step.node)?;
                engine.emit_inbox_event(EventKind::ChildAdded {
                    parent: step.parent,
                    child: step.node,
                    decl_id,
                });

                engine.run_node_ready_for_subtree(ready_ids.as_slice(), crate::node::NodeCreationContext::Fresh)?;
            }
            Self::MoveNode(step) => {
                const OP: &str = "UndoMoveNode";

                if !step.at_new_position {
                    return Ok(());
                }

                let current_parent = engine.detach_node(0, OP, step.node)?;
                engine.attach_node_between(
                    0,
                    OP,
                    step.node,
                    step.old_parent,
                    step.old_prev_sibling,
                    step.old_next_sibling,
                )?;

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
                }

                step.at_new_position = false;
            }
            Self::ReplaceNode(step) => {
                const OP: &str = "UndoReplaceNode";

                let Some(old_node) = step.old_node.take() else {
                    return Ok(());
                };

                if step.old_id == step.new_id {
                    let live_node_data = engine
                        .nodes
                        .get(step.new_id)
                        .ok_or(EngineEditError::NodeNotFound {
                            edit_index: 0,
                            operation: OP,
                            node: step.new_id,
                        })?
                        .node_data()
                        .clone();

                    let mut old_node = old_node;
                    {
                        let old_data = old_node.node_data_mut();
                        old_data.id = step.old_id;
                        old_data.parent = live_node_data.parent;
                        old_data.first_child = live_node_data.first_child;
                        old_data.last_child = live_node_data.last_child;
                        old_data.prev_sibling = live_node_data.prev_sibling;
                        old_data.next_sibling = live_node_data.next_sibling;
                    }

                    engine.unregister_node_uuid(step.new_id);
                    let detached_new_node = engine.nodes.detach(step.new_id).ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: step.new_id,
                    })?;
                    engine.purge_param_cache_entry(step.new_id);
                    engine.nodes.reattach(step.old_id, old_node);
                    engine.register_node_uuid(step.old_id);
                    engine.populate_param_cache_entry(step.old_id);
                    engine.mark_schedule_dirty();
                    let decl_id = child_decl_id(engine, 0, OP, step.old_id)?;
                    engine.emit_event(EventKind::ChildReplaced {
                        parent: step.parent,
                        old: step.new_id,
                        new: step.old_id,
                        decl_id,
                    });

                    step.new_node = Some(detached_new_node);
                    return Ok(());
                }

                engine.detach_node(0, OP, step.new_id)?;
                engine.unregister_node_uuid(step.new_id);
                let detached_new_node = engine.nodes.detach(step.new_id).ok_or(EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: OP,
                    node: step.new_id,
                })?;
                engine.purge_param_cache_entry(step.new_id);

                engine.nodes.reattach(step.old_id, old_node);
                engine.register_node_uuid(step.old_id);
                engine.populate_param_cache_entry(step.old_id);
                attach_node_for_history(
                    engine,
                    OP,
                    step.old_id,
                    step.parent,
                    step.prev_sibling,
                    step.next_sibling,
                )?;

                let first_child = engine
                    .nodes
                    .get(step.old_id)
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: step.old_id,
                    })?
                    .node_data()
                    .first_child;
                engine.reparent_child_chain(0, OP, first_child, step.old_id)?;

                engine.emit_event(EventKind::NodeCreated { node: step.old_id });
                let decl_id = child_decl_id(engine, 0, OP, step.old_id)?;
                engine.emit_event(EventKind::ChildReplaced {
                    parent: step.parent,
                    old: step.new_id,
                    new: step.old_id,
                    decl_id,
                });
                engine.emit_event(EventKind::NodeDeleted { node: step.new_id });

                step.new_node = Some(detached_new_node);
            }
        }

        Ok(())
    }

    /// Reapplies this step's forward operation.
    pub(super) fn redo(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        match self {
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
                let enabled_changed = {
                    let current = engine.nodes.get(step.node).ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: "RedoPatchMeta",
                        node: step.node,
                    })?;
                    current.node_data().meta.enabled != step.new_meta.enabled
                };

                let target = engine.nodes.get_mut(step.node).ok_or(EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: "RedoPatchMeta",
                    node: step.node,
                })?;
                target.node_data_mut().meta = step.new_meta.clone();
                engine.emit_event(EventKind::MetaChanged {
                    node: step.node,
                    patch: meta_to_patch(&step.new_meta),
                });

                if enabled_changed {
                    engine.mark_schedule_dirty();
                }
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
                const OP: &str = "RedoAddNode";

                let Some(detached_nodes) = step.detached_nodes.take() else {
                    return Ok(());
                };

                let created_ids: Vec<NodeId> = detached_nodes.iter().map(|(id, _)| *id).collect();
                for (id, node) in detached_nodes {
                    engine.nodes.reattach(id, node);
                    engine.register_node_uuid(id);
                    engine.populate_param_cache_entry(id);
                }
                attach_node_for_history(engine, OP, step.node, step.parent, step.prev_sibling, step.next_sibling)?;

                let mut ready_ids = created_ids;
                ready_ids.reverse();

                push_history_subtree_inserted_ui_event(engine, OP, step.node, step.parent, &ready_ids)?;

                for node in ready_ids.iter().copied() {
                    engine.emit_inbox_event(EventKind::NodeCreated { node });
                }
                let decl_id = child_decl_id(engine, 0, OP, step.node)?;
                engine.emit_inbox_event(EventKind::ChildAdded {
                    parent: step.parent,
                    child: step.node,
                    decl_id,
                });

                engine.run_node_ready_for_subtree(ready_ids.as_slice(), crate::node::NodeCreationContext::Fresh)?;
            }
            Self::RemoveNode(step) => {
                const OP: &str = "RedoRemoveNode";

                if step.detached_nodes.is_some() {
                    return Ok(());
                }

                let subtree = engine.collect_subtree(0, OP, step.node)?;
                engine.run_destroy_for_subtree(subtree.as_slice());
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
                    engine.purge_param_cache_entry(removed);
                    engine.emit_inbox_event(EventKind::NodeDeleted { node: removed });
                }

                engine.emit_inbox_event(EventKind::ChildRemoved {
                    parent,
                    child: step.node,
                });
                push_history_subtree_removed_ui_event(engine, step.node, removed_ids, parent);

                step.parent = parent;
                step.prev_sibling = prev_sibling;
                step.next_sibling = next_sibling;
                step.detached_nodes = Some(detached_nodes);
            }
            Self::MoveNode(step) => {
                const OP: &str = "RedoMoveNode";

                if step.at_new_position {
                    return Ok(());
                }

                let current_parent = engine.detach_node(0, OP, step.node)?;
                engine.attach_node_between(
                    0,
                    OP,
                    step.node,
                    step.new_parent,
                    step.new_prev_sibling,
                    step.new_next_sibling,
                )?;

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
                }

                step.at_new_position = true;
            }
            Self::ReplaceNode(step) => {
                const OP: &str = "RedoReplaceNode";

                let Some(new_node) = step.new_node.take() else {
                    return Ok(());
                };

                if step.old_id == step.new_id {
                    let live_node_data = engine
                        .nodes
                        .get(step.old_id)
                        .ok_or(EngineEditError::NodeNotFound {
                            edit_index: 0,
                            operation: OP,
                            node: step.old_id,
                        })?
                        .node_data()
                        .clone();

                    let mut new_node = new_node;
                    {
                        let new_data = new_node.node_data_mut();
                        new_data.id = step.new_id;
                        new_data.parent = live_node_data.parent;
                        new_data.first_child = live_node_data.first_child;
                        new_data.last_child = live_node_data.last_child;
                        new_data.prev_sibling = live_node_data.prev_sibling;
                        new_data.next_sibling = live_node_data.next_sibling;
                    }

                    engine.unregister_node_uuid(step.old_id);
                    let detached_old_node = engine.nodes.detach(step.old_id).ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: step.old_id,
                    })?;
                    engine.purge_param_cache_entry(step.old_id);
                    engine.nodes.reattach(step.new_id, new_node);
                    engine.register_node_uuid(step.new_id);
                    engine.populate_param_cache_entry(step.new_id);
                    engine.mark_schedule_dirty();
                    let decl_id = child_decl_id(engine, 0, OP, step.new_id)?;
                    engine.emit_event(EventKind::ChildReplaced {
                        parent: step.parent,
                        old: step.old_id,
                        new: step.new_id,
                        decl_id,
                    });

                    step.old_node = Some(detached_old_node);
                    return Ok(());
                }

                engine.detach_node(0, OP, step.old_id)?;
                engine.unregister_node_uuid(step.old_id);
                let detached_old_node = engine.nodes.detach(step.old_id).ok_or(EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: OP,
                    node: step.old_id,
                })?;
                engine.purge_param_cache_entry(step.old_id);

                engine.nodes.reattach(step.new_id, new_node);
                engine.register_node_uuid(step.new_id);
                engine.populate_param_cache_entry(step.new_id);
                attach_node_for_history(
                    engine,
                    OP,
                    step.new_id,
                    step.parent,
                    step.prev_sibling,
                    step.next_sibling,
                )?;

                let first_child = engine
                    .nodes
                    .get(step.new_id)
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: step.new_id,
                    })?
                    .node_data()
                    .first_child;
                engine.reparent_child_chain(0, OP, first_child, step.new_id)?;

                engine.emit_event(EventKind::NodeCreated { node: step.new_id });
                let decl_id = child_decl_id(engine, 0, OP, step.new_id)?;
                engine.emit_event(EventKind::ChildReplaced {
                    parent: step.parent,
                    old: step.old_id,
                    new: step.new_id,
                    decl_id,
                });
                engine.emit_event(EventKind::NodeDeleted { node: step.old_id });

                step.old_node = Some(detached_old_node);
            }
        }

        Ok(())
    }

    /// Releases detached node payloads owned by this step, if any.
    pub(super) fn dispose(&mut self, engine: &mut Engine<T>) {
        match self {
            Self::SetParam(_)
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
                    purge_detached_node(engine, step.old_id, node);
                }
                if let Some(node) = step.new_node.take() {
                    purge_detached_node(engine, step.new_id, node);
                }
            }
        }
    }
}

fn attach_node_for_history<T: Node>(
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
        parent_children_after: engine.ui_direct_children(parent).unwrap_or_default(),
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
