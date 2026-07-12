use super::*;

impl<T: Node> Engine<T> {
    pub(crate) fn run_node_attached_for_batch(
        &mut self,
        node_ids: &[NodeId],
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        if node_ids.is_empty() {
            return Ok(());
        }

        let event_cursor = self.inbox.events.len();
        let tree_snapshot = self.build_process_tree_snapshot();
        for node_id in node_ids.iter().copied() {
            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.runtime_elapsed = self.runtime_elapsed;
            ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.engine_on_attached(&mut ctx);
                });
            }
            self.absorb_edits(&mut ctx)?;
        }
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, creation_context)?;
        }

        Ok(())
    }

    pub(crate) fn run_node_init_for_batch(
        &mut self,
        node_ids: &[NodeId],
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        if node_ids.is_empty() {
            return Ok(());
        }

        let event_cursor = self.inbox.events.len();
        let tree_snapshot = self.build_process_tree_snapshot();
        for node_id in node_ids.iter().copied() {
            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.runtime_elapsed = self.runtime_elapsed;
            ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.init(&mut ctx);
                });
            }
            self.absorb_edits(&mut ctx)?;
        }
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, creation_context)?;
        }

        Ok(())
    }

    pub(crate) fn run_node_ready_for_batch(
        &mut self,
        node_ids: &[NodeId],
        creation_context: NodeCreationContext,
    ) -> Result<(), EngineEditError> {
        if node_ids.is_empty() {
            return Ok(());
        }

        let mut event_cursor = self.inbox.events.len();
        let mut tree_snapshot = self.build_process_tree_snapshot();
        for node_id in node_ids.iter().copied() {
            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.runtime_elapsed = self.runtime_elapsed;
            ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.on_node_ready(&mut ctx, creation_context);
                });
            }
            self.absorb_edits(&mut ctx)?;
            if self.stabilization_scope_depth == 0 && self.pending_lifecycle_edits_include_structural() {
                self.stabilize_added_node_structure(event_cursor, Some(creation_context))?;
                event_cursor = self.inbox.events.len();
                tree_snapshot = self.build_process_tree_snapshot();
            }
        }
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, Some(creation_context))?;
        }

        Ok(())
    }

    /// Applies queued structural side effects and preprocesses newly emitted events
    /// until the add/bootstrap pipeline reaches a fixed point.
    pub(crate) fn stabilize_added_node_structure(
        &mut self,
        mut event_cursor: usize,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        self.stabilization_scope_depth = self.stabilization_scope_depth.saturating_add(1);
        let result = (|| -> Result<(), EngineEditError> {
            loop {
                while !self.edits.pending.is_empty() {
                    self.apply_edits_internal(false, creation_context)?;
                }

                let precomputed = self.precompute_inbox_dispatch_since(event_cursor);
                event_cursor = self.inbox.events.len();

                if precomputed.is_empty() {
                    if self.edits.pending.is_empty() {
                        break;
                    }
                    continue;
                }

                if creation_context.is_some() {
                    self.preprocess_precomputed_inbox(ExecutionPhase::EngineTick, precomputed)?;
                } else {
                    self.materialize_declared_precomputed_inbox(ExecutionPhase::EngineTick, precomputed)?;
                }
            }

            Ok(())
        })();
        self.stabilization_scope_depth = self.stabilization_scope_depth.saturating_sub(1);
        result
    }

    pub(crate) fn run_node_init(
        &mut self,
        node_id: NodeId,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        let needs_tree_snapshot = self
            .nodes
            .get(node_id)
            .is_some_and(|node| node.lifecycle_requires_tree_snapshot());
        let init_tree_snapshot = needs_tree_snapshot.then(|| self.build_process_tree_snapshot());
        let mut init_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        init_ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(init_tree_snapshot) = &init_tree_snapshot {
            init_ctx.set_tree_snapshot(Arc::clone(init_tree_snapshot));
        }
        if let Some(node) = self.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.init(&mut init_ctx);
            });
        }
        self.absorb_edits(&mut init_ctx)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(self.inbox.events.len(), creation_context)?;
        }

        Ok(())
    }

    pub(crate) fn run_node_ready(
        &mut self,
        node_id: NodeId,
        creation_context: NodeCreationContext,
    ) -> Result<(), EngineEditError> {
        let needs_tree_snapshot = self
            .nodes
            .get(node_id)
            .is_some_and(|node| node.lifecycle_requires_tree_snapshot());
        let created_tree_snapshot = needs_tree_snapshot.then(|| self.build_process_tree_snapshot());
        let mut created_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        created_ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(created_tree_snapshot) = &created_tree_snapshot {
            created_ctx.set_tree_snapshot(Arc::clone(created_tree_snapshot));
        }
        if let Some(node) = self.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.on_node_ready(&mut created_ctx, creation_context);
            });
        }
        self.absorb_edits(&mut created_ctx)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(self.inbox.events.len(), Some(creation_context))?;
        }

        Ok(())
    }

    pub(crate) fn run_node_destroy(
        &mut self,
        node_id: NodeId,
        tree_snapshot: Option<&Arc<crate::process_ctx::ProcessTreeSnapshot>>,
    ) {
        let mut destroy_ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        destroy_ctx.runtime_elapsed = self.runtime_elapsed;
        if let Some(tree_snapshot) = tree_snapshot {
            destroy_ctx.set_tree_snapshot(Arc::clone(tree_snapshot));
        }

        if let Some(node) = self.nodes.get_mut(node_id) {
            crate::logger::with_node_origin(node_id, || {
                node.destroy(&mut destroy_ctx);
            });
        }
    }

    pub(crate) fn run_destroy_for_subtree(&mut self, node_ids: &[NodeId]) {
        if node_ids.is_empty() {
            return;
        }

        // Same dynamic gate as the add/init path: only pay for a whole-tree snapshot
        // when some node in the removed subtree actually reads it during destroy.
        let needs_tree_snapshot = node_ids.iter().any(|node_id| {
            self.nodes
                .get(*node_id)
                .is_some_and(|node| node.lifecycle_requires_tree_snapshot())
        });
        let tree_snapshot = needs_tree_snapshot.then(|| self.build_process_tree_snapshot());
        for node_id in node_ids.iter().rev().copied() {
            self.run_node_destroy(node_id, tree_snapshot.as_ref());
        }
    }

    pub(crate) fn run_node_ready_for_subtree(
        &mut self,
        node_ids: &[NodeId],
        creation_context: NodeCreationContext,
    ) -> Result<(), EngineEditError> {
        let ready_ids = node_ids
            .iter()
            .copied()
            .filter(|node_id| self.nodes.contains(*node_id))
            .collect::<Vec<_>>();
        self.run_node_ready_for_batch(ready_ids.as_slice(), creation_context)
    }

    pub(crate) fn queue_node_ready(&mut self, node_id: NodeId, creation_context: NodeCreationContext) {
        self.pending_node_ready.push((node_id, creation_context));
    }

    pub(crate) fn run_pending_node_ready_callbacks(&mut self) -> Result<(), EngineEditError> {
        let pending = std::mem::take(&mut self.pending_node_ready);
        let mut current_context = None::<NodeCreationContext>;
        let mut ready_ids = Vec::<NodeId>::new();

        for (node_id, creation_context) in pending {
            if !self.nodes.contains(node_id) {
                continue;
            }

            if current_context.is_some_and(|context| context != creation_context) {
                let context = current_context.expect("current_context should exist for a non-empty batch");
                self.run_node_ready_for_batch(ready_ids.as_slice(), context)?;
                ready_ids.clear();
            }

            current_context = Some(creation_context);
            ready_ids.push(node_id);
        }

        if let Some(context) = current_context {
            self.run_node_ready_for_batch(ready_ids.as_slice(), context)?;
        }

        Ok(())
    }

    fn pending_lifecycle_edits_include_structural(&self) -> bool {
        self.edits
            .pending
            .iter()
            .any(|request| lifecycle_edit_is_structural(&request.edit))
    }
}

fn lifecycle_edit_is_structural(edit: &crate::edit::Edit) -> bool {
    matches!(
        edit,
        crate::edit::Edit::AddNode { .. }
            | crate::edit::Edit::AddNodeTree { .. }
            | crate::edit::Edit::AddUserItemTree { .. }
            | crate::edit::Edit::AddUserItem { .. }
            | crate::edit::Edit::CreateBlueprintInstance { .. }
            | crate::edit::Edit::ReplaceNode { .. }
            | crate::edit::Edit::RemoveNode { .. }
            | crate::edit::Edit::MoveNode { .. }
    )
}
