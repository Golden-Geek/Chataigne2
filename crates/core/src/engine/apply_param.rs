use crate::edit::NodeMutation;
use crate::events::EventKind;
use crate::node::{Node, NodeId, NodeMetaPatch, NodeWarning};
use crate::parameter::{ParamValue, ParameterChangeCheck, ParameterConstraints, ParameterEventBehaviour};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};
use crate::script::ScriptNodeConfig;
use crate::ui_sync::{UiGraphOp, UiNodeMetaPatch};

use super::history::{PatchMetaEffect, SetParamConstraintsEffect, SetParamEffect, SetScriptConfigEffect};
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Applies a parameter value update and returns the captured before/after effect for history.
    pub(crate) fn apply_set_param(
        &mut self,
        edit_index: usize,
        node: NodeId,
        value: ParamValue,
    ) -> Result<Option<SetParamEffect>, EngineEditError> {
        const OP: &str = "SetParam";
        let node_type_hint = self
            .nodes
            .get(node)
            .map(|target| target.get_type().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let value = if let ParamValue::Reference(reference) = value {
            let normalized = self
                .normalize_reference_value_for_param(node, reference)
                .map_err(|message| EngineEditError::ParamConstraintViolation {
                    edit_index,
                    node,
                    node_type: node_type_hint.clone(),
                    message,
                })?;
            ParamValue::Reference(normalized)
        } else {
            value
        };

        let (old_value, new_value, change_check) = {
            let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?;
            let node_type = target.get_type().to_string();
            let prepared_value = target.engine_prepare_param_value(value).map_err(|message| {
                EngineEditError::ParamConstraintViolation {
                    edit_index,
                    node,
                    node_type: node_type.clone(),
                    message,
                }
            })?;
            let snapshot = target.engine_param_snapshot();
            let change_check = snapshot.as_ref().map(|state| state.change_check.clone());

            if !matches!(&prepared_value, ParamValue::Trigger())
                && matches!(change_check, Some(ParameterChangeCheck::ValueChange))
                && snapshot.as_ref().is_some_and(|state| state.value == prepared_value)
            {
                return Ok(None);
            }

            let new_value = prepared_value.clone();

            match target.engine_set_param_value(prepared_value) {
                Some(old) => (old, new_value, change_check),
                None => {
                    return Err(EngineEditError::ParamEditTargetMismatch {
                        edit_index,
                        node,
                        node_type,
                    });
                }
            }
        };

        let should_emit = match change_check {
            Some(ParameterChangeCheck::None) => true,
            Some(ParameterChangeCheck::ValueChange) => {
                matches!(&new_value, ParamValue::Trigger()) || old_value != new_value
            }
            None => true,
        };

        if !should_emit {
            return Ok(None);
        }

        // eprintln!("[gc-engine] apply_set_param node={:?} old={:?} new={:?}", node, old_value, new_value);

        self.emit_event(EventKind::ParamChanged {
            param: node,
            old_value: old_value.clone(),
            new_value: new_value.clone(),
        });
        self.param_change_counter = self.param_change_counter.saturating_add(1);
        self.param_last_change_counter.insert(node, self.param_change_counter);
        if !self.parameter_values_dirty {
            self.parameter_values_cache.insert(node, new_value.clone());
        }

        Ok(Some(SetParamEffect {
            node,
            old_value,
            new_value,
            behaviour: ParameterEventBehaviour::Append,
            tick: self.time.tick,
        }))
    }

    /// Applies a live parameter-constraints update and emits value/constraint events as needed.
    pub(crate) fn apply_set_param_constraints(
        &mut self,
        edit_index: usize,
        node: NodeId,
        constraints: ParameterConstraints,
    ) -> Result<Option<SetParamConstraintsEffect>, EngineEditError> {
        const OP: &str = "SetParamConstraints";
        let old_snapshot = self.capture_param_snapshot(edit_index, OP, node)?;

        let node_type = self
            .nodes
            .get(node)
            .map(|target| target.get_type().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        {
            let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?;
            target.engine_set_param_constraints(constraints).map_err(|message| {
                EngineEditError::ParamConstraintsRejected {
                    edit_index,
                    operation: OP,
                    node,
                    node_type: node_type.clone(),
                    message,
                }
            })?;
        }

        let new_snapshot = self.capture_param_snapshot(edit_index, OP, node)?;
        if old_snapshot.constraints == new_snapshot.constraints && old_snapshot.value == new_snapshot.value {
            return Ok(None);
        }

        self.emit_param_events_for_state_change(node, &old_snapshot.value, &new_snapshot.value);
        self.emit_param_constraints_event(node, old_snapshot.constraints.clone(), new_snapshot.constraints.clone());

        Ok(Some(SetParamConstraintsEffect {
            node,
            old_constraints: old_snapshot.constraints,
            new_constraints: new_snapshot.constraints,
            old_value: old_snapshot.value,
            new_value: new_snapshot.value,
        }))
    }

    /// Validates a node target for metadata changes and emits the corresponding meta-changed event.
    pub(crate) fn apply_patch_meta(
        &mut self,
        edit_index: usize,
        node: NodeId,
        patch: NodeMetaPatch,
    ) -> Result<PatchMetaEffect, EngineEditError> {
        const OP: &str = "PatchMeta";

        let subtree_ids = patch
            .enabled
            .map(|_| self.collect_subtree_node_ids(node))
            .unwrap_or_default();
        let old_effective_enabled = subtree_ids
            .iter()
            .map(|node_id| (*node_id, self.is_effectively_enabled(*node_id)))
            .collect::<Vec<_>>();

        let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation: OP,
            node,
        })?;
        let old_meta = target.node_data().meta.clone();
        patch.apply_to(&mut target.node_data_mut().meta);
        let new_meta = target.node_data().meta.clone();

        if old_meta.enabled != new_meta.enabled {
            self.mark_schedule_dirty();
        }

        if old_meta.enabled != new_meta.enabled {
            let effective_enabled_changes = old_effective_enabled
                .into_iter()
                .filter_map(|(node_id, was_enabled)| {
                    let enabled = self.is_effectively_enabled(node_id);
                    (enabled != was_enabled).then_some((node_id, enabled))
                })
                .collect::<Vec<_>>();
            self.queue_effective_enabled_callbacks(&effective_enabled_changes)?;
        }

        let ui_patch = UiNodeMetaPatch::from(&patch);
        self.emit_inbox_event(EventKind::MetaChanged { node, patch });
        if !ui_patch.is_empty() {
            self.push_ui_graph_transaction(vec![UiGraphOp::NodeMetaPatched { node, patch: ui_patch }]);
        }
        Ok(PatchMetaEffect {
            node,
            old_meta,
            new_meta,
        })
    }

    fn apply_script_config_inner(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        config: ScriptNodeConfig,
        force_reload: bool,
    ) -> Result<(), EngineEditError> {
        let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation,
            node,
        })?;
        let node_type = target.get_type().to_string();
        target
            .engine_set_script_config(config, force_reload)
            .map_err(|message| EngineEditError::ScriptConfigRejected {
                edit_index,
                operation,
                node,
                node_type,
                message,
            })?;

        if force_reload {
            self.mark_schedule_dirty();
            self.push_ui_custom_event(
                "__transport.resync_required",
                Some(node),
                serde_json::json!({
                    "reason": "script_config_updated",
                    "node": node.0
                }),
            );
        }
        Ok(())
    }

    /// Applies a script-config update and returns captured before/after payload for history.
    pub(crate) fn apply_set_script_config(
        &mut self,
        edit_index: usize,
        node: NodeId,
        config: ScriptNodeConfig,
        force_reload: bool,
    ) -> Result<Option<SetScriptConfigEffect>, EngineEditError> {
        const OP: &str = "SetScriptConfig";

        let old_config = {
            let target = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?;
            let node_type = target.get_type().to_string();
            let Some(state) = target.engine_script_state() else {
                return Err(EngineEditError::ScriptConfigRejected {
                    edit_index,
                    operation: OP,
                    node,
                    node_type,
                    message: "node does not expose script configuration".to_string(),
                });
            };
            ScriptNodeConfig::from(state.config)
        };

        if old_config == config && !force_reload {
            return Ok(None);
        }

        let new_config = config.clone();
        self.apply_script_config_inner(edit_index, OP, node, config, force_reload)?;
        Ok(Some(SetScriptConfigEffect {
            node,
            old_config,
            new_config,
            force_reload,
        }))
    }

    /// Applies a script-property write on one target node.
    pub(crate) fn apply_set_node_script_property(
        &mut self,
        edit_index: usize,
        node: NodeId,
        property: String,
        value: ParamValue,
    ) -> Result<(), EngineEditError> {
        const OP: &str = "SetNodeScriptProperty";
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        ctx.runtime_elapsed = self.runtime_elapsed;
        let handled = {
            let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?;
            let node_type = target.get_type().to_string();
            target
                .engine_set_script_property(&mut ctx, property.as_str(), value)
                .map_err(|message| EngineEditError::ScriptPropertyRejected {
                    edit_index,
                    operation: OP,
                    node,
                    node_type,
                    message,
                })?
        };

        if !handled {
            let node_type = self
                .nodes
                .get(node)
                .map(|entry| entry.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            return Err(EngineEditError::ScriptPropertyRejected {
                edit_index,
                operation: OP,
                node,
                node_type,
                message: format!("property '{property}' is not exposed by target node"),
            });
        }

        self.absorb_edits(&mut ctx)?;
        Ok(())
    }

    /// Applies a script-method call on one target node.
    pub(crate) fn apply_call_node_script_method(
        &mut self,
        edit_index: usize,
        node: NodeId,
        method: String,
        args: Vec<ParamValue>,
    ) -> Result<(), EngineEditError> {
        const OP: &str = "CallNodeScriptMethod";
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        ctx.runtime_elapsed = self.runtime_elapsed;
        ctx.set_tree_snapshot(self.get_or_build_tick_snapshot());
        let handled = {
            let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?;
            let node_type = target.get_type().to_string();
            target
                .engine_call_script_method(&mut ctx, method.as_str(), args.as_slice())
                .map_err(|message| EngineEditError::ScriptMethodRejected {
                    edit_index,
                    operation: OP,
                    node,
                    node_type,
                    message,
                })?
        };

        if !handled {
            let node_type = self
                .nodes
                .get(node)
                .map(|entry| entry.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            return Err(EngineEditError::ScriptMethodRejected {
                edit_index,
                operation: OP,
                node,
                node_type,
                message: format!("method '{method}' is not exposed by target node"),
            });
        }

        self.absorb_edits(&mut ctx)?;
        Ok(())
    }

    /// Applies one typed runtime node-mutation callback on one target node.
    pub(crate) fn apply_call_node_mutation(
        &mut self,
        edit_index: usize,
        node: NodeId,
        callback: NodeMutation,
    ) -> Result<(), EngineEditError> {
        const OP: &str = "CallNodeMutation";
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
        ctx.runtime_elapsed = self.runtime_elapsed;
        ctx.set_tree_snapshot(self.get_or_build_tick_snapshot());

        {
            let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?;
            let node_type = target.get_type().to_string();
            callback(target as &mut dyn Node, &mut ctx).map_err(|message| EngineEditError::NodeMutationRejected {
                edit_index,
                operation: OP,
                node,
                node_type,
                message,
            })?;
        }

        self.absorb_edits(&mut ctx)?;
        Ok(())
    }

    pub(crate) fn apply_script_config_for_history(
        &mut self,
        operation: &'static str,
        node: NodeId,
        config: ScriptNodeConfig,
        force_reload: bool,
    ) -> Result<(), EngineEditError> {
        self.apply_script_config_inner(0, operation, node, config, force_reload)
    }

    pub(crate) fn apply_restore_param_state_for_history(
        &mut self,
        operation: &'static str,
        node: NodeId,
        value: ParamValue,
        constraints: ParameterConstraints,
    ) -> Result<(), EngineEditError> {
        let old_snapshot = self.capture_param_snapshot(0, operation, node)?;
        let node_type = self
            .nodes
            .get(node)
            .map(|target| target.get_type().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        {
            let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound {
                edit_index: 0,
                operation,
                node,
            })?;
            target
                .engine_restore_param_state(value, constraints)
                .map_err(|message| EngineEditError::ParamConstraintsRejected {
                    edit_index: 0,
                    operation,
                    node,
                    node_type: node_type.clone(),
                    message,
                })?;
        }

        let new_snapshot = self.capture_param_snapshot(0, operation, node)?;
        self.emit_param_events_for_state_change(node, &old_snapshot.value, &new_snapshot.value);
        self.emit_param_constraints_event(node, old_snapshot.constraints, new_snapshot.constraints);
        Ok(())
    }

    /// Sets/replaces one warning by id and emits `MetaChanged` when runtime metadata changed.
    pub(crate) fn apply_set_node_warning(
        &mut self,
        edit_index: usize,
        node: NodeId,
        warning: NodeWarning,
    ) -> Result<Option<PatchMetaEffect>, EngineEditError> {
        const OP: &str = "SetNodeWarning";

        let current_presentation = self
            .nodes
            .get(node)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?
            .node_data()
            .meta
            .presentation
            .clone();

        let mut next_presentation = current_presentation.clone();
        next_presentation.set_warning(warning);
        if next_presentation == current_presentation {
            return Ok(None);
        }

        let effect = self.apply_patch_meta(
            edit_index,
            node,
            NodeMetaPatch {
                presentation: Some(next_presentation),
                ..Default::default()
            },
        )?;
        Ok(Some(effect))
    }

    /// Clears one warning by id or all warnings when id is omitted.
    pub(crate) fn apply_clear_node_warning(
        &mut self,
        edit_index: usize,
        node: NodeId,
        warning_id: Option<String>,
    ) -> Result<Option<PatchMetaEffect>, EngineEditError> {
        const OP: &str = "ClearNodeWarning";

        let current_presentation = self
            .nodes
            .get(node)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?
            .node_data()
            .meta
            .presentation
            .clone();

        let mut next_presentation = current_presentation.clone();
        let changed = match warning_id.as_deref() {
            Some(id) => next_presentation.clear_warning(Some(id)),
            None => next_presentation.clear_warnings(),
        };
        if !changed {
            return Ok(None);
        }

        let effect = self.apply_patch_meta(
            edit_index,
            node,
            NodeMetaPatch {
                presentation: Some(next_presentation),
                ..Default::default()
            },
        )?;
        Ok(Some(effect))
    }

    /// Sets warning surfacing depth for descendant warnings.
    pub(crate) fn apply_set_node_child_warning_depth(
        &mut self,
        edit_index: usize,
        node: NodeId,
        max_depth: u32,
    ) -> Result<Option<PatchMetaEffect>, EngineEditError> {
        const OP: &str = "SetNodeChildWarningDepth";

        let current_presentation = self
            .nodes
            .get(node)
            .ok_or(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            })?
            .node_data()
            .meta
            .presentation
            .clone();

        let mut next_presentation = current_presentation.clone();
        if !next_presentation.set_child_warning_depth(max_depth) {
            return Ok(None);
        }

        let effect = self.apply_patch_meta(
            edit_index,
            node,
            NodeMetaPatch {
                presentation: Some(next_presentation),
                ..Default::default()
            },
        )?;
        Ok(Some(effect))
    }

    fn capture_param_snapshot(
        &self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
    ) -> Result<crate::parameter::ParameterSnapshot, EngineEditError> {
        let target = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound {
            edit_index,
            operation,
            node,
        })?;
        let node_type = target.get_type().to_string();
        target
            .engine_param_snapshot()
            .ok_or(EngineEditError::ParamEditTargetMismatch {
                edit_index,
                node,
                node_type,
            })
    }

    fn emit_param_events_for_state_change(&mut self, node: NodeId, old_value: &ParamValue, new_value: &ParamValue) {
        if old_value == new_value {
            return;
        }

        self.emit_event(EventKind::ParamChanged {
            param: node,
            old_value: old_value.clone(),
            new_value: new_value.clone(),
        });
        self.param_change_counter = self.param_change_counter.saturating_add(1);
        self.param_last_change_counter.insert(node, self.param_change_counter);
    }

    fn emit_param_constraints_event(
        &mut self,
        node: NodeId,
        old_constraints: ParameterConstraints,
        new_constraints: ParameterConstraints,
    ) {
        if old_constraints == new_constraints {
            return;
        }

        self.emit_event(EventKind::ParamConstraintsChanged {
            param: node,
            old_constraints,
            new_constraints,
        });
    }
}
