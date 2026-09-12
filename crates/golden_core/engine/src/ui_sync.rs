use std::collections::{HashMap, HashSet};

use crate::contexts::UserContextValueType;
use crate::edit::{Edit, EditOrigin};
use crate::engine::{Engine, EngineTime, ProjectLoadRecoveryReport, ProjectPersistenceError};
use crate::events::{Event, EventKind};
use crate::node::{
    CurveNode, DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_NODE_WIDGET_NODE_TYPE, DASHBOARD_PAGE_NODE_TYPE,
    DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, DeclId, FOLDER_NODE_TYPE, Node, NodeCreationContext, NodeId, NodeMeta,
    NodeMetaPatch as EngineNodeMetaPatch, NodeReference, NodeUuid, UserCreatableItem, UserCreatableItemInitialParam,
};
use crate::parameter::{
    ParamValue, ParameterConstraints, ParameterControlMode, ParameterControlSpec, ParameterControlState,
    ParameterEnumOption, ParameterEventBehaviour, RangeConstraint, available_control_modes_for_parameter,
    compatibility_for_binding_values, compatibility_for_values,
};
use crate::process_ctx::ProcessTreeSnapshot;
use crate::script::{ScriptNodeConfig, ScriptUiConfig, ScriptUiState};

pub use golden_protocol::*;

mod conversion;
mod creation;
mod snapshot;

pub(crate) const UI_USER_CONTEXT_SCOPE_TOPIC: &str = "__user_context.scope_changed";
pub(crate) const UI_USER_CONTEXT_ENTRY_TOPIC: &str = "__user_context.entry_changed";

impl<T: Node> Engine<T> {
    /// Returns script runtime state for `node` when it is a script node.
    pub fn ui_script_state(&self, node: NodeId) -> Result<ScriptUiState, String> {
        let Some(target) = self.nodes.get(node) else {
            return Err(format!("node {} not found", node.0));
        };

        target
            .engine_script_state()
            .ok_or_else(|| format!("node {} does not expose script runtime state", node.0))
    }

    /// Replaces script configuration for `node`.
    pub fn ui_set_script_config(
        &mut self,
        node: NodeId,
        config: ScriptUiConfig,
        force_reload: bool,
    ) -> Result<(), String> {
        self.edits.push(Edit::SetScriptConfig {
            node,
            config: ScriptNodeConfig::from(config),
            force_reload,
        });
        self.apply_edits().map_err(|err| err.to_string())
    }

    /// Requests runtime reload for script node `node`.
    pub fn ui_reload_script(&mut self, node: NodeId) -> Result<(), String> {
        {
            let Some(target) = self.nodes.get_mut(node) else {
                return Err(format!("node {} not found", node.0));
            };

            target.engine_request_script_reload()?;
        }

        self.push_ui_custom_event(
            "__transport.resync_required",
            Some(node),
            serde_json::json!({
                "reason": "script_reload_requested",
                "node": node.0
            }),
        );

        Ok(())
    }

    /// Applies one UI edit intent and returns an acknowledgement payload.
    pub fn apply_ui_intent(&mut self, intent: UiEditIntent) -> UiAck {
        self.apply_ui_intent_from_client(intent, None)
    }

    /// Applies one UI edit intent while attributing any opened edit session to a stable client.
    pub fn apply_ui_intent_from_client(&mut self, intent: UiEditIntent, ui_client_instance_id: Option<&str>) -> UiAck {
        let before_event_time = self.ui_event_log().last().map(|event| event.time);
        // eprintln!("[gc-ui] intent recv: {intent:?} | undo_len={} redo_len={} active_session={}", self.undo_len(), self.redo_len(), self.has_active_edit_session());

        let ack = match intent {
            UiEditIntent::BeginEdit { client_edit_id, label } => {
                self.edits.push(Edit::BeginEditSession {
                    origin: EditOrigin::Ui,
                    label,
                    client_edit_id,
                    ui_client_instance_id: ui_client_instance_id.map(str::to_owned),
                });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::EndEdit { client_edit_id } => {
                self.edits.push(Edit::EndEditSession { client_edit_id });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::SetParam { node, value, behaviour } => {
                self.edits.push(Edit::SetParam { node, value, behaviour });
                let result = self.apply_ui_stabilization_to_fixed_point(16);
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::SetTextParamSmart { node, value, behaviour } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Set text parameter",
                    "__ui-set-text-param-smart",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_set_text_param_smart(node, value, behaviour),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::SetParamControlState { node, state } => {
                match self.apply_set_param_control_state(0, node, state.into()) {
                    Ok(Some(effect)) => {
                        self.record_set_param_control_state_history(effect);
                        self.finish_ui_apply_now(before_event_time, Ok(()))
                    }
                    Ok(None) => self.finish_ui_apply_now(before_event_time, Ok(())),
                    Err(err) => self.finish_ui_apply_now(before_event_time, Err(err)),
                }
            }
            UiEditIntent::SetParamConstraints { node, constraints } => {
                match self.apply_set_param_constraints(0, node, constraints) {
                    Ok(Some(effect)) => {
                        self.record_set_param_constraints_history(effect);
                        self.finish_ui_apply_now(before_event_time, Ok(()))
                    }
                    Ok(None) => self.finish_ui_apply_now(before_event_time, Ok(())),
                    Err(err) => self.finish_ui_apply_now(before_event_time, Err(err)),
                }
            }
            UiEditIntent::MoveNode {
                node,
                new_parent,
                new_prev_sibling,
            } => {
                self.edits.push(Edit::MoveNode {
                    node,
                    new_parent,
                    new_prev_sibling,
                });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::RemoveNode { node } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Remove node",
                    "__ui-remove-node",
                    ui_client_instance_id,
                    |engine| {
                        engine.edits.push(Edit::RemoveNode { node });
                        engine.apply_ui_stabilization_to_fixed_point(16)
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::RemoveNodes { nodes } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Remove nodes",
                    "__ui-remove-nodes",
                    ui_client_instance_id,
                    |engine| {
                        let mut seen = HashSet::<NodeId>::new();
                        for node in nodes {
                            if seen.insert(node) {
                                engine.edits.push(Edit::RemoveNode { node });
                            }
                        }
                        engine.apply_ui_stabilization_to_fixed_point(16)
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::CreateUserItem {
                parent,
                node_type,
                label,
                initial_params,
            } => {
                let edit_label = label.clone().unwrap_or_else(|| "Create user item".to_string());
                let result = self.apply_implicit_ui_edit_session(
                    &edit_label,
                    "__ui-create-user-item",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_create_user_item(parent, node_type, label, initial_params),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::CreateDashboardContainerWidget {
                parent,
                label,
                placement,
                layout_kind,
                prev_sibling,
            } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Create dashboard container",
                    "__ui-create-dashboard-container-widget",
                    ui_client_instance_id,
                    |engine| {
                        engine
                            .ui_apply_create_dashboard_container_widget(
                                parent,
                                label,
                                placement,
                                layout_kind,
                                prev_sibling,
                            )
                            .map(|_| ())
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::CreateDashboardNodeWidget {
                parent,
                target,
                placement,
                prev_sibling,
            } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Create dashboard node widget",
                    "__ui-create-dashboard-node-widget",
                    ui_client_instance_id,
                    |engine| {
                        engine
                            .ui_apply_create_dashboard_node_widget(parent, target, placement, prev_sibling)
                            .map(|_| ())
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::CreateDashboardGenericWidget {
                parent,
                target,
                placement,
                prev_sibling,
            } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Create dashboard generic widget",
                    "__ui-create-dashboard-generic-widget",
                    ui_client_instance_id,
                    |engine| {
                        engine
                            .ui_apply_create_dashboard_generic_widget(parent, target, placement, prev_sibling)
                            .map(|_| ())
                    },
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::BindDashboardNodeWidgetTarget { widget, target } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Bind dashboard node widget",
                    "__ui-bind-dashboard-node-widget-target",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_bind_dashboard_node_widget_target(widget, target),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::BindDashboardGenericWidgetTarget { widget, target } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Bind dashboard generic widget",
                    "__ui-bind-dashboard-generic-widget-target",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_bind_dashboard_generic_widget_target(widget, target),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::WrapDashboardWidgetInContainer {
                widget,
                placement,
                layout_kind,
            } => {
                let result = self.apply_implicit_ui_edit_session(
                    "Wrap dashboard widget in container",
                    "__ui-wrap-dashboard-widget-in-container",
                    ui_client_instance_id,
                    |engine| engine.ui_apply_wrap_dashboard_widget_in_container(widget, placement, layout_kind),
                );
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::DuplicateNode {
                source,
                new_parent,
                new_prev_sibling,
                initial_params: _,
            } => self.rejected_ui_ack(
                "duplicate_node_transport_required",
                format!(
                    "duplicateNode requires transport-level project codec support (source={}, new_parent={}, new_prev_sibling={:?})",
                    source.0, new_parent.0, new_prev_sibling
                ),
            ),
            UiEditIntent::DuplicateNodes {
                nodes,
                created_items,
                dependent_items,
            } => self.rejected_ui_ack(
                "duplicate_nodes_transport_required",
                format!(
                    "duplicateNodes requires transport-level project codec support (nodes={}, created_items={}, dependent_items={})",
                    nodes.len(),
                    created_items.len(),
                    dependent_items.len()
                ),
            ),
            UiEditIntent::FitAnimationCurvePath { curve, points, options } => {
                self.edits.push(Edit::CallNodeMutation {
                    node: curve,
                    needs_tree_snapshot: true,
                    callback: Box::new(move |node, ctx| {
                        let Some(curve_node) = node.as_any_mut().downcast_mut::<CurveNode>() else {
                            return Err("target node should be CurveNode".to_string());
                        };
                        curve_node.replace_range_with_fitted_samples(ctx, points.as_slice(), options)?;
                        Ok(())
                    }),
                });
                let result = self.apply_edits_to_fixed_point(16);
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::PatchMeta { node, patch } => {
                self.edits.push(Edit::PatchMeta {
                    node,
                    patch: patch.into(),
                });
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::EnsureUserContextScope { owner } => match self.ensure_user_context_scope(owner) {
                Ok(changed) => {
                    if changed {
                        self.push_ui_custom_event(
                            UI_USER_CONTEXT_SCOPE_TOPIC,
                            Some(owner),
                            serde_json::json!({
                                "action": "ensure_scope",
                                "owner": owner.0,
                            }),
                        );
                    }
                    self.applied_ui_ack_since(before_event_time)
                }
                Err(message) => self.rejected_ui_ack("user_context_scope_error", message),
            },
            UiEditIntent::RemoveUserContextScope { owner } => {
                let removed = self.remove_user_context_scope(owner);
                if removed {
                    self.push_ui_custom_event(
                        UI_USER_CONTEXT_SCOPE_TOPIC,
                        Some(owner),
                        serde_json::json!({
                            "action": "remove_scope",
                            "owner": owner.0,
                        }),
                    );
                }
                self.applied_ui_ack_since(before_event_time)
            }
            UiEditIntent::UpsertUserContextEntry { owner, symbol, param } => {
                match self.upsert_user_context_entry(owner, symbol.as_str(), param) {
                    Ok(changed) => {
                        if changed {
                            self.push_ui_custom_event(
                                UI_USER_CONTEXT_ENTRY_TOPIC,
                                Some(owner),
                                serde_json::json!({
                                    "action": "upsert_entry",
                                    "owner": owner.0,
                                    "symbol": symbol,
                                    "param": param.0,
                                }),
                            );
                        }
                        self.applied_ui_ack_since(before_event_time)
                    }
                    Err(message) => self.rejected_ui_ack("user_context_entry_error", message),
                }
            }
            UiEditIntent::RemoveUserContextEntry { owner, symbol } => {
                let removed = self.remove_user_context_entry(owner, symbol.as_str());
                if removed {
                    self.push_ui_custom_event(
                        UI_USER_CONTEXT_ENTRY_TOPIC,
                        Some(owner),
                        serde_json::json!({
                            "action": "remove_entry",
                            "owner": owner.0,
                            "symbol": symbol,
                        }),
                    );
                }
                self.applied_ui_ack_since(before_event_time)
            }
            UiEditIntent::SendNodeEvent { node, topic, payload } => {
                let result = if !self.nodes.contains(node) {
                    Err(crate::engine::EngineEditError::NodeNotFound {
                        edit_index: 0,
                        operation: "SendNodeEvent",
                        node,
                    })
                } else {
                    self.emit_inbox_event(EventKind::Custom(crate::events::CustomEvent::new(
                        topic,
                        Some(node),
                        payload,
                    )));
                    self.apply_ui_stabilization_to_fixed_point(16)
                };
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::ReevaluateGraph => {
                self.edits.push(Edit::ReevaluateGraph);
                let result = self.apply_edits();
                self.finish_ui_apply_now(before_event_time, result)
            }
            UiEditIntent::ClearLogs => {
                crate::logger::clear();
                self.push_ui_custom_event(crate::logger::UI_LOG_CLEARED_TOPIC, None, serde_json::json!({}));
                self.applied_ui_ack_since(before_event_time)
            }
            UiEditIntent::SetLogMaxEntries { max_entries } => {
                let applied_max_entries = crate::logger::set_max_entries(max_entries);
                self.push_ui_custom_event(
                    crate::logger::UI_LOG_MAX_ENTRIES_TOPIC,
                    None,
                    serde_json::json!({ "max_entries": applied_max_entries }),
                );
                self.applied_ui_ack_since(before_event_time)
            }
            UiEditIntent::Undo => match self.undo() {
                Ok(true) => self.applied_ui_ack_since(before_event_time),
                Ok(false) => self.rejected_ui_ack("undo_unavailable", "there is no transaction to undo".to_string()),
                Err(err) => self.rejected_ui_ack(ui_error_code(&err), err.to_string()),
            },
            UiEditIntent::Redo => match self.redo() {
                Ok(true) => self.applied_ui_ack_since(before_event_time),
                Ok(false) => self.rejected_ui_ack("redo_unavailable", "there is no transaction to redo".to_string()),
                Err(err) => self.rejected_ui_ack(ui_error_code(&err), err.to_string()),
            },
        };

        // eprintln!(
        //     "[gc-ui] intent ack: success={} status={:?} code={:?} earliest={:?} history={{undo_len:{}, redo_len:{}, can_undo:{}, can_redo:{}, active_session:{}}}",
        //     ack.success, ack.status, ack.error_code, ack.earliest_event_time, ack.history.undo_len, ack.history.redo_len, ack.history.can_undo, ack.history.can_redo, ack.history.active_edit_session
        // );

        ack
    }

    fn apply_implicit_ui_edit_session<F>(
        &mut self,
        label: &str,
        client_edit_id: &str,
        ui_client_instance_id: Option<&str>,
        operation: F,
    ) -> Result<(), crate::engine::EngineEditError>
    where
        F: FnOnce(&mut Self) -> Result<(), crate::engine::EngineEditError>,
    {
        if self.has_active_edit_session() {
            return operation(self);
        }

        let client_edit_id = client_edit_id.to_string();
        self.edits.push(Edit::BeginEditSession {
            origin: EditOrigin::Ui,
            label: Some(label.to_string()),
            client_edit_id: client_edit_id.clone(),
            ui_client_instance_id: ui_client_instance_id.map(str::to_owned),
        });
        self.apply_edits()?;

        let operation_result = operation(self);
        self.edits.push(Edit::EndEditSession { client_edit_id });
        let end_result = self.apply_edits();

        operation_result?;
        end_result
    }

    fn applied_ui_ack_since(&self, previous_event_time: Option<EngineTime>) -> UiAck {
        let start = self.ui_event_log_start_index(previous_event_time);
        let events = &self.ui_event_log()[start..];
        UiAck {
            success: true,
            status: UiAckStatus::Applied,
            error_code: None,
            error_message: None,
            earliest_event_time: events.first().map(|event| event.time),
            latest_event_time: events.last().map(|event| event.time),
            history: self.ui_history_state(),
        }
    }

    fn rejected_ui_ack(&self, error_code: &str, error_message: String) -> UiAck {
        UiAck {
            success: false,
            status: UiAckStatus::Rejected,
            error_code: Some(error_code.to_string()),
            error_message: Some(error_message),
            earliest_event_time: None,
            latest_event_time: None,
            history: self.ui_history_state(),
        }
    }

    fn finish_ui_apply_now(
        &self,
        previous_event_time: Option<EngineTime>,
        result: Result<(), crate::engine::EngineEditError>,
    ) -> UiAck {
        match result {
            Ok(()) => self.applied_ui_ack_since(previous_event_time),
            Err(err) => self.rejected_ui_ack(ui_error_code(&err), err.to_string()),
        }
    }

    fn apply_edits_to_fixed_point(&mut self, max_passes: usize) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            self.apply_edits()?;
            if self.edits.pending.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!("ui intent left pending edits after {pass_limit} stabilization passes"),
        })
    }

    fn apply_ui_stabilization_to_fixed_point(
        &mut self,
        max_passes: usize,
    ) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            if !self.edits.pending.is_empty() {
                self.apply_edits()?;
            }

            if !self.inbox.events.is_empty() {
                self.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)?;
            }

            if self.edits.pending.is_empty() && self.inbox.events.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyStabilizationFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!("ui intent left pending edits or inbox events after {pass_limit} stabilization passes"),
        })
    }

    fn apply_ui_initialization_to_fixed_point(
        &mut self,
        max_passes: usize,
    ) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            if !self.edits.pending.is_empty() {
                self.apply_edits_without_history()?;
            }

            if !self.inbox.events.is_empty() {
                self.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)?;
            }

            if self.edits.pending.is_empty() && self.inbox.events.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "UiApplyInitializationFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!(
                "ui initializer left pending edits or inbox events after {pass_limit} stabilization passes"
            ),
        })
    }
}

fn ui_error_code(error: &crate::engine::EngineEditError) -> &'static str {
    match error {
        crate::engine::EngineEditError::NodeTypeMismatch { .. } => "node_type_mismatch",
        crate::engine::EngineEditError::ParamEditTargetMismatch { .. } => "param_edit_target_mismatch",
        crate::engine::EngineEditError::ParamConstraintViolation { .. } => "param_constraint_violation",
        crate::engine::EngineEditError::ParamConstraintsRejected { .. } => "param_constraints_rejected",
        crate::engine::EngineEditError::ParamControlStateRejected { .. } => "param_control_error",
        crate::engine::EngineEditError::ScriptConfigRejected { .. } => "script_config_rejected",
        crate::engine::EngineEditError::ScriptPropertyRejected { .. } => "script_property_rejected",
        crate::engine::EngineEditError::ScriptMethodRejected { .. } => "script_method_rejected",
        crate::engine::EngineEditError::NodeMutationRejected { .. } => "node_mutation_rejected",
        crate::engine::EngineEditError::NodeNotFound { .. } => "node_not_found",
        crate::engine::EngineEditError::ParentNotFound { .. } => "parent_not_found",
        crate::engine::EngineEditError::SiblingNotFound { .. } => "sibling_not_found",
        crate::engine::EngineEditError::InvalidSiblingParent { .. } => "invalid_sibling_parent",
        crate::engine::EngineEditError::InvalidSiblingReference { .. } => "invalid_sibling_reference",
        crate::engine::EngineEditError::CannotMutateRoot { .. } => "cannot_mutate_root",
        crate::engine::EngineEditError::CycleDetected { .. } => "cycle_detected",
        crate::engine::EngineEditError::EditSessionAlreadyActive { .. } => "edit_session_already_active",
        crate::engine::EngineEditError::EditSessionNotActive { .. } => "edit_session_not_active",
        crate::engine::EngineEditError::EditSessionIdMismatch { .. } => "edit_session_id_mismatch",
        crate::engine::EngineEditError::UserItemContainerRequired { .. } => "user_item_container_required",
        crate::engine::EngineEditError::UserItemKindRejected { .. } => "user_item_kind_rejected",
        crate::engine::EngineEditError::UserItemTypeUnavailable { .. } => "user_item_type_unavailable",
    }
}
