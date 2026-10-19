use super::*;
use crate::engine::apply_opaque::AuthoredNodeState;
use crate::node::NodeCreationContext;

#[derive(Clone, Copy)]
struct Position {
    parent: NodeId,
    prev: Option<NodeId>,
    next: Option<NodeId>,
}

impl Position {
    fn new(parent: NodeId, prev: Option<NodeId>, next: Option<NodeId>) -> Self {
        Self { parent, prev, next }
    }
}

/// Original history owns non-cloneable payloads. These records borrow their transitions
/// and retain live scalar state rather than assuming recorded forward values are current.
enum ReplayBackup {
    Unchanged,
    Authored {
        node: NodeId,
        state: Box<AuthoredNodeState>,
    },
    Subtree {
        detached: bool,
        position: Position,
    },
    Move {
        position: Position,
        at_new_position: bool,
    },
    Replace {
        old_detached: bool,
    },
}

impl ReplayBackup {
    fn capture<T: Node>(step: &HistoryStep<T>, engine: &Engine<T>) -> Result<Self, EngineEditError> {
        let node = match step {
            HistoryStep::AuthoredNode(step) => step.node,
            HistoryStep::SetParam(step) => step.node,
            HistoryStep::SetParamControlState(step) => step.node,
            HistoryStep::SetParamConstraints(step) => step.node,
            HistoryStep::PatchMeta(step) => step.node,
            HistoryStep::SetScriptConfig(step) => step.node,
            HistoryStep::AddNode(step) => {
                return Ok(Self::Subtree {
                    detached: step.detached_nodes.is_some(),
                    position: Position::new(step.parent, step.prev_sibling, step.next_sibling),
                });
            }
            HistoryStep::RemoveNode(step) => {
                return Ok(Self::Subtree {
                    detached: step.detached_nodes.is_some(),
                    position: Position::new(step.parent, step.prev_sibling, step.next_sibling),
                });
            }
            HistoryStep::MoveNode(step) => {
                return Ok(engine
                    .node_position(0, "HistoryReplay", step.node)
                    .map(|(parent, prev, next)| Self::Move {
                        position: Position::new(parent, prev, next),
                        at_new_position: step.at_new_position,
                    })
                    .unwrap_or(Self::Unchanged));
            }
            HistoryStep::ReplaceNode(step) => {
                return Ok(Self::Replace {
                    old_detached: step.old_node.is_some(),
                });
            }
        };
        let Some(target) = engine.nodes.get(node) else {
            // Let the actual operation report its precise missing-target error.
            return Ok(Self::Unchanged);
        };
        let state = AuthoredNodeState::capture(target).map_err(|message| EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "HistoryReplayCheckpoint",
            node,
            node_type: target.get_type().to_owned(),
            message,
        })?;
        Ok(Self::Authored {
            node,
            state: Box::new(state),
        })
    }

    fn restore<T: Node>(
        self,
        step: &mut HistoryStep<T>,
        engine: &mut Engine<T>,
        restored: &mut Vec<NodeId>,
    ) -> Result<(), EngineEditError> {
        match self {
            Self::Unchanged => Ok(()),
            Self::Authored { node, state } => engine.restore_authored_node_state(node, &state),
            Self::Subtree { detached, position } => {
                let currently_detached = match step {
                    HistoryStep::AddNode(step) => &step.detached_nodes,
                    HistoryStep::RemoveNode(step) => &step.detached_nodes,
                    _ => unreachable!("subtree backup matches its step"),
                };
                if !detached && let Some(nodes) = currently_detached {
                    restored.extend(nodes.iter().rev().map(|(id, _)| *id));
                }
                let changed = detached != currently_detached.is_some();
                let result = if changed {
                    match step {
                        HistoryStep::AddNode(_) if detached => step.undo(engine),
                        HistoryStep::AddNode(_) => step.redo(engine),
                        HistoryStep::RemoveNode(_) if detached => step.redo(engine),
                        HistoryStep::RemoveNode(_) => step.undo(engine),
                        _ => unreachable!("subtree backup matches its step"),
                    }
                } else {
                    Ok(())
                };
                match step {
                    HistoryStep::AddNode(step) => {
                        step.parent = position.parent;
                        step.prev_sibling = position.prev;
                        step.next_sibling = position.next;
                    }
                    HistoryStep::RemoveNode(step) => {
                        step.parent = position.parent;
                        step.prev_sibling = position.prev;
                        step.next_sibling = position.next;
                    }
                    _ => unreachable!("subtree backup matches its step"),
                }
                result
            }
            Self::Move {
                position,
                at_new_position,
            } => {
                let HistoryStep::MoveNode(step) = step else {
                    unreachable!("move backup")
                };
                // Failed replay may have committed its move before a callback rejected.
                engine.detach_node(0, "HistoryRollbackMove", step.node)?;
                super::replay::attach_node_for_history(
                    engine,
                    "HistoryRollbackMove",
                    step.node,
                    position.parent,
                    position.prev,
                    position.next,
                )?;
                step.at_new_position = at_new_position;
                let changes = engine.subtree_effective_enabled_changes(step.node);
                engine.queue_effective_enabled_callbacks(&changes)
            }
            Self::Replace { old_detached } => {
                let HistoryStep::ReplaceNode(replace) = step else {
                    unreachable!("replace backup")
                };
                if old_detached == replace.old_node.is_some() {
                    return Ok(());
                }
                restored.push(replace.node);
                if old_detached {
                    step.redo(engine)
                } else {
                    step.undo(engine)
                }
            }
        }
    }
}

impl<T: Node> HistoryTransaction<T> {
    pub(super) fn replay_atomically(&mut self, engine: &mut Engine<T>, undo: bool) -> Result<(), EngineEditError> {
        if engine.edit_journal.is_some() {
            return Err(EngineEditError::NodeMutationRejected {
                edit_index: 0,
                operation: "HistoryReplay",
                node: engine.root,
                node_type: engine
                    .nodes
                    .get(engine.root)
                    .expect("engine root")
                    .get_type()
                    .to_owned(),
                message: "history cannot be replayed inside another edit operation".into(),
            });
        }
        let checkpoint = engine.edit_checkpoint();
        engine.history_replay = true;
        let mut backups = Vec::with_capacity(self.steps.len());
        let indices = if undo {
            (0..self.steps.len()).rev().collect::<Vec<_>>()
        } else {
            (0..self.steps.len()).collect::<Vec<_>>()
        };
        let batch = self.is_same_parent_add_batch() || self.is_independent_remove_batch(engine);
        let result = (|| {
            for index in indices {
                backups.push((index, ReplayBackup::capture(&self.steps[index], engine)?));
                if !batch {
                    if undo {
                        self.steps[index].undo(engine)?;
                    } else {
                        self.steps[index].redo(engine)?;
                    }
                }
            }
            if batch {
                if undo {
                    self.undo(engine)?;
                } else {
                    self.redo(engine)?;
                }
            }
            engine.sync_missing_reference_warnings();
            engine.rebuild_user_context_registry_from_nodes();
            engine.mark_user_context_graph_changed();
            // Only replay notifications are delivered here. Earlier pending work is
            // isolated by the checkpoint and keeps its ordinary dispatch semantics.
            engine.settle_edit_notifications(32)
        })();
        if result.is_ok() {
            engine.history_replay = false;
            engine.commit_edit_checkpoint(checkpoint);
            return result;
        }

        engine.rollback_replay = true;
        let mut rollback_error = None;
        let mut restored = Vec::new();
        for (index, backup) in backups.into_iter().rev() {
            if let Err(error) = backup.restore(&mut self.steps[index], engine, &mut restored)
                && rollback_error.is_none()
            {
                rollback_error = Some(error);
            }
        }
        engine.rollback_replay = false;
        let mut reactivated = std::collections::HashSet::new();
        restored.retain(|id| engine.nodes.contains(*id) && reactivated.insert(*id));
        if let Err(error) = engine.run_node_ready_for_batch(&restored, NodeCreationContext::HistoryRestore)
            && rollback_error.is_none()
        {
            rollback_error = Some(error);
        }
        if let Err(error) = engine.rollback_edit_checkpoint(checkpoint)
            && rollback_error.is_none()
        {
            rollback_error = Some(error);
        }
        engine.history_replay = false;
        let original = result.expect_err("replay rejected");
        Err(match rollback_error {
            Some(rollback) => EngineEditError::AtomicRollbackFailed {
                original: Box::new(original),
                rollback: Box::new(rollback),
            },
            None => original,
        })
    }
}
