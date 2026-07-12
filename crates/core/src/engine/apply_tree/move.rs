use super::*;

impl<T: Node> Engine<T> {
    /// Applies a move-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_move_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        new_parent: NodeId,
        new_prev_sibling: Option<NodeId>,
    ) -> Result<MoveNodeEffect, EngineEditError> {
        const OP: &str = "MoveNode";

        if node == self.root {
            return Err(EngineEditError::CannotMutateRoot {
                edit_index,
                operation: OP,
                node,
            });
        }

        if !self.nodes.contains(node) {
            return Err(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            });
        }

        if !self.nodes.contains(new_parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index,
                operation: OP,
                parent: new_parent,
            });
        }

        if node == new_parent || self.is_descendant_of(new_parent, node) {
            return Err(EngineEditError::CycleDetected {
                edit_index,
                operation: OP,
                node,
                new_parent,
            });
        }

        if let Some(sibling) = new_prev_sibling
            && sibling == node
        {
            return Err(EngineEditError::InvalidSiblingReference {
                edit_index,
                operation: OP,
                node,
                sibling,
            });
        }

        self.validate_item_roots_for_move(edit_index, OP, node, new_parent)?;

        let (old_parent, old_prev_sibling, old_next_sibling) = self.node_position(edit_index, OP, node)?;

        let detached_parent = self.detach_node(edit_index, OP, node)?;
        if let Some(new_prev_sibling) = new_prev_sibling {
            self.attach_node(edit_index, OP, node, new_parent, Some(new_prev_sibling))?;
        } else {
            // MoveNode uses previous-sibling semantics: None means "become first child".
            let new_next_sibling = self
                .nodes
                .get(new_parent)
                .and_then(|entry| entry.node_data().first_child);
            self.attach_node_between(edit_index, OP, node, new_parent, None, new_next_sibling)?;
        }

        let new_node_data = self
            .nodes
            .get(node)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?
            .node_data();
        let new_prev_sibling = new_node_data.prev_sibling;
        let new_next_sibling = new_node_data.next_sibling;

        if detached_parent == new_parent {
            self.emit_inbox_event(EventKind::ChildReordered {
                parent: new_parent,
                child: node,
            });
            if let Some(children) = self.ui_direct_children(new_parent) {
                self.push_ui_graph_transaction(vec![UiGraphOp::ChildrenReordered {
                    parent: new_parent,
                    children,
                }]);
            }
        } else {
            self.emit_inbox_event(EventKind::ChildMoved {
                child: node,
                old_parent: detached_parent,
                new_parent,
            });
            self.push_ui_graph_transaction(vec![UiGraphOp::NodeMoved {
                node,
                old_parent: Some(detached_parent),
                new_parent: Some(new_parent),
                old_parent_after: self.ui_children_order_patch(detached_parent),
                new_parent_after: self.ui_children_order_patch(new_parent),
            }]);
        }

        // When the parent changed the effective_enabled of the whole subtree may have changed.
        if detached_parent != new_parent {
            let changes = self.subtree_effective_enabled_changes(node);
            self.queue_effective_enabled_callbacks(&changes)?;
        }

        Ok(MoveNodeEffect {
            node,
            old_parent,
            old_prev_sibling,
            old_next_sibling,
            new_parent,
            new_prev_sibling,
            new_next_sibling,
        })
    }
}
