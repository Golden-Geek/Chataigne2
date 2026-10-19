use super::*;

impl<T: Node> Engine<T> {
    pub(super) fn replay_loaded_subtree_lifecycle(
        &mut self,
        root: NodeId,
        creation_context: NodeCreationContext,
        ready_mode: LoadedReadyMode,
    ) -> Result<(), ProjectPersistenceError> {
        self.replay_loaded_subtrees_lifecycle(&[root], creation_context, ready_mode)
    }

    pub(super) fn replay_loaded_subtrees_lifecycle(
        &mut self,
        roots: &[NodeId],
        creation_context: NodeCreationContext,
        ready_mode: LoadedReadyMode,
    ) -> Result<(), ProjectPersistenceError> {
        self.replay_loaded_subtrees_lifecycle_with_initial_params(roots, creation_context, ready_mode, &[])
    }

    pub(super) fn replay_loaded_subtrees_lifecycle_with_initial_params(
        &mut self,
        roots: &[NodeId],
        creation_context: NodeCreationContext,
        ready_mode: LoadedReadyMode,
        initial_params: &[(NodeId, &[(DeclId, crate::parameter::ParamValue)])],
    ) -> Result<(), ProjectPersistenceError> {
        let collect_nodes = |engine: &Self| -> Result<Vec<NodeId>, ProjectPersistenceError> {
            let mut node_ids = Vec::new();
            for root in roots {
                node_ids.extend(engine.collect_loaded_subtree_node_ids(*root)?);
            }
            Ok(node_ids)
        };
        let mut loaded_node_ids = collect_nodes(self)?;

        self.prune_loaded_duplicate_declared_children(loaded_node_ids.as_slice())?;
        loaded_node_ids = collect_nodes(self)?;

        let snapshot_consumers_can_reuse = matches!(ready_mode, LoadedReadyMode::Immediate)
            && loaded_node_ids
                .iter()
                .filter_map(|node_id| self.nodes.get(*node_id))
                .filter(|node| node.attached_requires_tree_snapshot() || node.ready_requires_tree_snapshot())
                .all(Node::attached_snapshot_reusable_for_ready);
        let shared_snapshot = snapshot_consumers_can_reuse
            .then(|| {
                self.batch_lifecycle_tree_snapshot("attached-ready-batch", loaded_node_ids.as_slice(), |node| {
                    node.attached_requires_tree_snapshot() || node.ready_requires_tree_snapshot()
                })
            })
            .flatten();
        let snapshot_time = self.time;
        let snapshot_node_count = self.nodes.len();
        let snapshot_graph_version = self.ui_graph_version;
        let snapshot_callback_ids = shared_snapshot.as_ref().map(|_| loaded_node_ids.clone());

        self.run_node_attached_for_batch_with_snapshot(
            loaded_node_ids.as_slice(),
            Some(creation_context),
            shared_snapshot.clone(),
        )?;
        self.reconcile_loaded_declared_children(loaded_node_ids.as_slice(), creation_context)?;
        loaded_node_ids = collect_nodes(self)?;
        self.prune_loaded_duplicate_declared_children(loaded_node_ids.as_slice())?;
        loaded_node_ids = collect_nodes(self)?;
        self.reconcile_loaded_declared_children(loaded_node_ids.as_slice(), creation_context)?;
        loaded_node_ids = collect_nodes(self)?;
        self.run_node_init_for_batch(loaded_node_ids.as_slice(), Some(creation_context))?;

        // Init may write domain defaults. Explicit caller values win before
        // ready callbacks configure resources from the authoritative parameters.
        let has_initial_params = initial_params.iter().any(|(_, params)| !params.is_empty());
        if has_initial_params {
            for (root, params) in initial_params {
                for (decl_id, value) in *params {
                    let event_cursor = self.inbox.events.len();
                    let param = self.find_staged_parameter(*root, &decl_id.0).ok_or_else(|| {
                        EngineEditError::NodeMutationRejected {
                            edit_index: 0,
                            operation: "InitializePreparedItem",
                            node: *root,
                            node_type: self
                                .nodes
                                .get(*root)
                                .map(|node| node.get_type().to_string())
                                .unwrap_or_default(),
                            message: format!("prepared parameter '{}' disappeared during init", decl_id.0),
                        }
                    })?;
                    self.apply_set_param(0, param, value.clone())?;
                    self.stabilize_added_node_structure(event_cursor, None)?;
                }
            }
            loaded_node_ids = collect_nodes(self)?;
            let values = &self.parameter_values_cache;
            for node_id in &loaded_node_ids {
                if let Some(node) = self.nodes.get_mut(*node_id) {
                    node.engine_sync_bound_param_handles(&mut |param| values.get(&param).cloned());
                }
            }
        }

        let ready_snapshot = shared_snapshot.filter(|_| {
            !has_initial_params
                && self.time == snapshot_time
                && self.nodes.len() == snapshot_node_count
                && self.ui_graph_version == snapshot_graph_version
                && snapshot_callback_ids.as_ref() == Some(&loaded_node_ids)
        });
        match ready_mode {
            LoadedReadyMode::Immediate => self.run_node_ready_for_batch_with_snapshot(
                loaded_node_ids.as_slice(),
                creation_context,
                ready_snapshot,
            )?,
            LoadedReadyMode::Deferred => {
                for node_id in loaded_node_ids {
                    self.queue_node_ready(node_id, creation_context);
                }
            }
        }

        Ok(())
    }

    pub(super) fn prune_loaded_duplicate_declared_children(
        &mut self,
        parent_ids: &[NodeId],
    ) -> Result<(), ProjectPersistenceError> {
        let mut duplicates = Vec::<NodeId>::new();

        for parent_id in parent_ids {
            if !self.nodes.contains(*parent_id) {
                continue;
            }

            let parent_first_child = {
                let parent_node = self
                    .nodes
                    .get(*parent_id)
                    .ok_or(ProjectPersistenceError::MissingNode(*parent_id))?;
                if user_context_multiplex_list_value_type(parent_node.get_type()).is_some() {
                    continue;
                }
                parent_node.node_data().first_child
            };

            let mut seen = HashMap::<String, NodeId>::new();
            let mut child = parent_first_child;

            while let Some(child_id) = child {
                let child_node = self
                    .nodes
                    .get(child_id)
                    .ok_or(ProjectPersistenceError::MissingNode(child_id))?;
                child = child_node.node_data().next_sibling;

                let child_data = child_node.node_data();
                if child_data.user_role != UserNodeRole::Regular {
                    continue;
                }

                let decl_id = child_data.meta.decl_id.0.trim();
                if decl_id.is_empty() {
                    continue;
                }

                let key = format!("{}|{:?}|{}", child_node.get_type(), child_data.user_role, decl_id);
                if seen.insert(key, child_id).is_some() {
                    duplicates.push(child_id);
                }
            }
        }

        for duplicate in duplicates {
            if self.nodes.contains(duplicate) {
                self.discard_loaded_duplicate_declared_subtree(duplicate)?;
            }
        }

        Ok(())
    }

    fn discard_loaded_duplicate_declared_subtree(&mut self, root: NodeId) -> Result<(), ProjectPersistenceError> {
        const OP: &str = "LoadProjectPruneDuplicateDeclaredChild";

        let subtree = self
            .collect_subtree(0, OP, root)
            .map_err(ProjectPersistenceError::Engine)?;
        self.detach_node(0, OP, root).map_err(ProjectPersistenceError::Engine)?;

        for removed in subtree.into_iter().rev() {
            self.unregister_node_uuid(removed);
            self.nodes
                .detach(removed)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index: 0,
                    operation: OP,
                    node: removed,
                })
                .map_err(ProjectPersistenceError::Engine)?;
            self.purge_param_cache_entry(removed);
            self.blueprints.unregister_instance(removed);
        }

        Ok(())
    }

    pub(crate) fn queue_loaded_subtree_structure_events(
        &mut self,
        node_ids: &[NodeId],
    ) -> Result<(), ProjectPersistenceError> {
        for node_id in node_ids.iter().copied() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            let Some(parent) = node.node_data().parent else {
                continue;
            };
            let decl_id = node.node_data().meta.decl_id.clone();

            self.emit_inbox_event(EventKind::NodeCreated { node: node_id });
            self.emit_inbox_event(EventKind::ChildAdded {
                parent,
                child: node_id,
                decl_id,
            });
        }

        Ok(())
    }

    pub(super) fn reconcile_loaded_declared_children(
        &mut self,
        node_ids: &[NodeId],
        creation_context: NodeCreationContext,
    ) -> Result<(), ProjectPersistenceError> {
        let mut index_by_node = HashMap::<NodeId, usize>::new();
        let mut per_node_events = Vec::<(NodeId, EventFrame)>::new();

        for parent_id in node_ids {
            let mut child = self
                .nodes
                .get(*parent_id)
                .ok_or(ProjectPersistenceError::MissingNode(*parent_id))?
                .node_data()
                .first_child;

            while let Some(child_id) = child {
                let child_node = self
                    .nodes
                    .get(child_id)
                    .ok_or(ProjectPersistenceError::MissingNode(child_id))?;
                let event = Event {
                    time: self.time,
                    kind: EventKind::ChildAdded {
                        parent: *parent_id,
                        child: child_id,
                        decl_id: child_node.node_data().meta.decl_id.clone(),
                    },
                };

                let event = Arc::new(event);
                for recipient in self.route_event_recipients(event.as_ref()) {
                    let slot = match index_by_node.get(&recipient).copied() {
                        Some(index) => index,
                        None => {
                            let index = per_node_events.len();
                            per_node_events.push((recipient, EventFrame::new()));
                            index_by_node.insert(recipient, index);
                            index
                        }
                    };
                    per_node_events[slot].1.push_shared(Arc::clone(&event));
                }

                child = child_node.node_data().next_sibling;
            }
        }

        if per_node_events.is_empty() {
            return Ok(());
        }

        let event_cursor = self.inbox.events.len();
        self.preprocess_precomputed_inbox(ExecutionPhase::EngineTick, per_node_events)?;
        if self.stabilization_scope_depth == 0 {
            self.stabilize_added_node_structure(event_cursor, Some(creation_context))?;
        }

        Ok(())
    }
}
