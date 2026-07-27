use super::*;

impl<T: Node> Engine<T> {
    /// Applies a replace-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_replace_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        new_node: Box<dyn Node>,
    ) -> Result<ReplaceNodeEffect<T>, EngineEditError> {
        const OP: &str = "ReplaceNode";

        let old_data = self
            .nodes
            .get(node)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?
            .node_data()
            .clone();

        let Some(parent) = old_data.parent else {
            return Err(EngineEditError::CannotMutateRoot {
                edit_index,
                operation: OP,
                node,
            });
        };

        let mut replacement = self.coerce_node_for_engine(edit_index, OP, new_node)?;
        {
            let replacement_data = replacement.node_data_mut();
            replacement_data.id = node;
            replacement_data.parent = old_data.parent;
            replacement_data.first_child = old_data.first_child;
            replacement_data.last_child = old_data.last_child;
            replacement_data.prev_sibling = old_data.prev_sibling;
            replacement_data.next_sibling = old_data.next_sibling;
            replacement_data.meta.decl_id = old_data.meta.decl_id.clone();
        }

        self.unregister_node_uuid(node);
        let old_node = self.nodes.detach(node).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation: OP,
            node,
        })?;
        self.purge_param_cache_entry(node);
        self.nodes.reattach(node, replacement);
        self.register_node_uuid(node);
        self.populate_param_cache_entry(node);
        self.mark_schedule_dirty();
        self.blueprints.unregister_instance(node);

        self.emit_event(EventKind::ChildReplaced {
            parent,
            old: node,
            new: node,
            decl_id: old_data.meta.decl_id.clone(),
        });

        if let Some(snapshot) = self.ui_node_dto_for_event(node) {
            let index = self.ui_child_index(parent, node);
            self.push_ui_graph_transaction(vec![UiGraphOp::NodeCreated {
                snapshot: Box::new(snapshot),
                parent: Some(parent),
                index,
            }]);
        }

        Ok(ReplaceNodeEffect {
            parent,
            old_id: node,
            new_id: node,
            prev_sibling: old_data.prev_sibling,
            next_sibling: old_data.next_sibling,
            old_node,
        })
    }
}
