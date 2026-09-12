use super::replay::{emit_added_node_events, restore_add_node, undo_add_node};
use super::*;
use crate::node::NodeCreationContext;

impl<T: Node> HistoryTransaction<T> {
    /// Same-parent additions are disjoint and can share lifecycle and UI snapshots.
    pub(super) fn is_same_parent_add_batch(&self) -> bool {
        if self.steps.len() < 2 {
            return false;
        }
        let HistoryStep::AddNode(first) = &self.steps[0] else {
            return false;
        };
        self.steps
            .iter()
            .all(|step| matches!(step, HistoryStep::AddNode(add) if add.parent == first.parent))
    }

    pub(super) fn undo_add_batch(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        let mut all_nodes = Vec::new();
        for step in &self.steps {
            let HistoryStep::AddNode(add) = step else {
                unreachable!("batch eligibility was checked");
            };
            if add.detached_nodes.is_some() || !engine.nodes.contains(add.node) {
                return self.undo_steps_individually(engine);
            }
            all_nodes.extend(engine.collect_subtree(0, "UndoAddNode", add.node)?);
        }

        engine.run_destroy_for_subtree(&all_nodes);
        for step in self.steps.iter_mut().rev() {
            let HistoryStep::AddNode(add) = step else {
                unreachable!("batch eligibility was checked");
            };
            undo_add_node(engine, add, false)?;
        }
        Ok(())
    }

    pub(super) fn redo_add_batch(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        if self
            .steps
            .iter()
            .any(|step| matches!(step, HistoryStep::AddNode(add) if add.detached_nodes.is_none()))
        {
            return self.redo_steps_individually(engine);
        }

        let mut restored = Vec::with_capacity(self.steps.len());
        let mut all_ready_ids = Vec::new();
        for step in &mut self.steps {
            let HistoryStep::AddNode(add) = step else {
                unreachable!("batch eligibility was checked");
            };
            let ready_ids = restore_add_node(engine, add)?.expect("all additions have detached payloads");
            emit_added_node_events(engine, add, &ready_ids)?;
            all_ready_ids.extend(ready_ids.iter().copied());
            restored.push((add.node, add.parent, ready_ids));
        }

        let catalog_snapshot = engine.build_process_tree_snapshot();
        let mut ops = Vec::with_capacity(restored.len());
        for (root, parent, ready_ids) in restored {
            let mut nodes = Vec::with_capacity(ready_ids.len());
            for node_id in ready_ids {
                let snapshot = engine
                    .ui_node_dto_for_event_with_catalog_snapshot(node_id, catalog_snapshot.as_ref())
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: "RedoAddNode",
                        node: node_id,
                    })?;
                nodes.push(snapshot);
            }
            ops.push(UiGraphOp::SubtreeInserted {
                root,
                parent,
                nodes,
                parent_children_after: engine.ui_direct_children(parent).unwrap_or_default(),
            });
        }
        engine.push_ui_graph_transaction(ops);
        engine.run_node_ready_for_batch(&all_ready_ids, NodeCreationContext::Fresh)
    }

    fn undo_steps_individually(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        for step in self.steps.iter_mut().rev() {
            step.undo(engine)?;
        }
        Ok(())
    }

    fn redo_steps_individually(&mut self, engine: &mut Engine<T>) -> Result<(), EngineEditError> {
        for step in &mut self.steps {
            step.redo(engine)?;
        }
        Ok(())
    }
}
