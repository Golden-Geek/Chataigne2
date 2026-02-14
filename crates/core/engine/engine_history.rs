use crate::events::EventKind;
use crate::node::{Node, NodeId};
use crate::parameter::{ParamValue, ParameterEventBehaviour};

use super::{Engine, EngineEditError};

/// Collection of reversible history steps representing one applied edit transaction.
pub(crate) struct HistoryTransaction<T: Node> {
    /// Ordered steps captured while applying a single queue drain transaction.
    steps: Vec<HistoryStep<T>>,
}

impl<T: Node> HistoryTransaction<T> {
    /// Creates an empty transaction.
    pub(crate) fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Appends a history step to the transaction.
    pub(crate) fn push(&mut self, step: HistoryStep<T>) {
        self.steps.push(step);
    }

    /// Returns `true` when no reversible steps were captured.
    pub(crate) fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Updates the newest matching coalesced SetParam step in this transaction, if present.
    fn try_update_coalesced_set_param(&mut self, node: NodeId, tick: u64, new_value: ParamValue) -> bool {
        for step in self.steps.iter_mut().rev() {
            let HistoryStep::SetParam(existing) = step else {
                continue;
            };

            if existing.behaviour == ParameterEventBehaviour::Coalesce && existing.node == node && existing.tick == tick {
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
                        && previous.try_update_coalesced_set_param(incoming.node, incoming.tick, incoming.new_value.clone());

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

/// Reversible unit describing how to undo/redo one applied edit operation.
pub(crate) enum HistoryStep<T: Node> {
    /// Parameter value update history.
    SetParam(SetParamHistory),
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
            Self::AddNode(step) => {
                const OP: &str = "UndoAddNode";

                if step.detached_node.is_some() {
                    return Ok(());
                }

                let parent = engine.detach_node(0, OP, step.node)?;
                let detached_node = engine.nodes.detach(step.node).ok_or(EngineEditError::NodeNotFound { edit_index: 0, operation: OP, node: step.node })?;

                step.parent = parent;
                step.detached_node = Some(detached_node);

                engine.emit_event(EventKind::NodeDeleted { node: step.node });
                engine.emit_event(EventKind::ChildRemoved { parent, child: step.node });
            }
            Self::RemoveNode(step) => {
                const OP: &str = "UndoRemoveNode";

                let Some(detached_nodes) = step.detached_nodes.take() else {
                    return Ok(());
                };

                let created_ids: Vec<NodeId> = detached_nodes.iter().map(|(id, _)| *id).collect();
                for (id, node) in detached_nodes {
                    engine.nodes.reattach(id, node);
                }

                engine.attach_node_between(0, OP, step.node, step.parent, step.prev_sibling, step.next_sibling)?;

                for node in created_ids.into_iter().rev() {
                    engine.emit_event(EventKind::NodeCreated { node });
                }
                engine.emit_event(EventKind::ChildAdded { parent: step.parent, child: step.node });
            }
            Self::MoveNode(step) => {
                const OP: &str = "UndoMoveNode";

                if !step.at_new_position {
                    return Ok(());
                }

                let current_parent = engine.detach_node(0, OP, step.node)?;
                engine.attach_node_between(0, OP, step.node, step.old_parent, step.old_prev_sibling, step.old_next_sibling)?;

                if current_parent == step.old_parent {
                    engine.emit_event(EventKind::ChildReordered { parent: step.old_parent, child: step.node });
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

                engine.detach_node(0, OP, step.new_id)?;
                let detached_new_node = engine.nodes.detach(step.new_id).ok_or(EngineEditError::NodeNotFound { edit_index: 0, operation: OP, node: step.new_id })?;

                engine.nodes.reattach(step.old_id, old_node);
                engine.attach_node_between(0, OP, step.old_id, step.parent, step.prev_sibling, step.next_sibling)?;

                let first_child = engine.nodes.get(step.old_id).ok_or(EngineEditError::NodeNotFound { edit_index: 0, operation: OP, node: step.old_id })?.node_data().first_child;
                engine.reparent_child_chain(0, OP, first_child, step.old_id)?;

                engine.emit_event(EventKind::NodeCreated { node: step.old_id });
                engine.emit_event(EventKind::ChildReplaced {
                    parent: step.parent,
                    old: step.new_id,
                    new: step.old_id,
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
            Self::AddNode(step) => {
                const OP: &str = "RedoAddNode";

                let Some(node) = step.detached_node.take() else {
                    return Ok(());
                };

                engine.nodes.reattach(step.node, node);
                engine.attach_node_between(0, OP, step.node, step.parent, step.prev_sibling, step.next_sibling)?;

                engine.emit_event(EventKind::NodeCreated { node: step.node });
                engine.emit_event(EventKind::ChildAdded { parent: step.parent, child: step.node });
            }
            Self::RemoveNode(step) => {
                const OP: &str = "RedoRemoveNode";

                if step.detached_nodes.is_some() {
                    return Ok(());
                }

                let subtree = engine.collect_subtree(0, OP, step.node)?;
                let (parent, prev_sibling, next_sibling) = engine.node_position(0, OP, step.node)?;
                engine.detach_node(0, OP, step.node)?;

                let mut detached_nodes = Vec::with_capacity(subtree.len());
                for removed in subtree.into_iter().rev() {
                    let detached = engine.nodes.detach(removed).ok_or(EngineEditError::NodeNotFound { edit_index: 0, operation: OP, node: removed })?;
                    detached_nodes.push((removed, detached));
                    engine.emit_event(EventKind::NodeDeleted { node: removed });
                }

                engine.emit_event(EventKind::ChildRemoved { parent, child: step.node });

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
                engine.attach_node_between(0, OP, step.node, step.new_parent, step.new_prev_sibling, step.new_next_sibling)?;

                if current_parent == step.new_parent {
                    engine.emit_event(EventKind::ChildReordered { parent: step.new_parent, child: step.node });
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

                engine.detach_node(0, OP, step.old_id)?;
                let detached_old_node = engine.nodes.detach(step.old_id).ok_or(EngineEditError::NodeNotFound { edit_index: 0, operation: OP, node: step.old_id })?;

                engine.nodes.reattach(step.new_id, new_node);
                engine.attach_node_between(0, OP, step.new_id, step.parent, step.prev_sibling, step.next_sibling)?;

                let first_child = engine.nodes.get(step.new_id).ok_or(EngineEditError::NodeNotFound { edit_index: 0, operation: OP, node: step.new_id })?.node_data().first_child;
                engine.reparent_child_chain(0, OP, first_child, step.new_id)?;

                engine.emit_event(EventKind::NodeCreated { node: step.new_id });
                engine.emit_event(EventKind::ChildReplaced {
                    parent: step.parent,
                    old: step.old_id,
                    new: step.new_id,
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
            Self::SetParam(_) | Self::MoveNode(_) => {}
            Self::AddNode(step) => {
                if let Some(node) = step.detached_node.take() {
                    purge_detached_node(engine, step.node, node);
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

/// Temporarily reattaches then permanently removes a previously detached node payload.
fn purge_detached_node<T: Node>(engine: &mut Engine<T>, id: NodeId, node: T) {
    engine.nodes.reattach(id, node);
    let _ = engine.nodes.remove(id);
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
    /// Detached payload used to replay undo/redo.
    detached_node: Option<T>,
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

impl<T: Node> From<AddNodeEffect> for HistoryStep<T> {
    fn from(effect: AddNodeEffect) -> Self {
        Self::AddNode(AddNodeHistory {
            node: effect.node,
            parent: effect.parent,
            prev_sibling: effect.prev_sibling,
            next_sibling: effect.next_sibling,
            detached_node: None,
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

        if let Some(previous) = self.undo_stack.last_mut() {
            transaction.absorb_coalesced_set_params(previous);
        }

        if !transaction.is_empty() {
            self.undo_stack.push(transaction);
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
    pub fn clear_history(&mut self) {
        self.clear_redo_history();
        while let Some(mut transaction) = self.undo_stack.pop() {
            transaction.dispose(self);
        }
    }

    /// Reverts the last applied edit transaction.
    pub fn undo(&mut self) -> Result<bool, EngineEditError> {
        let Some(mut transaction) = self.undo_stack.pop() else {
            return Ok(false);
        };

        transaction.undo(self)?;
        self.redo_stack.push(transaction);
        Ok(true)
    }

    /// Reapplies the last undone edit transaction.
    pub fn redo(&mut self) -> Result<bool, EngineEditError> {
        let Some(mut transaction) = self.redo_stack.pop() else {
            return Ok(false);
        };

        transaction.redo(self)?;
        self.undo_stack.push(transaction);
        Ok(true)
    }
}
