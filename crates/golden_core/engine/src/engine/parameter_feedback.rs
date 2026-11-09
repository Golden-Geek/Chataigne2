//! Runtime scalar values share normal reads/events while authored snapshots retain intent.
use super::{Engine, EngineEditError};
use crate::{
    node::{Node, NodeId},
    parameter::{ParamValue, Parameter},
};

impl<T: Node> Engine<T> {
    pub(super) fn apply_param_feedback(
        &mut self,
        edit_index: usize,
        node: NodeId,
        value: Option<ParamValue>,
        source: Option<NodeId>,
    ) -> Result<(), EngineEditError> {
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
        if !parameter.read_only {
            return Err(EngineEditError::ParamConstraintViolation {
                edit_index,
                node,
                node_type,
                message: "runtime status feedback requires a read-only Parameter".into(),
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
        let value = value
            .map(|value| parameter.engine_prepare_param_value(value))
            .transpose()
            .map_err(|message| EngineEditError::ParamConstraintViolation {
                edit_index,
                node,
                node_type,
                message,
            })?;
        let old = parameter.get().clone();
        parameter.set_runtime_value(value);
        let new = parameter.get().clone();
        self.clear_tick_tree_snapshot();
        self.emit_param_events_for_state_change(node, &old, &new, source);
        Ok(())
    }
}
