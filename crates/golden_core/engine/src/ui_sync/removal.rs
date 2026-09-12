use crate::{engine::EngineEditError, node::NodeId};

use super::*;

impl<T: Node> Engine<T> {
    /// Validates the whole selection before any edit, then retains only outermost
    /// roots in first-seen order. A selected parent already owns its descendants.
    pub(super) fn normalize_ui_remove_roots(&self, nodes: Vec<NodeId>) -> Result<Vec<NodeId>, EngineEditError> {
        const OP: &str = "RemoveNodes";

        let mut selected = HashSet::with_capacity(nodes.len());
        let mut unique = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.into_iter().enumerate() {
            if selected.insert(node) {
                self.node_position(index, OP, node)?;
                unique.push(node);
            }
        }

        let mut roots = Vec::with_capacity(unique.len());
        // Shared ancestor paths are visited once even for a deep all-selected chain.
        let mut selected_at_or_above = HashMap::<NodeId, bool>::with_capacity(unique.len());
        for node in unique {
            let mut ancestor = self.nodes.get(node).and_then(|entry| entry.node_data().parent);
            let mut has_selected_ancestor = false;
            let mut uncached_ancestors = Vec::new();
            let mut remaining_hops = self.nodes.len();
            while let Some(parent) = ancestor {
                if let Some(&cached) = selected_at_or_above.get(&parent) {
                    has_selected_ancestor = cached;
                    break;
                }
                if remaining_hops == 0 {
                    return Err(EngineEditError::NodeMutationRejected {
                        edit_index: 0,
                        operation: OP,
                        node,
                        node_type: self
                            .nodes
                            .get(node)
                            .expect("selected node was validated")
                            .get_type()
                            .to_owned(),
                        message: "parent ancestry contains a cycle".to_owned(),
                    });
                }
                remaining_hops -= 1;
                uncached_ancestors.push(parent);
                ancestor = self
                    .nodes
                    .get(parent)
                    .ok_or(EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: OP,
                        node: parent,
                    })?
                    .node_data()
                    .parent;
            }
            for ancestor in uncached_ancestors.into_iter().rev() {
                has_selected_ancestor |= selected.contains(&ancestor);
                selected_at_or_above.insert(ancestor, has_selected_ancestor);
            }
            if !has_selected_ancestor {
                roots.push(node);
            }
            selected_at_or_above.insert(node, true);
        }
        Ok(roots)
    }
}
