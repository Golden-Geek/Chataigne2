use crate::contexts::UserContextValueType;
use crate::edit::Edit;
use crate::events::{CustomEventRetention, Event, EventKind};
use crate::node::{Node, NodeCreationContext};
use crate::parameter::ParamValue;
use std::{any::type_name, collections::BTreeMap};

use super::edit_journal::JournalMode;
use super::history::HistoryStep;
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Applies all pending edits in queue order and emits resulting events.
    ///
    /// On failure, committed edits remain applied and undoable, including structural
    /// edits or mutable callbacks whose completion failed. Requests rejected before
    /// mutation and the unprocessed suffix are discarded; this is not an atomic intent.
    ///
    /// # Examples
    /// ```rust
    /// use golden_engine::engine::Engine;
    /// use golden_engine::node::Folder;
    ///
    /// let mut engine = Engine::new(Folder::new("root".to_string()));
    /// engine.add_node(Folder::new("child".to_string()), None);
    /// engine.apply_edits().expect("edit application should succeed");
    /// ```
    pub fn apply_edits(&mut self) -> Result<(), EngineEditError> {
        self.apply_edits_internal(true, Some(NodeCreationContext::Fresh))
    }

    /// Applies app-owned project-load augmentation before the engine becomes live.
    ///
    /// Loaded-node lifecycle callbacks run with project-load semantics. Undo history
    /// and per-edit UI projections are omitted because the host publishes one
    /// complete snapshot after cutover.
    pub fn apply_project_load_edits(&mut self) -> Result<(), EngineEditError> {
        self.apply_edits_internal(false, Some(NodeCreationContext::ProjectLoadAugmentation))
    }

    /// Attaches multiple app-owned trees as one pre-cutover lifecycle batch.
    ///
    /// This is the scalable project-load path for data-driven assets that share a
    /// parent. Pending edits are settled first, the trees retain input order, and
    /// the host is expected to publish one complete snapshot after cutover.
    pub fn apply_project_load_node_trees(
        &mut self,
        trees: Vec<crate::edit::NodeTree>,
        parent: crate::node::NodeId,
        prev_sibling: Option<crate::node::NodeId>,
    ) -> Result<(), EngineEditError> {
        self.apply_project_load_edits()?;
        if trees.is_empty() {
            return Ok(());
        }
        self.apply_add_node_trees_for_project_load(trees, parent, prev_sibling)?;
        self.sync_missing_reference_warnings_silent();
        self.rebuild_user_context_registry_from_nodes();
        self.mark_user_context_graph_changed();
        Ok(())
    }

    /// Applies pending edits without recording undo transactions.
    ///
    /// Runtime-driven edit flushes use this so periodic/internal graph activity
    /// does not pollute user-facing undo history.
    pub(crate) fn apply_edits_without_history(&mut self) -> Result<(), EngineEditError> {
        if *super::runtime::PERF_TRACE_ENABLED && !self.edits.pending.is_empty() {
            eprintln!(
                "[engine] runtime_edit_flush count={} kinds={}",
                self.edits.pending.len(),
                pending_edit_kind_summary(&self.edits.pending)
            );
        }
        self.tick_scratch.stats.edits_applied += self.edits.pending.len();
        self.apply_edits_internal(false, Some(NodeCreationContext::Fresh))
    }

    /// Applies pending edits without runtime creation callbacks for newly added nodes.
    ///
    /// Macro-generated declared-child structure is still materialized recursively so
    /// persistence baselines observe the same declaration defaults as the live tree.
    pub(crate) fn apply_edits_without_creation_callbacks(&mut self) -> Result<(), EngineEditError> {
        self.apply_edits_internal(false, None)
    }

    pub(crate) fn apply_edits_internal(
        &mut self,
        capture_history: bool,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        let owns_journal = capture_history && self.edit_journal.is_none();
        if owns_journal {
            self.begin_edit_journal(JournalMode::AppliedPrefix);
        }
        let result = self.apply_edits_drain(capture_history, creation_context);
        if owns_journal {
            // Ordinary drains keep their committed prefix, even when completion fails.
            self.commit_edit_journal();
        }
        result
    }

    fn apply_edits_drain(
        &mut self,
        capture_history: bool,
        creation_context: Option<NodeCreationContext>,
    ) -> Result<(), EngineEditError> {
        if let Err(error) = self.charge_lifecycle_work(self.edits.pending.len()) {
            self.edits.pending.clear();
            return Err(error);
        }
        if !self.edits.pending.is_empty() {
            self.clear_prepared_first_tick_snapshot();
        }

        let mut missing_reference_warning_dirty = false;
        let mut user_context_graph_dirty = false;

        let requests = self.edits.drain();
        let remove_batch_nodes = self.independent_remove_batch_nodes(&requests);
        let batch_destroyed = remove_batch_nodes.is_some();
        let emit_remove_batch_ui =
            batch_destroyed && !creation_context.is_some_and(NodeCreationContext::is_project_load);
        let mut removed_batch_ui = Vec::new();
        if let Some(node_ids) = remove_batch_nodes {
            self.run_destroy_for_subtree(&node_ids)?;
        }

        let mut result = Ok(());
        for (edit_index, request) in requests.into_iter().enumerate() {
            let journal_order = self.edit_order();
            let project_dirty_node = opaque_project_dirty_node(&request.edit);
            let mut completion = Ok(());
            // Keep recoverable exits inside the request boundary so the accepted
            // prefix always reaches history, reference, context, and UI finalization.
            let outcome = (|| -> Result<_, EngineEditError> {
                if (self.has_atomic_journal() || (self.edit_journal.is_some() && !capture_history))
                    && matches!(
                        &request.edit,
                        Edit::BeginEditSession { .. } | Edit::EndEditSession { .. }
                    )
                {
                    return Err(EngineEditError::NodeMutationRejected {
                        edit_index,
                        operation: "CallbackEditSession",
                        node: self.root,
                        node_type: self
                            .nodes
                            .get(self.root)
                            .map(|node| node.get_type().to_string())
                            .unwrap_or_default(),
                        message: "callbacks cannot change the enclosing edit session".into(),
                    });
                }
                let (outcome, should_clear_redo): (Result<Option<HistoryStep<T>>, EngineEditError>, bool) =
                    match request.edit {
                        Edit::CheckPreconditions { preconditions } => {
                            let nodes_match = preconditions.nodes.iter().all(|guard| {
                                self.nodes.get(guard.node).is_some_and(|node| {
                                    node.node_data().meta.uuid == guard.uuid
                                        && node.node_data().effective_enabled == guard.enabled
                                        && if guard.authored {
                                            self.tree_revisions.unchanged_since(guard.node, guard.revision)
                                        } else {
                                            self.tree_revisions.same_epoch(guard.revision)
                                        }
                                })
                            });
                            if !nodes_match || !preconditions.native.iter().all(|guard| guard.is_current()) {
                                return Err(EngineEditError::EditPreconditionFailed { edit_index });
                            }
                            (Ok(None), false)
                        }
                        Edit::BeginEditSession {
                            origin,
                            label,
                            client_edit_id,
                            ui_client_instance_id,
                        } => {
                            if self.edit_journal.is_some() {
                                self.flush_edit_journal();
                            }
                            if let Some(active) = &self.active_edit_session {
                                (
                                    Err(EngineEditError::EditSessionAlreadyActive {
                                        edit_index,
                                        requested_client_edit_id: client_edit_id,
                                        active_client_edit_id: active.client_edit_id.clone(),
                                    }),
                                    false,
                                )
                            } else {
                                self.active_edit_session = Some(super::history::ActiveEditSession::new(
                                    origin,
                                    label,
                                    client_edit_id,
                                    ui_client_instance_id,
                                ));
                                (Ok(None), false)
                            }
                        }
                        Edit::EndEditSession { client_edit_id } => {
                            if self.edit_journal.is_some() {
                                self.flush_edit_journal();
                            }
                            if let Some(active) = self.active_edit_session.take() {
                                if active.client_edit_id != client_edit_id {
                                    let active_client_edit_id = active.client_edit_id.clone();
                                    self.active_edit_session = Some(active);
                                    (
                                        Err(EngineEditError::EditSessionIdMismatch {
                                            edit_index,
                                            requested_client_edit_id: client_edit_id,
                                            active_client_edit_id,
                                        }),
                                        false,
                                    )
                                } else {
                                    if !active.transaction.is_empty() {
                                        self.push_undo_transaction(active.transaction);
                                    }
                                    (Ok(None), false)
                                }
                            } else {
                                (
                                    Err(EngineEditError::EditSessionNotActive {
                                        edit_index,
                                        requested_client_edit_id: client_edit_id,
                                    }),
                                    false,
                                )
                            }
                        }
                        Edit::SetParam { node, value, behaviour } => {
                            match self.apply_set_param(edit_index, node, value, request.param_source)? {
                                Some(mut effect) => {
                                    if matches!(effect.old_value, ParamValue::Reference(_))
                                        || matches!(effect.new_value, ParamValue::Reference(_))
                                    {
                                        missing_reference_warning_dirty = true;
                                    }
                                    if UserContextValueType::from_param_value(&effect.old_value)
                                        != UserContextValueType::from_param_value(&effect.new_value)
                                        && self.node_within_user_context_scope(effect.node)
                                    {
                                        user_context_graph_dirty = true;
                                    }
                                    if self.queue_user_context_multiplex_resize_for_count(effect.node) {
                                        user_context_graph_dirty = true;
                                    }
                                    effect.behaviour = behaviour;
                                    effect.tick = self.time.tick;
                                    (Ok(Some(effect.into())), true)
                                }
                                None => (Ok(None), false),
                            }
                        }
                        Edit::SetParamControlState { node, state } => {
                            let effect = self
                                .apply_set_param_control_state(edit_index, node, state)?
                                .capture_completion(&mut completion);
                            let changed = effect.is_some();
                            (Ok(effect.map(Into::into)), changed)
                        }
                        Edit::SetParamConstraints { node, constraints } => {
                            missing_reference_warning_dirty = true;
                            let effect =
                                self.apply_set_param_constraints(edit_index, node, constraints, request.param_source)?;
                            let should_clear_redo = effect.is_some();
                            (Ok(effect.map(Into::into)), should_clear_redo)
                        }
                        Edit::SetParamEnumOptions {
                            node,
                            expected_uuid,
                            expected_value,
                            options,
                        } => {
                            missing_reference_warning_dirty = true;
                            let effect = self.apply_set_param_enum_options(
                                edit_index,
                                node,
                                expected_uuid,
                                expected_value,
                                options,
                            )?;
                            let changed = effect.is_some();
                            (Ok(effect.map(Into::into)), changed)
                        }
                        Edit::SetNodeScriptProperty { node, write } => {
                            let capture = self.capture_opaque_history(capture_history, node);
                            let effect = self
                                .apply_set_node_script_property(edit_index, node, write, capture)?
                                .capture_completion(&mut completion);
                            missing_reference_warning_dirty |= effect.is_some();
                            user_context_graph_dirty |= effect.is_some();
                            let changed = effect.is_some() || !capture;
                            (Ok(effect.map(Into::into)), changed)
                        }
                        Edit::CallNodeScriptMethod { node, call } => {
                            let capture = self.capture_opaque_history(capture_history, node);
                            let effect = self
                                .apply_call_node_script_method(edit_index, node, call, capture)?
                                .capture_completion(&mut completion);
                            missing_reference_warning_dirty |= effect.is_some();
                            user_context_graph_dirty |= effect.is_some();
                            let changed = effect.is_some() || !capture;
                            (Ok(effect.map(Into::into)), changed)
                        }
                        Edit::CallNodeMutation {
                            node,
                            callback,
                            needs_tree_snapshot,
                        } => {
                            let capture = self.capture_opaque_history(capture_history, node);
                            let effect = self
                                .apply_call_node_mutation(
                                    edit_index,
                                    node,
                                    callback,
                                    needs_tree_snapshot,
                                    capture,
                                    request.param_source,
                                )?
                                .capture_completion(&mut completion);
                            missing_reference_warning_dirty |= effect.is_some();
                            user_context_graph_dirty |= effect.is_some();
                            let changed = effect.is_some() || !capture;
                            (Ok(effect.map(Into::into)), changed)
                        }
                        Edit::AddNode {
                            node,
                            parent,
                            prev_sibling,
                        } => {
                            missing_reference_warning_dirty = true;
                            user_context_graph_dirty = true;
                            let effect = self
                                .apply_add_node(edit_index, node, parent, prev_sibling, creation_context)?
                                .capture_completion(&mut completion);
                            if self.queue_user_context_multiplex_resize_for_list(effect.node) {
                                user_context_graph_dirty = true;
                            }
                            (Ok(Some(effect.into())), true)
                        }
                        Edit::AddNodeTree {
                            tree,
                            parent,
                            prev_sibling,
                        } => {
                            missing_reference_warning_dirty = true;
                            user_context_graph_dirty = true;
                            let effect = self
                                .apply_add_node_tree(edit_index, tree, parent, prev_sibling, creation_context)?
                                .capture_completion(&mut completion);
                            if self.queue_user_context_multiplex_resize_for_list(effect.node) {
                                user_context_graph_dirty = true;
                            }
                            (Ok(Some(effect.into())), true)
                        }
                        Edit::AddUserItemTree {
                            tree,
                            parent,
                            prev_sibling,
                        } => {
                            missing_reference_warning_dirty = true;
                            user_context_graph_dirty = true;
                            let effect = self
                                .apply_add_user_item_tree(edit_index, tree, parent, prev_sibling, creation_context)?
                                .capture_completion(&mut completion);
                            if self.queue_user_context_multiplex_resize_for_list(effect.node) {
                                user_context_graph_dirty = true;
                            }
                            (Ok(Some(effect.into())), true)
                        }
                        Edit::AddUserItemTrees { items } => {
                            let effects = self
                                .apply_add_user_item_trees(edit_index, items, creation_context)?
                                .capture_completion(&mut completion);
                            let changed = !effects.is_empty();
                            if changed {
                                missing_reference_warning_dirty = true;
                                user_context_graph_dirty = true;
                            }
                            for effect in effects {
                                if self.queue_user_context_multiplex_resize_for_list(effect.node) {
                                    user_context_graph_dirty = true;
                                }
                                if let Some(order) = journal_order {
                                    self.capture_journal_step(order, effect.into());
                                }
                            }
                            (Ok(None), changed)
                        }
                        Edit::AddUserItem {
                            node,
                            parent,
                            prev_sibling,
                        } => {
                            missing_reference_warning_dirty = true;
                            user_context_graph_dirty = true;
                            let effect = self
                                .apply_add_user_item(edit_index, node, parent, prev_sibling, creation_context)?
                                .capture_completion(&mut completion);
                            if self.queue_user_context_multiplex_resize_for_list(effect.node) {
                                user_context_graph_dirty = true;
                            }
                            (Ok(Some(effect.into())), true)
                        }
                        Edit::CreateBlueprintInstance {
                            blueprint_id,
                            parent,
                            prev_sibling,
                            label,
                        } => {
                            missing_reference_warning_dirty = true;
                            user_context_graph_dirty = true;
                            let effect = self
                                .apply_create_blueprint_instance(
                                    edit_index,
                                    blueprint_id,
                                    parent,
                                    prev_sibling,
                                    label,
                                    creation_context,
                                )?
                                .capture_completion(&mut completion);
                            (Ok(Some(effect.into())), true)
                        }
                        Edit::ReplaceNode { node, new_node } => {
                            missing_reference_warning_dirty = true;
                            user_context_graph_dirty = true;
                            let effect = self
                                .apply_replace_node(edit_index, node, new_node, creation_context)?
                                .capture_completion(&mut completion);
                            (Ok(Some(effect.into())), true)
                        }
                        Edit::RemoveNode { node } => {
                            missing_reference_warning_dirty = true;
                            user_context_graph_dirty = true;
                            let result = if batch_destroyed {
                                self.apply_remove_node_after_batch_destroy(edit_index, node, creation_context)
                            } else {
                                self.apply_remove_node(edit_index, node, creation_context)
                            };
                            match result {
                                Ok(effect) => {
                                    if emit_remove_batch_ui {
                                        removed_batch_ui.push((
                                            effect.node,
                                            effect.detached_nodes.iter().rev().map(|(id, _)| *id).collect(),
                                            effect.parent,
                                        ));
                                    }
                                    (Ok(Some(effect.into())), true)
                                }
                                Err(err) => (Err(err), false),
                            }
                        }
                        Edit::MoveNode {
                            node,
                            new_parent,
                            new_prev_sibling,
                        } => {
                            missing_reference_warning_dirty = true;
                            user_context_graph_dirty = true;
                            let effect = self
                                .apply_move_node(edit_index, node, new_parent, new_prev_sibling)?
                                .capture_completion(&mut completion);
                            (Ok(Some(effect.into())), true)
                        }
                        Edit::PatchMeta { node, patch } => {
                            missing_reference_warning_dirty = true;
                            let effect = self
                                .apply_patch_meta_with_completion(edit_index, node, patch)?
                                .capture_completion(&mut completion);
                            (Ok(Some(effect.into())), true)
                        }
                        Edit::SetScriptConfig {
                            node,
                            config,
                            force_reload,
                        } => {
                            let effect = self.apply_set_script_config(edit_index, node, config, force_reload)?;
                            let should_clear_redo = effect.is_some();
                            (Ok(effect.map(Into::into)), should_clear_redo)
                        }
                        Edit::SetNodeWarning { node, warning } => {
                            let effect = self.apply_set_node_warning(edit_index, node, warning)?;
                            let should_clear_redo = effect.is_some();
                            (Ok(effect.map(Into::into)), should_clear_redo)
                        }
                        Edit::ClearNodeWarning { node, warning_id } => {
                            let effect = self.apply_clear_node_warning(edit_index, node, warning_id)?;
                            let should_clear_redo = effect.is_some();
                            (Ok(effect.map(Into::into)), should_clear_redo)
                        }
                        Edit::SetNodeChildWarningDepth { node, max_depth } => {
                            let effect = self.apply_set_node_child_warning_depth(edit_index, node, max_depth)?;
                            let should_clear_redo = effect.is_some();
                            (Ok(effect.map(Into::into)), should_clear_redo)
                        }
                        Edit::EmitCustomEvent { event } => {
                            if event.effective_retention() == CustomEventRetention::Transient {
                                self.emit_inbox_event(EventKind::Custom(event));
                                (Ok(None), false)
                            } else {
                                self.emit_event(EventKind::Custom(event));
                                (Ok(None), true)
                            }
                        }
                        Edit::ReevaluateGraph => {
                            self.mark_schedule_dirty();
                            (Ok(None), true)
                        }
                        Edit::AddEventListener {
                            subscriber,
                            subscription,
                        } => {
                            self.apply_add_event_listener(edit_index, subscriber, subscription)?;
                            (Ok(None), true)
                        }
                        Edit::RemoveEventListener {
                            subscriber,
                            subscription,
                        } => {
                            self.apply_remove_event_listener(subscriber, subscription);
                            (Ok(None), true)
                        }
                    };

                outcome.map(|step| (step, should_clear_redo))
            })();

            match outcome {
                Ok((step, should_clear_redo)) => {
                    if should_clear_redo && let Some(node) = project_dirty_node {
                        self.project_dirty_nodes.insert(node);
                    }
                    if let Some(step) = step {
                        if let Some(order) = journal_order {
                            self.capture_journal_step(order, step);
                        } else {
                            let mut step = step;
                            step.dispose_detached(self);
                        }
                    }
                }
                Err(err) => {
                    result = Err(err);
                    break;
                }
            }
            if completion.is_err() {
                result = completion;
                break;
            }
            self.complete_edit_requests(request.completions);
        }

        if emit_remove_batch_ui {
            self.push_removed_subtrees_ui_batch(removed_batch_ui);
        }

        if missing_reference_warning_dirty {
            if creation_context.is_some_and(NodeCreationContext::is_project_load) {
                self.sync_missing_reference_warnings_silent();
            } else {
                self.sync_missing_reference_warnings();
            }
        }
        if user_context_graph_dirty {
            self.rebuild_user_context_registry_from_nodes();
            self.mark_user_context_graph_changed();
        }

        result
    }

    /// Validates and downcasts a dynamically provided node to the engine node type `T`.
    ///
    /// Returns [`EngineEditError::NodeTypeMismatch`] when the node cannot be coerced.
    pub(crate) fn coerce_node_for_engine(
        &self,
        edit_index: usize,
        operation: &'static str,
        node: Box<dyn Node>,
    ) -> Result<T, EngineEditError> {
        let provided_node_type = node.get_type().to_string();
        T::from_boxed_node(node).ok_or(EngineEditError::NodeTypeMismatch {
            edit_index,
            operation,
            provided_node_type,
            expected_engine_node_type: type_name::<T>(),
        })
    }

    fn apply_event_side_effects(&mut self, kind: &EventKind) {
        let sequence = self.tree_revisions.next_change();
        let nodes = &self.nodes;
        let revisions = &mut self.tree_revisions;
        let dirty = &mut self.project_dirty_nodes;
        let mut changed = false;
        super::change_targets::visit_changed_nodes(kind, &mut |node| {
            dirty.insert(node);
            changed = true;
            if nodes.contains(node) {
                revisions.mark(node, sequence);
            }
        });
        if changed {
            self.clear_tick_tree_snapshot();
        }
        match &kind {
            EventKind::NodeCreated { node } => {
                self.node_wakes.source.attach([*node]);
                self.last_update_elapsed_by_node
                    .entry(*node)
                    .or_insert(self.runtime_elapsed);
            }
            EventKind::NodeDeleted { node } => {
                self.node_wakes.source.close(*node);
                self.purge_event_listeners_for_node(*node);
                self.last_update_elapsed_by_node.remove(node);
                self.param_last_change_counter.remove(node);
                self.tree_revisions.remove(*node);
                self.expression_runtime.remove(node);
            }
            _ => {}
        }
    }

    /// Pushes an engine event to the UI log and inbox, then advances the per-tick event sequence counter.
    pub(crate) fn emit_event(&mut self, kind: EventKind) {
        self.apply_event_side_effects(&kind);
        let time = self.time;
        let event = Event { time, kind };
        self.push_ui_event_log(event.clone());
        self.inbox.push(event);
        self.tick_scratch.stats.events_emitted += 1;
        self.time.seq = self.time.seq.saturating_add(1);
    }

    /// Pushes an engine-internal event to the inbox without exposing it to UI replay.
    pub(crate) fn emit_inbox_event(&mut self, kind: EventKind) {
        self.apply_event_side_effects(&kind);
        let time = self.time;
        self.inbox.push(Event { time, kind });
        self.tick_scratch.stats.events_emitted += 1;
        self.time.seq = self.time.seq.saturating_add(1);
    }
}

fn opaque_project_dirty_node(edit: &Edit) -> Option<crate::node::NodeId> {
    match edit {
        Edit::SetNodeScriptProperty { node, .. }
        | Edit::CallNodeScriptMethod { node, .. }
        | Edit::CallNodeMutation { node, .. }
        | Edit::SetScriptConfig { node, .. } => Some(*node),
        _ => None,
    }
}

fn pending_edit_kind_summary(pending: &[crate::edit::EditRequest]) -> String {
    let mut counts = BTreeMap::<&'static str, usize>::new();
    for request in pending {
        *counts.entry(edit_kind_name(&request.edit)).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(kind, count)| format!("{kind}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn edit_kind_name(edit: &Edit) -> &'static str {
    match edit {
        Edit::CheckPreconditions { .. } => "CheckPreconditions",
        Edit::BeginEditSession { .. } => "BeginEditSession",
        Edit::EndEditSession { .. } => "EndEditSession",
        Edit::SetParam { .. } => "SetParam",
        Edit::SetParamConstraints { .. } => "SetParamConstraints",
        Edit::SetParamEnumOptions { .. } => "SetParamEnumOptions",
        Edit::SetParamControlState { .. } => "SetParamControlState",
        Edit::SetNodeScriptProperty { .. } => "SetNodeScriptProperty",
        Edit::CallNodeScriptMethod { .. } => "CallNodeScriptMethod",
        Edit::CallNodeMutation { .. } => "CallNodeMutation",
        Edit::AddNode { .. } => "AddNode",
        Edit::AddNodeTree { .. } => "AddNodeTree",
        Edit::AddUserItemTree { .. } => "AddUserItemTree",
        Edit::AddUserItemTrees { .. } => "AddUserItemTrees",
        Edit::AddUserItem { .. } => "AddUserItem",
        Edit::CreateBlueprintInstance { .. } => "CreateBlueprintInstance",
        Edit::ReplaceNode { .. } => "ReplaceNode",
        Edit::RemoveNode { .. } => "RemoveNode",
        Edit::MoveNode { .. } => "MoveNode",
        Edit::PatchMeta { .. } => "PatchMeta",
        Edit::SetScriptConfig { .. } => "SetScriptConfig",
        Edit::SetNodeWarning { .. } => "SetNodeWarning",
        Edit::ClearNodeWarning { .. } => "ClearNodeWarning",
        Edit::SetNodeChildWarningDepth { .. } => "SetNodeChildWarningDepth",
        Edit::EmitCustomEvent { .. } => "EmitCustomEvent",
        Edit::ReevaluateGraph => "ReevaluateGraph",
        Edit::AddEventListener { .. } => "AddEventListener",
        Edit::RemoveEventListener { .. } => "RemoveEventListener",
    }
}
