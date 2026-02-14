use crate::events::EventKind;
use crate::node::{Node, NodeId, NodeMetaPatch};
use crate::parameter::{ParamValue, ParameterEventBehaviour};

use super::engine_history::SetParamEffect;
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Applies a parameter value update and returns the captured before/after effect for history.
    pub(crate) fn apply_set_param(&mut self, edit_index: usize, node: NodeId, value: ParamValue) -> Result<SetParamEffect, EngineEditError> {
        const OP: &str = "SetParam";
        let (old_value, new_value) = {
            let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?;
            let node_type = target.get_type().to_string();
            let prepared_value = target.engine_prepare_param_value(value).map_err(|message| EngineEditError::ParamConstraintViolation {
                edit_index,
                node,
                node_type: node_type.clone(),
                message,
            })?;
            let new_value = prepared_value.clone();

            match target.engine_set_param_value(prepared_value) {
                Some(old) => (old, new_value),
                None => {
                    return Err(EngineEditError::ParamEditTargetMismatch { edit_index, node, node_type });
                }
            }
        };

        self.emit_event(EventKind::ParamChanged {
            param: node,
            old_value: old_value.clone(),
            new_value: new_value.clone(),
        });

        Ok(SetParamEffect {
            node,
            old_value,
            new_value,
            behaviour: ParameterEventBehaviour::Append,
            tick: self.time.tick,
        })
    }

    /// Validates a node target for metadata changes and emits the corresponding meta-changed event.
    pub(crate) fn apply_patch_meta(&mut self, edit_index: usize, node: NodeId, patch: NodeMetaPatch) -> Result<(), EngineEditError> {
        const OP: &str = "PatchMeta";

        let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?;
        patch.apply_to(&mut target.node_data_mut().meta);

        self.emit_event(EventKind::MetaChanged { node, patch });
        Ok(())
    }
}
