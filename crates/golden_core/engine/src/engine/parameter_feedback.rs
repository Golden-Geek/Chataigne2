//! Runtime scalar values share normal reads/events while authored snapshots retain intent.
use super::{Engine, EngineEditError};
use crate::{
    node::{Node, NodeId},
    parameter::{ParamValue, Parameter},
};

pub(super) struct PendingParameterFeedback {
    pub value: Option<ParamValue>,
    pub expected_input: Option<super::EngineTime>,
    pub guard: Option<golden_io::RevisionGuard>,
    pub source: Option<NodeId>,
}

impl<T: Node> Engine<T> {
    pub(super) fn apply_param_feedback(
        &mut self,
        edit_index: usize,
        node: NodeId,
        feedback: PendingParameterFeedback,
    ) -> Result<(), EngineEditError> {
        let PendingParameterFeedback {
            value,
            expected_input,
            guard,
            source,
        } = feedback;
        if guard.as_ref().is_some_and(|guard| !guard.is_current()) {
            return Ok(());
        }
        if let Some(source) = source
            && self.nodes.get(source).is_none()
        {
            return Ok(());
        }
        let Some(target) = self.nodes.get_mut(node) else {
            return Ok(());
        };
        let node_type = target.get_type().to_string();
        let parameter = target.as_any_mut().downcast_mut::<Parameter>().ok_or_else(|| {
            EngineEditError::ParamEditTargetMismatch {
                edit_index,
                node,
                node_type: node_type.clone(),
            }
        })?;
        if !parameter.read_only && !parameter.transient_value {
            return Err(EngineEditError::ParamConstraintViolation {
                edit_index,
                node,
                node_type,
                message: "runtime status feedback requires a read-only or transient Parameter".into(),
            });
        }
        if !matches!(
            parameter.value,
            ParamValue::Bool(_) | ParamValue::Int(_) | ParamValue::Float(_)
        ) {
            return Err(EngineEditError::ParamConstraintViolation {
                edit_index,
                node,
                node_type,
                message: "runtime feedback requires a bool, int or float Parameter".into(),
            });
        }
        if parameter.transient_value && parameter.runtime_input_time != expected_input {
            return Ok(());
        }
        // Restoring the baseline must also respect currently available bounds.
        let reset = value.is_none();
        let value = parameter
            .engine_prepare_param_value(value.unwrap_or_else(|| parameter.value.clone()))
            .map_err(|message| EngineEditError::ParamConstraintViolation {
                edit_index,
                node,
                node_type,
                message,
            })?;
        let value = if reset && value == parameter.value {
            None
        } else {
            Some(value)
        };
        if parameter.transient_value
            && let Some(session) = &mut self.active_edit_session
            && session.transient_parameters.contains(&node)
        {
            session.feedback.insert(
                node,
                PendingParameterFeedback {
                    value,
                    expected_input,
                    guard,
                    source,
                },
            );
            return Ok(());
        }
        let old = parameter.get().clone();
        parameter.set_runtime_value(value);
        let new = parameter.get().clone();
        self.clear_tick_tree_snapshot();
        self.emit_param_events_for_state_change(node, &old, &new, source);
        Ok(())
    }
}

impl<T: Node> Engine<T> {
    pub(super) fn apply_param_runtime_range(
        &mut self,
        edit_index: usize,
        node: NodeId,
        range: Option<crate::parameter::RangeConstraint>,
        source: Option<NodeId>,
        guard: Option<golden_io::RevisionGuard>,
    ) -> Result<(), EngineEditError> {
        if guard.as_ref().is_some_and(|guard| !guard.is_current()) {
            return Ok(());
        }
        if source.is_some_and(|source| self.nodes.get(source).is_none()) {
            return Ok(());
        }
        let Some(target) = self.nodes.get_mut(node) else {
            return Ok(());
        };
        let node_type = target.get_type().to_string();
        let rejected = |message: String| EngineEditError::ParamConstraintViolation {
            edit_index,
            node,
            node_type: node_type.clone(),
            message,
        };
        let parameter = target
            .as_any_mut()
            .downcast_mut::<Parameter>()
            .ok_or_else(|| rejected("runtime bounds require a built-in Parameter".into()))?;
        if (!parameter.read_only && !parameter.transient_value)
            || !matches!(parameter.value, ParamValue::Int(_) | ParamValue::Float(_))
        {
            return Err(rejected(
                "runtime bounds require a read-only or transient numeric Parameter".into(),
            ));
        }
        let old = parameter.snapshot();
        parameter.set_runtime_range(range).map_err(rejected)?;
        let new = parameter.snapshot();
        self.clear_tick_tree_snapshot();
        self.emit_param_events_for_state_change(node, &old.value, &new.value, source);
        self.emit_param_constraints_event(node, old.constraints, new.constraints);
        Ok(())
    }
}

impl<T: Node> Engine<T> {
    pub(super) fn queue_deferred_feedback(
        &mut self,
        feedback: std::collections::HashMap<NodeId, PendingParameterFeedback>,
    ) {
        let mut pending = feedback.into_iter().collect::<Vec<_>>();
        pending.sort_unstable_by_key(|(node, _)| node.0);
        for (
            node,
            PendingParameterFeedback {
                value,
                expected_input,
                guard,
                source,
            },
        ) in pending
        {
            let edit = crate::edit::Edit::SetParamFeedback {
                node,
                value,
                expected_input,
                guard,
            };
            if let Some(source) = source {
                self.edits.push_from(source, edit);
            } else {
                self.edits.push(edit);
            }
        }
    }
}
