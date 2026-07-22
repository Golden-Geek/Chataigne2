use super::*;

impl<T: Node> Engine<T> {
    /// Applies a remove-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_remove_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<RemoveNodeEffect<T>, EngineEditError> {
        const OP: &str = "RemoveNode";

        if node == self.root {
            return Err(EngineEditError::CannotMutateRoot {
                edit_index,
                operation: OP,
                node,
            });
        }

        let (parent, prev_sibling, next_sibling) = self.node_position(edit_index, OP, node)?;
        let subtree = self.collect_subtree(edit_index, OP, node)?;
        let removed_ids = subtree.clone();
        self.run_destroy_for_subtree(subtree.as_slice());
        self.detach_node(edit_index, OP, node)?;

        let mut detached_nodes = Vec::with_capacity(subtree.len());
        for removed in subtree.into_iter().rev() {
            self.unregister_node_uuid(removed);
            let detached_node = self.nodes.detach(removed).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node: removed,
            })?;
            detached_nodes.push((removed, detached_node));
            self.purge_param_cache_entry(removed);
            self.blueprints.unregister_instance(removed);
            self.emit_inbox_event(EventKind::NodeDeleted { node: removed });
        }

        self.emit_inbox_event(EventKind::ChildRemoved { parent, child: node });
        // Project load discards UI graph transactions before the engine goes live,
        // so skip building removal ops (see the matching gate in apply_add_node).
        if !creation_context.is_some_and(NodeCreationContext::is_project_load) {
            self.push_ui_graph_transaction(vec![UiGraphOp::SubtreeRemoved {
                root: node,
                removed_ids,
                parent_after: self.ui_children_order_patch(parent),
            }]);
        }

        Ok(RemoveNodeEffect {
            node,
            parent,
            prev_sibling,
            next_sibling,
            detached_nodes,
        })
    }

    /// Computes which nodes in the subtree rooted at `root` need their `effective_enabled`
    /// updated based on the current parent chain and `meta.enabled` flags.
    ///
    /// Returns `(node_id, new_effective_enabled)` only for nodes whose cached value differs.
    pub(super) fn subtree_effective_enabled_changes(&self, root: NodeId) -> Vec<(NodeId, bool)> {
        let parent_effective = self
            .nodes
            .get(root)
            .and_then(|n| n.node_data().parent)
            .map(|p| {
                self.nodes
                    .get(p)
                    .map(|n| n.node_data().effective_enabled)
                    .unwrap_or(false)
            })
            .unwrap_or(true);

        let mut changes = Vec::new();
        let mut stack = vec![(root, parent_effective)];
        while let Some((node_id, parent_enabled)) = stack.pop() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let new_effective = parent_enabled && node.node_data().meta.enabled;
            if new_effective != node.node_data().effective_enabled {
                changes.push((node_id, new_effective));
            }
            let mut child = node.node_data().first_child;
            while let Some(child_id) = child {
                let next = self.nodes.get(child_id).and_then(|n| n.node_data().next_sibling);
                stack.push((child_id, new_effective));
                child = next;
            }
        }
        changes
    }
}
