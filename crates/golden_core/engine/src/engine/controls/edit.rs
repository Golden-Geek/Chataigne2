use super::*;
use crate::engine::apply_tree::AppliedEdit;

impl<T: Node> Engine<T> {
    fn set_param_control_state_impl(
        &mut self,
        param: NodeId,
        mut state: ParameterControlState,
    ) -> Result<bool, String> {
        if !control_spec_matches_mode(state.mode, &state.spec) {
            return Err("parameter control state has a mode/spec mismatch".to_string());
        }

        let Some(node) = self.nodes.get(param) else {
            return Err(format!("parameter node {} was not found", param.0));
        };
        let Some(snapshot) = node.engine_param_snapshot() else {
            return Err(format!("node {} is not a parameter node", param.0));
        };
        if !available_control_modes_for_parameter(&snapshot.value, snapshot.control_modes_enabled).contains(&state.mode)
        {
            return Err(format!(
                "control mode '{:?}' is not supported for parameter type '{}'",
                state.mode,
                node.get_type()
            ));
        }
        if matches!(state.mode, ParameterControlMode::TemplateText)
            && self.ui_context_candidates_for_param(param).candidates.is_empty()
        {
            return Err("control mode 'templateText' requires at least one visible context entry".to_string());
        }

        let Some(current_state) = node.engine_param_control_state() else {
            return Err(format!("node {} does not expose parameter control state", param.0));
        };
        state.diagnostics.clear();
        self.sync_parameter_control_nodes(param, state.mode)?;
        if current_state == state {
            return Ok(false);
        }

        let next_state = state.clone();
        let Some(node) = self.nodes.get_mut(param) else {
            return Err(format!("parameter node {} was not found", param.0));
        };
        node.engine_set_param_control_state(state)?;
        self.mark_param_control_index_dirty();
        self.emit_event(EventKind::ParamControlChanged {
            param,
            old_state: current_state,
            new_state: next_state,
        });
        Ok(true)
    }

    /// Sets control state on one parameter node.
    ///
    /// Returns `true` when the state changed.
    pub fn set_param_control_state(&mut self, param: NodeId, state: ParameterControlState) -> Result<bool, String> {
        self.set_param_control_state_impl(param, state)
    }

    fn param_control_rejected_error(
        &self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        message: String,
    ) -> EngineEditError {
        let node_type = self
            .nodes
            .get(node)
            .map(|entry| entry.get_type().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        EngineEditError::ParamControlStateRejected {
            edit_index,
            operation,
            node,
            node_type,
            message,
        }
    }

    fn read_param_control_state_for_edit(
        &self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
    ) -> Result<ParameterControlState, EngineEditError> {
        let Some(target) = self.nodes.get(node) else {
            return Err(EngineEditError::NodeNotFound {
                edit_index,
                operation,
                node,
            });
        };
        target.engine_param_control_state().ok_or_else(|| {
            self.param_control_rejected_error(
                edit_index,
                operation,
                node,
                "target node does not expose parameter control state".to_string(),
            )
        })
    }

    fn apply_set_param_control_state_internal(
        &mut self,
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        state: ParameterControlState,
    ) -> Result<AppliedEdit<Option<SetParamControlStateEffect>>, EngineEditError> {
        let old_state = self.read_param_control_state_for_edit(edit_index, operation, node)?;
        let changed = self
            .set_param_control_state_impl(node, state)
            .map_err(|message| self.param_control_rejected_error(edit_index, operation, node, message))?;
        if !changed {
            return Ok(AppliedEdit {
                effect: None,
                completion: Ok(()),
            });
        }

        let new_state = self.read_param_control_state_for_edit(edit_index, operation, node)?;
        let effect = Some(SetParamControlStateEffect {
            node,
            old_state,
            new_state,
        });
        let _ = self.evaluate_parameter_controls();
        let completion = if self.edits.pending.is_empty() {
            Ok(())
        } else {
            self.apply_edits_without_history()
        };
        Ok(AppliedEdit { effect, completion })
    }

    pub(crate) fn apply_set_param_control_state(
        &mut self,
        edit_index: usize,
        node: NodeId,
        state: ParameterControlState,
    ) -> Result<AppliedEdit<Option<SetParamControlStateEffect>>, EngineEditError> {
        self.apply_set_param_control_state_internal(edit_index, "SetParamControlState", node, state)
    }

    pub(crate) fn apply_set_param_control_state_for_history(
        &mut self,
        operation: &'static str,
        node: NodeId,
        state: ParameterControlState,
    ) -> Result<(), EngineEditError> {
        let old_state = self.read_param_control_state_for_edit(0, operation, node)?;
        let node_type = self
            .nodes
            .get(node)
            .expect("validated control target")
            .get_type()
            .to_owned();
        self.nodes
            .get_mut(node)
            .expect("validated control target")
            .engine_set_param_control_state(state.clone())
            .map_err(|message| EngineEditError::ParamControlStateRejected {
                edit_index: 0,
                operation,
                node,
                node_type,
                message,
            })?;
        self.mark_param_control_index_dirty();
        if old_state != state {
            self.emit_event(EventKind::ParamControlChanged {
                param: node,
                old_state,
                new_state: state,
            });
        }
        Ok(())
    }

    fn make_expression_source_parameter(initial_expression: String) -> Parameter {
        let mut source = Parameter::new(
            "Expression",
            ParamValue::Str(initial_expression),
            ParameterChangeCheck::ValueChange,
        );
        source.node_data_mut().meta.decl_id = DeclId(PARAMETER_EXPRESSION_SOURCE_DECL_ID.to_string());
        source.node_data_mut().meta.can_be_disabled = false;
        source.control_modes_enabled = false;
        source
    }

    fn make_control_reference_parameter() -> Parameter {
        let mut reference = Parameter::new(
            "Reference",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::ValueChange,
        );
        reference.node_data_mut().meta.decl_id = DeclId(PARAMETER_CONTROL_REFERENCE_DECL_ID.to_string());
        reference.node_data_mut().meta.can_be_disabled = false;
        reference.control_modes_enabled = false;
        reference.constraints.reference.target_kind = ReferenceTargetKind::ParameterOnly;
        reference
    }

    fn sync_parameter_control_nodes(&mut self, param: NodeId, mode: ParameterControlMode) -> Result<(), String> {
        let children = self.direct_children(param);
        let mut control_reference_params = Vec::<NodeId>::new();
        let mut animation_nodes = Vec::<NodeId>::new();
        let mut expression_source_params = Vec::<NodeId>::new();

        for child in children {
            let Some(child_node) = self.nodes.get(child) else {
                continue;
            };

            if child_node.get_type() == PARAMETER_ANIMATION_CONTROL_NODE_TYPE {
                animation_nodes.push(child);
                continue;
            }
            if child_node.node_data().meta.decl_id.0 == PARAMETER_EXPRESSION_SOURCE_DECL_ID
                && child_node.engine_param_snapshot().is_some()
            {
                expression_source_params.push(child);
                continue;
            }
            if child_node.node_data().meta.decl_id.0 == PARAMETER_CONTROL_REFERENCE_DECL_ID
                && child_node.engine_param_snapshot().is_some()
            {
                control_reference_params.push(child);
            }
        }

        let mut changed = false;
        let mut to_remove = Vec::<NodeId>::new();

        match mode {
            ParameterControlMode::Proxy | ParameterControlMode::Binding => {
                if control_reference_params.is_empty() {
                    self.edits.push(Edit::AddNode {
                        parent: param,
                        node: Box::new(Self::make_control_reference_parameter()),
                        prev_sibling: None,
                    });
                    changed = true;
                } else {
                    to_remove.extend(control_reference_params.into_iter().skip(1));
                }
                to_remove.extend(expression_source_params);
                to_remove.extend(animation_nodes);
            }
            ParameterControlMode::Expression => {
                if expression_source_params.is_empty() {
                    self.edits.push(Edit::AddNode {
                        parent: param,
                        node: Box::new(Self::make_expression_source_parameter(String::new())),
                        prev_sibling: None,
                    });
                    changed = true;
                } else {
                    to_remove.extend(expression_source_params.into_iter().skip(1));
                }
                to_remove.extend(control_reference_params);
                to_remove.extend(animation_nodes);
            }
            ParameterControlMode::Animation => {
                if animation_nodes.is_empty() {
                    self.edits.push(Edit::AddNode {
                        parent: param,
                        node: Box::new(ParameterAnimationControlNode::new("Animation")),
                        prev_sibling: None,
                    });
                    changed = true;
                } else {
                    to_remove.extend(animation_nodes.into_iter().skip(1));
                }
                to_remove.extend(control_reference_params);
                to_remove.extend(expression_source_params);
            }
            _ => {
                to_remove.extend(control_reference_params);
                to_remove.extend(animation_nodes);
                to_remove.extend(expression_source_params);
            }
        }

        for node in to_remove {
            self.edits.push(Edit::RemoveNode { node });
            changed = true;
        }

        if changed {
            self.apply_edits_without_history()
                .map_err(|err| format!("failed to sync control nodes for parameter {}: {err}", param.0))?;
        }

        Ok(())
    }
}
