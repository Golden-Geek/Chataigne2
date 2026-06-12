use super::*;

impl<T: Node> Engine<T> {
    pub(super) fn insert_pending_node_tree(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        mut tree: PendingNodeTree<T>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        user_role: UserNodeRole,
        inserted: &mut Vec<InsertedNode>,
    ) -> Result<NodeId, EngineEditError> {
        self.prepare_node_for_insert(&mut tree.node, parent, user_role);
        let node_id = self.nodes.insert(tree.node);
        self.attach_node(edit_index, operation, node_id, parent, prev_sibling)?;
        self.populate_param_cache_entry(node_id);
        let decl_id = self
            .nodes
            .get(node_id)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation,
                node: node_id,
            })?
            .node_data()
            .meta
            .decl_id
            .clone();
        inserted.push(InsertedNode {
            id: node_id,
            parent,
            decl_id,
        });

        let mut child_prev_sibling = None;
        for child in tree.children {
            let child_id = self.insert_pending_node_tree(
                edit_index,
                operation,
                child,
                node_id,
                child_prev_sibling,
                UserNodeRole::Regular,
                inserted,
            )?;
            child_prev_sibling = Some(child_id);
        }

        Ok(node_id)
    }

    fn apply_add_node_with_role(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: Box<dyn Node>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        user_role: UserNodeRole,
        validate_as_user_item: bool,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index,
                operation,
                parent,
            });
        }

        let mut node = self.coerce_node_for_engine(edit_index, operation, node)?;

        if validate_as_user_item {
            self.ensure_item_kind_allowed(edit_index, operation, parent, node.get_type(), node.user_item_kind())?;
        }

        self.prepare_node_for_insert(&mut node, parent, user_role);
        let child_id = self.nodes.insert(node);
        self.attach_node(edit_index, operation, child_id, parent, prev_sibling)?;

        // Initialize effective_enabled now that the parent link is established.
        // Uses the parent-chain walk (init path only — not a hot path).
        {
            let enabled = self.is_effectively_enabled(child_id);
            if let Some(n) = self.nodes.get_mut(child_id) {
                n.node_data_mut().effective_enabled = enabled;
            }
        }
        self.populate_param_cache_entry(child_id);

        let (attached_prev_sibling, attached_next_sibling) = {
            let attached_data = self
                .nodes
                .get(child_id)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation,
                    node: child_id,
                })?
                .node_data();
            (attached_data.prev_sibling, attached_data.next_sibling)
        };

        let decl_id = self
            .nodes
            .get(child_id)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation,
                node: child_id,
            })?
            .node_data()
            .meta
            .decl_id
            .clone();

        self.emit_inbox_event(EventKind::NodeCreated { node: child_id });
        self.emit_inbox_event(EventKind::ChildAdded {
            parent,
            child: child_id,
            decl_id,
        });
        let child_tree_snapshot = Some(self.build_process_tree_snapshot());

        // Allow newly attached nodes to request deterministic follow-up structure before app init.
        let mut attach_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        attach_ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(child_tree_snapshot) = &child_tree_snapshot {
            attach_ctx.set_tree_snapshot(Arc::clone(child_tree_snapshot));
        }
        if let Some(node) = self.nodes.get_mut(child_id) {
            crate::logger::with_node_origin(child_id, || {
                node.engine_on_attached(&mut attach_ctx);
            });
        }
        self.absorb_edits(&mut attach_ctx)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(self.inbox.events.len(), creation_context)?;
        }

        // Run app init after declared/generated children are materialized and handles are bound.
        self.run_node_init(child_id, creation_context)?;
        if let Some(context) = creation_context {
            self.run_node_ready(child_id, context)?;
        }

        self.push_added_subtree_ui_events(child_id, parent);

        Ok(AddNodeEffect {
            node: child_id,
            parent,
            prev_sibling: attached_prev_sibling,
            next_sibling: attached_next_sibling,
        })
    }

    /// Applies an add-node edit and returns history data required for undo/redo.
    pub(crate) fn apply_add_node(
        &mut self,
        edit_index: usize,
        node: Box<dyn Node>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        self.apply_add_node_with_role(
            edit_index,
            "AddNode",
            node,
            parent,
            prev_sibling,
            UserNodeRole::Regular,
            false,
            creation_context,
        )
    }

    /// Applies an add-node-tree edit and returns history data for the inserted root.
    pub(crate) fn apply_add_node_tree(
        &mut self,
        edit_index: usize,
        tree: NodeTree,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        const OP: &str = "AddNodeTree";

        if !self.nodes.contains(parent) {
            return Err(EngineEditError::ParentNotFound {
                edit_index,
                operation: OP,
                parent,
            });
        }

        let tree = self.coerce_pending_node_tree(edit_index, OP, tree)?;
        let mut inserted = Vec::new();
        let root_id = self.insert_pending_node_tree(
            edit_index,
            OP,
            tree,
            parent,
            prev_sibling,
            UserNodeRole::Regular,
            &mut inserted,
        )?;

        let (attached_prev_sibling, attached_next_sibling) = {
            let attached_data = self
                .nodes
                .get(root_id)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation: OP,
                    node: root_id,
                })?
                .node_data();
            (attached_data.prev_sibling, attached_data.next_sibling)
        };

        for inserted_node in &inserted {
            self.emit_inbox_event(EventKind::NodeCreated { node: inserted_node.id });
            self.emit_inbox_event(EventKind::ChildAdded {
                parent: inserted_node.parent,
                child: inserted_node.id,
                decl_id: inserted_node.decl_id.clone(),
            });
        }

        let inserted_ids = inserted.iter().map(|node| node.id).collect::<Vec<_>>();

        // Initialize effective_enabled before any node callbacks fire.
        for node_id in &inserted_ids {
            let enabled = self.is_effectively_enabled(*node_id);
            if let Some(node) = self.nodes.get_mut(*node_id) {
                node.node_data_mut().effective_enabled = enabled;
            }
        }

        self.run_node_attached_for_batch(inserted_ids.as_slice(), creation_context)?;
        self.run_node_init_for_batch(inserted_ids.as_slice(), creation_context)?;
        if let Some(context) = creation_context {
            self.run_node_ready_for_batch(inserted_ids.as_slice(), context)?;
        }

        self.push_added_subtree_ui_events(root_id, parent);

        Ok(AddNodeEffect {
            node: root_id,
            parent,
            prev_sibling: attached_prev_sibling,
            next_sibling: attached_next_sibling,
        })
    }

    /// Applies an add-user-item edit and returns history data required for undo/redo.
    pub(crate) fn apply_add_user_item(
        &mut self,
        edit_index: usize,
        node: Box<dyn Node>,
        parent: NodeId,
        prev_sibling: Option<NodeId>,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<AddNodeEffect, EngineEditError> {
        self.apply_add_node_with_role(
            edit_index,
            "AddUserItem",
            node,
            parent,
            prev_sibling,
            UserNodeRole::ItemRoot,
            true,
            creation_context,
        )
    }

    /// Emits a `GraphTransaction` covering all nodes in the subtree rooted at `root`
    /// so that the UI can insert them without a full resync.
    ///
    /// Must be called AFTER all lifecycle hooks (attached / init / ready) so that
    /// any declared children added during init are also included in the transaction.
    fn push_added_subtree_ui_events(&mut self, root: NodeId, root_parent: NodeId) {
        let node_ids = self.collect_subtree_node_ids(root);
        let parent_children_after = self.ui_direct_children(root_parent).unwrap_or_default();

        // Above this threshold, a single SubtreeInserted op replaces N NodeCreated + ChildrenReordered.
        // This avoids the O(N²) ui_child_index scan that the NodeCreated path needs for `index`.
        const SUBTREE_COMPACT_THRESHOLD: usize = 8;

        if node_ids.len() > SUBTREE_COMPACT_THRESHOLD {
            let mut nodes = Vec::with_capacity(node_ids.len());
            for node_id in &node_ids {
                if let Some(snapshot) = self.ui_node_dto_for_event(*node_id) {
                    nodes.push(snapshot);
                }
            }
            self.push_ui_graph_transaction(vec![UiGraphOp::SubtreeInserted {
                root,
                parent: root_parent,
                nodes,
                parent_children_after,
            }]);
        } else {
            // Small subtree: keep individual NodeCreated ops so the UI can resolve parent/index.
            let mut ops = Vec::with_capacity(node_ids.len() + 1);
            for node_id in &node_ids {
                let parent = self.nodes.get(*node_id).and_then(|n| n.node_data().parent);
                let Some(snapshot) = self.ui_node_dto_for_event(*node_id) else {
                    continue;
                };
                let index = parent.and_then(|p| self.ui_child_index(p, *node_id));
                ops.push(UiGraphOp::NodeCreated {
                    snapshot,
                    parent,
                    index,
                });
            }
            if !parent_children_after.is_empty() {
                ops.push(UiGraphOp::ChildrenReordered {
                    parent: root_parent,
                    children: parent_children_after,
                });
            }
            self.push_ui_graph_transaction(ops);
        }
    }
}
