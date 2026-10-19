use super::*;

impl<T: Node> Engine<T> {
    /// Applies a replace-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_replace_node(
        &mut self,
        edit_index: usize,
        node: NodeId,
        new_node: Box<dyn Node>,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AppliedEdit<ReplaceNodeEffect<T>>, EngineEditError> {
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
            replacement_data.effective_enabled = old_data.effective_enabled;
        }

        // Only the replaced payload retires; its retained children stay live.
        self.run_destroy_for_subtree(&[node])?;
        self.unregister_node_uuid(node);
        let old_node = self.nodes.detach(node).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation: OP,
            node,
        })?;
        self.purge_param_cache_entry(node);
        self.node_wakes.source.close(node);
        self.event_listeners.remove_subscriber(node);
        self.nodes.reattach(node, replacement);
        self.register_node_uuid(node);
        self.populate_param_cache_entry(node);
        self.mark_schedule_dirty();
        let old_blueprint = self.blueprints.unregister_instance(node);
        let enabled_changes = self.subtree_effective_enabled_changes(node);
        let completion = (|| {
            self.queue_effective_enabled_callbacks(&enabled_changes)?;
            self.run_node_attached_for_batch(&[node], creation_context)?;
            if let Some(context) = creation_context {
                self.run_node_init(node, creation_context)?;
                if context.is_project_load() {
                    self.queue_node_ready(node, context);
                } else {
                    self.run_node_ready(node, context)?;
                }
            }
            Ok(())
        })();

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

        Ok(AppliedEdit {
            effect: ReplaceNodeEffect {
                node,
                old_node,
                old_blueprint,
            },
            completion,
        })
    }
}
