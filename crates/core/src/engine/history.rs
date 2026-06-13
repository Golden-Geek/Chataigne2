use crate::edit::EditOrigin;
use crate::events::EventKind;
use crate::node::{Node, NodeId, NodeMeta, NodeMetaPatch};
use crate::parameter::{ParamValue, ParameterConstraints, ParameterControlState, ParameterEventBehaviour};
use crate::script::ScriptNodeConfig;

use super::{Engine, EngineEditError};

/// Collection of reversible history steps representing one applied edit transaction.
pub(crate) struct HistoryTransaction<T: Node> {
    /// Ordered steps captured while applying a single queue drain transaction.
    steps: Vec<HistoryStep<T>>,
    /// Content-state id before this transaction was applied.
    before_state_id: Option<u64>,
    /// Content-state id after this transaction was applied.
    after_state_id: Option<u64>,
}

impl<T: Node> HistoryTransaction<T> {
    /// Creates an empty transaction.
    pub(crate) fn new() -> Self {
        Self {
            steps: Vec::new(),
            before_state_id: None,
            after_state_id: None,
        }
    }

    /// Appends a history step to the transaction.
    pub(crate) fn push(&mut self, step: HistoryStep<T>) {
        self.steps.push(step);
    }

    /// Returns `true` when no reversible steps were captured.
    pub(crate) fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Returns the content-state id before this transaction.
    fn before_state_id(&self) -> Option<u64> {
        self.before_state_id
    }

    /// Returns the content-state id after this transaction.
    fn after_state_id(&self) -> Option<u64> {
        self.after_state_id
    }

    /// Sets the content-state ids tracked by this transaction.
    fn set_state_ids(&mut self, before_state_id: u64, after_state_id: u64) {
        self.before_state_id = Some(before_state_id);
        self.after_state_id = Some(after_state_id);
    }

    /// Updates the content-state id produced by this transaction.
    fn set_after_state_id(&mut self, after_state_id: u64) {
        self.after_state_id = Some(after_state_id);
    }

    /// Updates the newest matching coalesced SetParam step in this transaction, if present.
    fn try_update_coalesced_set_param(&mut self, node: NodeId, tick: u64, new_value: ParamValue) -> bool {
        for step in self.steps.iter_mut().rev() {
            let HistoryStep::SetParam(existing) = step else {
                continue;
            };

            if existing.behaviour == ParameterEventBehaviour::Coalesce && existing.node == node && existing.tick == tick
            {
                existing.new_value = new_value;
                return true;
            }
        }

        false
    }

    /// Moves coalescable SetParam steps into `previous` when they target the same node/tick.
    fn absorb_coalesced_set_params(&mut self, previous: &mut HistoryTransaction<T>) {
        let mut remaining_steps = Vec::with_capacity(self.steps.len());

        for step in self.steps.drain(..) {
            match step {
                HistoryStep::SetParam(incoming) => {
                    let merged = incoming.behaviour == ParameterEventBehaviour::Coalesce
                        && previous.try_update_coalesced_set_param(
                            incoming.node,
                            incoming.tick,
                            incoming.new_value.clone(),
                        );

                    if !merged {
                        remaining_steps.push(HistoryStep::SetParam(incoming));
                    }
                }
                other => remaining_steps.push(other),
            }
        }

        self.steps = remaining_steps;
    }

    /// Replays all steps in reverse order to undo this transaction.
    fn undo(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        for step in self.steps.iter_mut().rev() {
            step.undo(engine)?;
        }
        Ok(())
    }

    /// Replays all steps in forward order to redo this transaction.
    fn redo(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        for step in self.steps.iter_mut() {
            step.redo(engine)?;
        }
        Ok(())
    }

    /// Releases detached payloads held by history steps that will no longer be restorable.
    fn dispose(&mut self, engine: &mut Engine<T>) {
        for step in self.steps.iter_mut() {
            step.dispose(engine);
        }
    }
}

/// Currently open edit session, used to group multiple queue drains into one undo entry.
pub(crate) struct ActiveEditSession<T: Node> {
    /// Source of the session.
    pub(crate) origin: EditOrigin,
    /// Optional user-facing label for this session.
    pub(crate) label: Option<String>,
    /// Client-provided id linking begin/end.
    pub(crate) client_edit_id: String,
    /// Stable UI client instance id that owns this session, when available.
    pub(crate) ui_client_instance_id: Option<String>,
    /// Accumulated history transaction for this session.
    pub(crate) transaction: HistoryTransaction<T>,
}

impl<T: Node> ActiveEditSession<T> {
    /// Creates a new empty active edit session.
    pub(crate) fn new(
        origin: EditOrigin,
        label: Option<String>,
        client_edit_id: String,
        ui_client_instance_id: Option<String>,
    ) -> Self {
        Self {
            origin,
            label,
            client_edit_id,
            ui_client_instance_id,
            transaction: HistoryTransaction::new(),
        }
    }
}

/// Reversible unit describing how to undo/redo one applied edit operation.
pub(crate) enum HistoryStep<T: Node> {
    /// Parameter value update history.
    SetParam(SetParamHistory),
    /// Parameter control-state update history.
    SetParamControlState(SetParamControlStateHistory),
    /// Parameter constraints update history.
    SetParamConstraints(SetParamConstraintsHistory),
    /// Node metadata patch history.
    PatchMeta(PatchMetaHistory),
    /// Script configuration update history.
    SetScriptConfig(SetScriptConfigHistory),
    /// Child insertion history.
    AddNode(AddNodeHistory<T>),
    /// Node/subtree removal history.
    RemoveNode(RemoveNodeHistory<T>),
    /// Node move/reorder history.
    MoveNode(MoveNodeHistory),
    /// Node replacement history.
    ReplaceNode(ReplaceNodeHistory<T>),
}

impl<T: Node> HistoryStep<T> {
    /// Applies this step's inverse operation.
    fn undo(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
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

                let subtree = engine.collect_subtree(0, OP, step.node)?;
                engine.run_destroy_for_subtree(subtree.as_slice());
                let (parent, prev_sibling, next_sibling) = engine.node_position(0, OP, step.node)?;
                engine.detach_node(0, OP, step.node)?;

                let mut detached_nodes = Vec::with_capacity(subtree.len());
                for removed in subtree.into_iter().rev() {
                    let detached = engine.nodes.detach(removed).ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: removed,
                    })?;
                    detached_nodes.push((removed, detached));
                    engine.purge_param_cache_entry(removed);
                    engine.emit_event(EventKind::NodeDeleted { node: removed });
                }

                step.parent = parent;
                step.prev_sibling = prev_sibling;
                step.next_sibling = next_sibling;
                step.detached_nodes = Some(detached_nodes);

                engine.emit_event(EventKind::ChildRemoved {
                    parent,
                    child: step.node,
                });
            }
            Self::RemoveNode(step) => {
                const OP: &str = "UndoRemoveNode";

                let Some(detached_nodes) = step.detached_nodes.take() else {
                    return Ok(());
                };

                let created_ids: Vec<NodeId> = detached_nodes.iter().map(|(id, _)| *id).collect();
                for (id, node) in detached_nodes {
                    engine.nodes.reattach(id, node);
                    engine.populate_param_cache_entry(id);
                }

                attach_node_for_history(engine, OP, step.node, step.parent, step.prev_sibling, step.next_sibling)?;

                for node in created_ids.iter().rev().copied() {
                    engine.emit_event(EventKind::NodeCreated { node });
                }
                let decl_id = child_decl_id(engine, 0, OP, step.node)?;
                engine.emit_event(EventKind::ChildAdded {
                    parent: step.parent,
                    child: step.node,
                    decl_id,
                });

                let mut ready_ids = created_ids;
                ready_ids.reverse();
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

                    let detached_new_node = engine.nodes.detach(step.new_id).ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: step.new_id,
                    })?;
                    engine.purge_param_cache_entry(step.new_id);
                    engine.nodes.reattach(step.old_id, old_node);
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
                let detached_new_node = engine.nodes.detach(step.new_id).ok_or(EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: OP,
                    node: step.new_id,
                })?;
                engine.purge_param_cache_entry(step.new_id);

                engine.nodes.reattach(step.old_id, old_node);
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
    fn redo(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
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
                    engine.populate_param_cache_entry(id);
                }
                attach_node_for_history(engine, OP, step.node, step.parent, step.prev_sibling, step.next_sibling)?;

                for node in created_ids.iter().rev().copied() {
                    engine.emit_event(EventKind::NodeCreated { node });
                }
                let decl_id = child_decl_id(engine, 0, OP, step.node)?;
                engine.emit_event(EventKind::ChildAdded {
                    parent: step.parent,
                    child: step.node,
                    decl_id,
                });

                let mut ready_ids = created_ids;
                ready_ids.reverse();
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
                for removed in subtree.into_iter().rev() {
                    let detached = engine.nodes.detach(removed).ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: removed,
                    })?;
                    detached_nodes.push((removed, detached));
                    engine.purge_param_cache_entry(removed);
                    engine.emit_event(EventKind::NodeDeleted { node: removed });
                }

                engine.emit_event(EventKind::ChildRemoved {
                    parent,
                    child: step.node,
                });

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

                    let detached_old_node = engine.nodes.detach(step.old_id).ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: step.old_id,
                    })?;
                    engine.purge_param_cache_entry(step.old_id);
                    engine.nodes.reattach(step.new_id, new_node);
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
                let detached_old_node = engine.nodes.detach(step.old_id).ok_or(EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: OP,
                    node: step.old_id,
                })?;
                engine.purge_param_cache_entry(step.old_id);

                engine.nodes.reattach(step.new_id, new_node);
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
    fn dispose(&mut self, engine: &mut Engine<T>) {
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

/// Captured effect for a successful `SetParam` edit.
pub(crate) struct SetParamEffect {
    /// Edited parameter node id.
    pub(crate) node: NodeId,
    /// Parameter value before the edit.
    pub(crate) old_value: ParamValue,
    /// Parameter value after the edit.
    pub(crate) new_value: ParamValue,
    /// Coalescing strategy chosen for this edit request.
    pub(crate) behaviour: ParameterEventBehaviour,
    /// Tick at which this edit was applied.
    pub(crate) tick: u64,
}

/// Captured effect for a successful `SetParamControlState` operation.
pub(crate) struct SetParamControlStateEffect {
    /// Edited parameter node id.
    pub(crate) node: NodeId,
    /// Control state before the edit.
    pub(crate) old_state: ParameterControlState,
    /// Control state after the edit.
    pub(crate) new_state: ParameterControlState,
}

/// Captured effect for a successful `SetParamConstraints` operation.
pub(crate) struct SetParamConstraintsEffect {
    /// Edited parameter node id.
    pub(crate) node: NodeId,
    /// Constraints before the edit.
    pub(crate) old_constraints: ParameterConstraints,
    /// Constraints after the edit.
    pub(crate) new_constraints: ParameterConstraints,
    /// Parameter value before the edit.
    pub(crate) old_value: ParamValue,
    /// Parameter value after the edit.
    pub(crate) new_value: ParamValue,
}

/// Captured effect for a successful `PatchMeta` edit.
pub(crate) struct PatchMetaEffect {
    /// Edited node id.
    pub(crate) node: NodeId,
    /// Metadata before patch.
    pub(crate) old_meta: NodeMeta,
    /// Metadata after patch.
    pub(crate) new_meta: NodeMeta,
}

/// Captured effect for a successful `SetScriptConfig` edit.
pub(crate) struct SetScriptConfigEffect {
    /// Edited script node id.
    pub(crate) node: NodeId,
    /// Configuration before edit.
    pub(crate) old_config: ScriptNodeConfig,
    /// Configuration after edit.
    pub(crate) new_config: ScriptNodeConfig,
    /// Reload policy requested by this edit.
    pub(crate) force_reload: bool,
}

/// Captured effect for a successful `AddNode` edit.
pub(crate) struct AddNodeEffect {
    /// Inserted node id.
    pub(crate) node: NodeId,
    /// Parent receiving the inserted node.
    pub(crate) parent: NodeId,
    /// Sibling directly preceding the inserted node after application.
    pub(crate) prev_sibling: Option<NodeId>,
    /// Sibling directly following the inserted node after application.
    pub(crate) next_sibling: Option<NodeId>,
}

/// Captured effect for a successful `RemoveNode` edit.
pub(crate) struct RemoveNodeEffect<T: Node> {
    /// Removed root node id.
    pub(crate) node: NodeId,
    /// Parent from which the node was removed.
    pub(crate) parent: NodeId,
    /// Previous sibling position before removal.
    pub(crate) prev_sibling: Option<NodeId>,
    /// Next sibling position before removal.
    pub(crate) next_sibling: Option<NodeId>,
    /// Detached subtree payloads stored for undo.
    pub(crate) detached_nodes: Vec<(NodeId, T)>,
}

/// Captured effect for a successful `MoveNode` edit.
pub(crate) struct MoveNodeEffect {
    /// Moved node id.
    pub(crate) node: NodeId,
    /// Original parent before the move.
    pub(crate) old_parent: NodeId,
    /// Original previous sibling before the move.
    pub(crate) old_prev_sibling: Option<NodeId>,
    /// Original next sibling before the move.
    pub(crate) old_next_sibling: Option<NodeId>,
    /// New parent after the move.
    pub(crate) new_parent: NodeId,
    /// New previous sibling after the move.
    pub(crate) new_prev_sibling: Option<NodeId>,
    /// New next sibling after the move.
    pub(crate) new_next_sibling: Option<NodeId>,
}

/// Captured effect for a successful `ReplaceNode` edit.
pub(crate) struct ReplaceNodeEffect<T: Node> {
    /// Parent containing both old and replacement nodes.
    pub(crate) parent: NodeId,
    /// Original node id that was replaced.
    pub(crate) old_id: NodeId,
    /// New replacement node id.
    pub(crate) new_id: NodeId,
    /// Previous sibling position of the replaced node.
    pub(crate) prev_sibling: Option<NodeId>,
    /// Next sibling position of the replaced node.
    pub(crate) next_sibling: Option<NodeId>,
    /// Detached original node payload stored for undo.
    pub(crate) old_node: T,
}

/// Undo/redo payload for parameter edits.
pub(crate) struct SetParamHistory {
    /// Edited parameter node id.
    node: NodeId,
    /// Value before the edit.
    old_value: ParamValue,
    /// Value after the edit.
    new_value: ParamValue,
    /// Coalescing strategy chosen for this edit request.
    behaviour: ParameterEventBehaviour,
    /// Tick at which this edit was applied.
    tick: u64,
}

/// Undo/redo payload for control-state updates.
pub(crate) struct SetParamControlStateHistory {
    /// Edited parameter node id.
    node: NodeId,
    /// Control state before the edit.
    old_state: ParameterControlState,
    /// Control state after the edit.
    new_state: ParameterControlState,
}

/// Undo/redo payload for parameter-constraints updates.
pub(crate) struct SetParamConstraintsHistory {
    /// Edited parameter node id.
    node: NodeId,
    /// Constraints before the edit.
    old_constraints: ParameterConstraints,
    /// Constraints after the edit.
    new_constraints: ParameterConstraints,
    /// Value before the edit.
    old_value: ParamValue,
    /// Value after the edit.
    new_value: ParamValue,
}

/// Undo/redo payload for metadata patches.
pub(crate) struct PatchMetaHistory {
    /// Edited node id.
    node: NodeId,
    /// Metadata before patch.
    old_meta: NodeMeta,
    /// Metadata after patch.
    new_meta: NodeMeta,
}

/// Undo/redo payload for script-config edits.
pub(crate) struct SetScriptConfigHistory {
    /// Edited script node id.
    node: NodeId,
    /// Configuration before edit.
    old_config: ScriptNodeConfig,
    /// Configuration after edit.
    new_config: ScriptNodeConfig,
    /// Reload policy requested by this edit.
    force_reload: bool,
}

/// Undo/redo payload for add-node edits.
pub(crate) struct AddNodeHistory<T: Node> {
    /// Inserted node id.
    node: NodeId,
    /// Parent receiving the inserted node.
    parent: NodeId,
    /// Previous sibling after insertion.
    prev_sibling: Option<NodeId>,
    /// Next sibling after insertion.
    next_sibling: Option<NodeId>,
    /// Detached subtree payloads used to replay undo/redo.
    detached_nodes: Option<Vec<(NodeId, T)>>,
}

/// Undo/redo payload for remove-node edits.
pub(crate) struct RemoveNodeHistory<T: Node> {
    /// Removed root node id.
    node: NodeId,
    /// Parent from which the node was removed.
    parent: NodeId,
    /// Previous sibling before removal.
    prev_sibling: Option<NodeId>,
    /// Next sibling before removal.
    next_sibling: Option<NodeId>,
    /// Detached subtree payloads used to replay undo/redo.
    detached_nodes: Option<Vec<(NodeId, T)>>,
}

/// Undo/redo payload for move-node edits.
pub(crate) struct MoveNodeHistory {
    /// Moved node id.
    node: NodeId,
    /// Original parent before move.
    old_parent: NodeId,
    /// Original previous sibling before move.
    old_prev_sibling: Option<NodeId>,
    /// Original next sibling before move.
    old_next_sibling: Option<NodeId>,
    /// Destination parent after move.
    new_parent: NodeId,
    /// Destination previous sibling after move.
    new_prev_sibling: Option<NodeId>,
    /// Destination next sibling after move.
    new_next_sibling: Option<NodeId>,
    /// Tracks whether the node is currently at the moved position.
    at_new_position: bool,
}

/// Undo/redo payload for replace-node edits.
pub(crate) struct ReplaceNodeHistory<T: Node> {
    /// Parent containing old/new nodes.
    parent: NodeId,
    /// Original node id.
    old_id: NodeId,
    /// Replacement node id.
    new_id: NodeId,
    /// Previous sibling position.
    prev_sibling: Option<NodeId>,
    /// Next sibling position.
    next_sibling: Option<NodeId>,
    /// Detached old-node payload used by undo.
    old_node: Option<T>,
    /// Detached new-node payload used by redo.
    new_node: Option<T>,
}

impl<T: Node> From<SetParamEffect> for HistoryStep<T> {
    fn from(effect: SetParamEffect) -> Self {
        Self::SetParam(SetParamHistory {
            node: effect.node,
            old_value: effect.old_value,
            new_value: effect.new_value,
            behaviour: effect.behaviour,
            tick: effect.tick,
        })
    }
}

impl<T: Node> From<SetParamControlStateEffect> for HistoryStep<T> {
    fn from(effect: SetParamControlStateEffect) -> Self {
        Self::SetParamControlState(SetParamControlStateHistory {
            node: effect.node,
            old_state: effect.old_state,
            new_state: effect.new_state,
        })
    }
}

impl<T: Node> From<SetParamConstraintsEffect> for HistoryStep<T> {
    fn from(effect: SetParamConstraintsEffect) -> Self {
        Self::SetParamConstraints(SetParamConstraintsHistory {
            node: effect.node,
            old_constraints: effect.old_constraints,
            new_constraints: effect.new_constraints,
            old_value: effect.old_value,
            new_value: effect.new_value,
        })
    }
}

impl<T: Node> From<PatchMetaEffect> for HistoryStep<T> {
    fn from(effect: PatchMetaEffect) -> Self {
        Self::PatchMeta(PatchMetaHistory {
            node: effect.node,
            old_meta: effect.old_meta,
            new_meta: effect.new_meta,
        })
    }
}

impl<T: Node> From<SetScriptConfigEffect> for HistoryStep<T> {
    fn from(effect: SetScriptConfigEffect) -> Self {
        Self::SetScriptConfig(SetScriptConfigHistory {
            node: effect.node,
            old_config: effect.old_config,
            new_config: effect.new_config,
            force_reload: effect.force_reload,
        })
    }
}

impl<T: Node> From<AddNodeEffect> for HistoryStep<T> {
    fn from(effect: AddNodeEffect) -> Self {
        Self::AddNode(AddNodeHistory {
            node: effect.node,
            parent: effect.parent,
            prev_sibling: effect.prev_sibling,
            next_sibling: effect.next_sibling,
            detached_nodes: None,
        })
    }
}

impl<T: Node> From<RemoveNodeEffect<T>> for HistoryStep<T> {
    fn from(effect: RemoveNodeEffect<T>) -> Self {
        Self::RemoveNode(RemoveNodeHistory {
            node: effect.node,
            parent: effect.parent,
            prev_sibling: effect.prev_sibling,
            next_sibling: effect.next_sibling,
            detached_nodes: Some(effect.detached_nodes),
        })
    }
}

impl<T: Node> From<MoveNodeEffect> for HistoryStep<T> {
    fn from(effect: MoveNodeEffect) -> Self {
        Self::MoveNode(MoveNodeHistory {
            node: effect.node,
            old_parent: effect.old_parent,
            old_prev_sibling: effect.old_prev_sibling,
            old_next_sibling: effect.old_next_sibling,
            new_parent: effect.new_parent,
            new_prev_sibling: effect.new_prev_sibling,
            new_next_sibling: effect.new_next_sibling,
            at_new_position: true,
        })
    }
}

impl<T: Node> From<ReplaceNodeEffect<T>> for HistoryStep<T> {
    fn from(effect: ReplaceNodeEffect<T>) -> Self {
        Self::ReplaceNode(ReplaceNodeHistory {
            parent: effect.parent,
            old_id: effect.old_id,
            new_id: effect.new_id,
            prev_sibling: effect.prev_sibling,
            next_sibling: effect.next_sibling,
            old_node: Some(effect.old_node),
            new_node: None,
        })
    }
}

impl<T: Node> Engine<T> {
    fn allocate_history_state_id(&mut self) -> u64 {
        let state_id = self.next_history_state_id;
        self.next_history_state_id = self.next_history_state_id.saturating_add(1);
        state_id
    }

    pub(crate) fn push_step_into_active_history_transaction(&mut self, step: HistoryStep<T>) {
        let before_state_id = self.current_history_state_id;
        let after_state_id = self.allocate_history_state_id();
        let active = self
            .active_edit_session
            .as_mut()
            .expect("active edit session should exist when capturing a session history step");

        if active.transaction.before_state_id.is_none() {
            active.transaction.before_state_id = Some(before_state_id);
        }
        active.transaction.push(step);
        active.transaction.after_state_id = Some(after_state_id);
        self.current_history_state_id = after_state_id;
    }

    pub(crate) fn record_single_history_step(&mut self, step: HistoryStep<T>) {
        self.clear_redo_history();
        if self.active_edit_session.is_some() {
            self.push_step_into_active_history_transaction(step);
            return;
        }

        let mut transaction = HistoryTransaction::new();
        transaction.push(step);
        self.push_undo_transaction(transaction);
    }

    /// Records a parameter control-state update in undo/redo history.
    pub(crate) fn record_set_param_control_state_history(&mut self, effect: SetParamControlStateEffect) {
        self.record_single_history_step(effect.into());
    }

    /// Records a parameter-constraints update in undo/redo history.
    pub(crate) fn record_set_param_constraints_history(&mut self, effect: SetParamConstraintsEffect) {
        self.record_single_history_step(effect.into());
    }

    fn drop_active_edit_session_history(&mut self) {
        let Some(mut active) = self.active_edit_session.take() else {
            return;
        };

        let dropped_steps = active.transaction.steps.len();
        if dropped_steps > 0 {
            eprintln!(
                "[gc-history] drop active edit session during history clear: client_edit_id='{}' steps={}",
                active.client_edit_id, dropped_steps
            );
        }
        active.transaction.dispose(self);
    }

    /// Cancels the active UI edit session owned by `client_instance_id`, if any.
    pub fn cancel_active_ui_edit_session_for_client(&mut self, client_instance_id: &str) -> bool {
        let should_cancel = self.active_edit_session.as_ref().is_some_and(|active| {
            active.origin == EditOrigin::Ui && active.ui_client_instance_id.as_deref() == Some(client_instance_id)
        });
        if !should_cancel {
            return false;
        }

        self.drop_active_edit_session_history();
        true
    }

    /// Drops all redo transactions and releases any detached payloads they own.
    pub(crate) fn clear_redo_history(&mut self) {
        while let Some(mut transaction) = self.redo_stack.pop() {
            transaction.dispose(self);
        }
    }

    /// Pushes a non-empty transaction onto the undo stack.
    pub(crate) fn push_undo_transaction(&mut self, mut transaction: HistoryTransaction<T>) {
        if transaction.is_empty() {
            return;
        }

        let before_state_id = transaction.before_state_id().unwrap_or(self.current_history_state_id);
        let after_state_id = transaction
            .after_state_id()
            .unwrap_or_else(|| self.allocate_history_state_id());
        transaction.set_state_ids(before_state_id, after_state_id);

        // eprintln!("[gc-history] push undo tx: steps={} undo_before={} redo_current={}", transaction.steps.len(), self.undo_stack.len(), self.redo_stack.len());

        if let Some(previous) = self.undo_stack.last_mut() {
            transaction.absorb_coalesced_set_params(previous);
            if transaction.is_empty() {
                previous.set_after_state_id(after_state_id);
                self.current_history_state_id = after_state_id;
                return;
            }
        }

        if !transaction.is_empty() {
            self.undo_stack.push(transaction);
            self.current_history_state_id = after_state_id;
            // eprintln!("[gc-history] push undo tx done: undo_after={} redo_current={}", self.undo_stack.len(), self.redo_stack.len());
        }
    }

    /// Returns the number of available undo transactions.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the number of available redo transactions.
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clears undo/redo history and releases detached node payloads that can no longer be restored.
    ///
    /// Any active edit session history is also discarded so stale pre-baseline
    /// session edits cannot be committed later.
    pub fn clear_history(&mut self) {
        self.clear_redo_history();
        while let Some(mut transaction) = self.undo_stack.pop() {
            transaction.dispose(self);
        }
        self.drop_active_edit_session_history();
        self.current_history_state_id = 0;
        self.next_history_state_id = 1;
    }

    /// Reverts the last applied edit transaction.
    pub fn undo(&mut self) -> Result<bool, EngineEditError> {
        let Some(mut transaction) = self.undo_stack.pop() else {
            // eprintln!("[gc-history] undo: no-op (undo_len=0, redo_len={})", self.redo_stack.len());
            return Ok(false);
        };
        let before_state_id = transaction.before_state_id().unwrap_or(0);

        // eprintln!("[gc-history] undo start: tx_steps={} undo_after_pop={} redo_before={}", transaction.steps.len(), self.undo_stack.len(), self.redo_stack.len());
        transaction.undo(self)?;
        self.sync_missing_reference_warnings();
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        self.redo_stack.push(transaction);
        self.current_history_state_id = before_state_id;
        // eprintln!("[gc-history] undo done: undo_len={} redo_len={}", self.undo_stack.len(), self.redo_stack.len());
        Ok(true)
    }

    /// Reapplies the last undone edit transaction.
    pub fn redo(&mut self) -> Result<bool, EngineEditError> {
        let Some(mut transaction) = self.redo_stack.pop() else {
            // eprintln!("[gc-history] redo: no-op (undo_len={}, redo_len=0)", self.undo_stack.len());
            return Ok(false);
        };
        let after_state_id = transaction.after_state_id().unwrap_or(self.current_history_state_id);

        // eprintln!("[gc-history] redo start: tx_steps={} undo_before={} redo_after_pop={}", transaction.steps.len(), self.undo_stack.len(), self.redo_stack.len());
        transaction.redo(self)?;
        self.sync_missing_reference_warnings();
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        self.undo_stack.push(transaction);
        self.current_history_state_id = after_state_id;
        // eprintln!("[gc-history] redo done: undo_len={} redo_len={}", self.undo_stack.len(), self.redo_stack.len());
        Ok(true)
    }

    /// Returns the logical content-state id for the current graph.
    pub fn current_history_state_id(&self) -> u64 {
        self.current_history_state_id
    }

    /// Returns `true` when an edit session is currently active.
    pub fn has_active_edit_session(&self) -> bool {
        self.active_edit_session.is_some()
    }

    /// Returns the currently active client edit id, when present.
    pub fn active_edit_session_id(&self) -> Option<&str> {
        self.active_edit_session
            .as_ref()
            .map(|session| session.client_edit_id.as_str())
    }

    /// Returns the origin of the currently active edit session.
    pub fn active_edit_session_origin(&self) -> Option<EditOrigin> {
        self.active_edit_session.as_ref().map(|session| session.origin)
    }

    /// Returns the label of the currently active edit session.
    pub fn active_edit_session_label(&self) -> Option<&str> {
        self.active_edit_session
            .as_ref()
            .and_then(|session| session.label.as_deref())
    }

    /// Returns the stable UI client instance id that owns the active edit session.
    pub fn active_edit_session_ui_client_instance_id(&self) -> Option<&str> {
        self.active_edit_session
            .as_ref()
            .and_then(|session| session.ui_client_instance_id.as_deref())
    }
}
