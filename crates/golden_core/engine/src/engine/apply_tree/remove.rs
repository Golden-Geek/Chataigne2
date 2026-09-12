use super::*;
use crate::edit::{Edit, EditRequest};
use std::collections::{HashMap, HashSet};

impl<T: Node> Engine<T> {
    /// Collects disjoint removal roots in reverse request order so one destroy
    /// pass preserves the previous per-root callback ordering.
    pub(crate) fn independent_remove_batch_nodes(&self, requests: &[EditRequest]) -> Option<Vec<NodeId>> {
        if requests.len() < 2 {
            return None;
        }
        let mut roots = Vec::with_capacity(requests.len());
        for request in requests {
            let Edit::RemoveNode { node } = &request.edit else {
                return None;
            };
            let node = *node;
            let parent = self.node_position(0, "RemoveNode", node).ok()?.0;
            roots.push((node, parent));
        }
        if !self.removal_roots_are_independent(&roots) {
            return None;
        }

        let mut all_nodes = Vec::new();
        for (root, _) in roots.into_iter().rev() {
            all_nodes.extend(self.collect_subtree(0, "RemoveNode", root).ok()?);
        }
        Some(all_nodes)
    }

    /// Rejects nested roots while sharing ancestor checks across deep selections.
    /// Parents must remain attached so callbacks and final UI patches have a live owner.
    pub(crate) fn removal_roots_are_independent(&self, roots: &[(NodeId, NodeId)]) -> bool {
        let selected = roots.iter().map(|(root, _)| *root).collect::<HashSet<_>>();
        if selected.len() != roots.len() {
            return false;
        }
        let mut verified_ancestors = HashSet::new();
        for (_, root_parent) in roots {
            let mut ancestor = Some(*root_parent);
            let mut path = Vec::new();
            let mut remaining_hops = self.nodes.len();
            while let Some(parent) = ancestor {
                if selected.contains(&parent) || remaining_hops == 0 {
                    return false;
                }
                if verified_ancestors.contains(&parent) {
                    break;
                }
                remaining_hops -= 1;
                path.push(parent);
                let Some(node) = self.nodes.get(parent) else {
                    return false;
                };
                ancestor = node.node_data().parent;
            }
            verified_ancestors.extend(path);
        }
        true
    }

    /// Applies a remove-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_remove_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<RemoveNodeEffect<T>, EngineEditError> {
        self.apply_remove_node_inner(edit_index, node, creation_context, true, true)
    }

    pub(crate) fn apply_remove_node_after_batch_destroy(
        &mut self,
        edit_index: usize,
        node: NodeId,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<RemoveNodeEffect<T>, EngineEditError> {
        self.apply_remove_node_inner(edit_index, node, creation_context, false, false)
    }

    pub(crate) fn push_removed_subtrees_ui_batch(&mut self, removed: Vec<(NodeId, Vec<NodeId>, NodeId)>) {
        if removed.is_empty() {
            return;
        }
        let mut ops = Vec::with_capacity(removed.len());
        let last_index_by_parent = removed
            .iter()
            .enumerate()
            .map(|(index, (_, _, parent))| (*parent, index))
            .collect::<HashMap<_, _>>();
        for (index, (root, removed_ids, parent)) in removed.into_iter().enumerate() {
            ops.push(UiGraphOp::SubtreeRemoved {
                root,
                removed_ids,
                parent_after: (last_index_by_parent[&parent] == index)
                    .then(|| self.ui_children_order_patch(parent))
                    .flatten(),
            });
        }
        self.push_ui_graph_transaction(ops);
    }

    fn apply_remove_node_inner(
        &mut self,
        edit_index: usize,
        node: NodeId,
        creation_context: Option<NodeCreationContext>,
        run_destroy: bool,
        emit_ui: bool,
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
        if run_destroy {
            self.run_destroy_for_subtree(subtree.as_slice());
        }
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
        if emit_ui && !creation_context.is_some_and(NodeCreationContext::is_project_load) {
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
    pub(crate) fn subtree_effective_enabled_changes(&self, root: NodeId) -> Vec<(NodeId, bool)> {
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
