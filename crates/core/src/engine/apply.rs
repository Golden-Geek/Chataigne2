use crate::contexts::UserContextValueType;
use crate::edit::Edit;
use crate::events::{Event, EventKind};
use crate::node::{Node, NodeCreationContext};
use crate::parameter::ParamValue;
use std::{any::type_name, collections::BTreeMap};

use super::history::{HistoryStep, HistoryTransaction};
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Applies all pending edits in queue order and emits resulting events.
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
        self.absorb_external_edits()?;

        let mut transaction = HistoryTransaction::new();
        let mut redo_cleared = false;
        let mut missing_reference_warning_dirty = false;
        let mut user_context_graph_dirty = false;

        for (edit_index, request) in self.edits.drain().into_iter().enumerate() {
            let (outcome, should_clear_redo): (Result<Option<HistoryStep<T>>, EngineEditError>, bool) = match request
                .edit
            {
                Edit::BeginEditSession {
                    origin,
                    label,
                    client_edit_id,
                    ui_client_instance_id,
                } => {
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
                Edit::SetParam { node, value, behaviour } => match self.apply_set_param(edit_index, node, value)? {
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
                },
                Edit::SetParamConstraints { node, constraints } => {
                    missing_reference_warning_dirty = true;
                    let effect = self.apply_set_param_constraints(edit_index, node, constraints)?;
                    let should_clear_redo = effect.is_some();
                    (Ok(effect.map(Into::into)), should_clear_redo)
                }
                Edit::SetNodeScriptProperty { node, property, value } => {
                    self.apply_set_node_script_property(edit_index, node, property, value)?;
                    (Ok(None), true)
                }
                Edit::CallNodeScriptMethod { node, method, args } => {
                    self.apply_call_node_script_method(edit_index, node, method, args)?;
                    (Ok(None), true)
                }
                Edit::CallNodeMutation {
                    node,
                    callback,
                    needs_tree_snapshot,
                } => {
                    self.apply_call_node_mutation(edit_index, node, callback, needs_tree_snapshot)?;
                    (Ok(None), true)
                }
                Edit::AddNode {
                    node,
                    parent,
                    prev_sibling,
                } => {
                    missing_reference_warning_dirty = true;
                    user_context_graph_dirty = true;
                    let effect = self.apply_add_node(edit_index, node, parent, prev_sibling, creation_context)?;
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
                    let effect = self.apply_add_node_tree(edit_index, tree, parent, prev_sibling, creation_context)?;
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
                    let effect =
                        self.apply_add_user_item_tree(edit_index, tree, parent, prev_sibling, creation_context)?;
                    if self.queue_user_context_multiplex_resize_for_list(effect.node) {
                        user_context_graph_dirty = true;
                    }
                    (Ok(Some(effect.into())), true)
                }
                Edit::AddUserItem {
                    node,
                    parent,
                    prev_sibling,
                } => {
                    missing_reference_warning_dirty = true;
                    user_context_graph_dirty = true;
                    let effect = self.apply_add_user_item(edit_index, node, parent, prev_sibling, creation_context)?;
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
                    let effect = self.apply_create_blueprint_instance(
                        edit_index,
                        blueprint_id,
                        parent,
                        prev_sibling,
                        label,
                        creation_context,
                    )?;
                    (Ok(Some(effect.into())), true)
                }
                Edit::ReplaceNode { node, new_node } => {
                    missing_reference_warning_dirty = true;
                    user_context_graph_dirty = true;
                    let effect = self.apply_replace_node(edit_index, node, new_node)?;
                    (Ok(Some(effect.into())), true)
                }
                Edit::RemoveNode { node } => {
                    missing_reference_warning_dirty = true;
                    user_context_graph_dirty = true;
                    let effect = self.apply_remove_node(edit_index, node, creation_context)?;
                    (Ok(Some(effect.into())), true)
                }
                Edit::MoveNode {
                    node,
                    new_parent,
                    new_prev_sibling,
                } => {
                    missing_reference_warning_dirty = true;
                    user_context_graph_dirty = true;
                    let effect = self.apply_move_node(edit_index, node, new_parent, new_prev_sibling)?;
                    (Ok(Some(effect.into())), true)
                }
                Edit::PatchMeta { node, patch } => {
                    missing_reference_warning_dirty = true;
                    let effect = self.apply_patch_meta(edit_index, node, patch)?;
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
                    self.emit_event(EventKind::Custom(event));
                    (Ok(None), true)
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

            match outcome {
                Ok(step) => {
                    if capture_history && should_clear_redo && !redo_cleared {
                        self.clear_redo_history();
                        redo_cleared = true;
                    }

                    if capture_history && let Some(step) = step {
                        if self.active_edit_session.is_some() {
                            self.push_step_into_active_history_transaction(step);
                        } else {
                            transaction.push(step);
                        }
                    }
                }
                Err(err) => {
                    if capture_history && !transaction.is_empty() {
                        self.push_undo_transaction(transaction);
                    }
                    return Err(err);
                }
            }
        }

        if capture_history && !transaction.is_empty() {
            self.push_undo_transaction(transaction);
        }
        if missing_reference_warning_dirty {
            self.sync_missing_reference_warnings();
        }
        if user_context_graph_dirty {
            self.rebuild_user_context_registry_from_nodes();
            self.mark_user_context_graph_changed();
        }

        Ok(())
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
        match &kind {
            EventKind::NodeCreated { node } => {
                self.last_update_elapsed_by_node
                    .entry(*node)
                    .or_insert(self.runtime_elapsed);
            }
            EventKind::NodeDeleted { node } => {
                self.purge_event_listeners_for_node(*node);
                self.last_update_elapsed_by_node.remove(node);
                self.param_last_change_counter.remove(node);
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
        Edit::BeginEditSession { .. } => "BeginEditSession",
        Edit::EndEditSession { .. } => "EndEditSession",
        Edit::SetParam { .. } => "SetParam",
        Edit::SetParamConstraints { .. } => "SetParamConstraints",
        Edit::SetNodeScriptProperty { .. } => "SetNodeScriptProperty",
        Edit::CallNodeScriptMethod { .. } => "CallNodeScriptMethod",
        Edit::CallNodeMutation { .. } => "CallNodeMutation",
        Edit::AddNode { .. } => "AddNode",
        Edit::AddNodeTree { .. } => "AddNodeTree",
        Edit::AddUserItemTree { .. } => "AddUserItemTree",
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
