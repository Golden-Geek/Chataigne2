use crate::events::EventKind;
use crate::node::{Node, NodeId, NodeMetaPatch};
use crate::parameter::ParamValue;

use super::engine_history::SetParamEffect;
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Applies a parameter value update and returns the captured before/after effect for history.
    pub(crate) fn apply_set_param(
        &mut self,
        edit_index: usize,
        node: NodeId,
        value: ParamValue,
    ) -> Result<SetParamEffect, EngineEditError> {
        const OP: &str = "SetParam";
        let new_value = value.clone();

        let old_value = {
            let target = self
                .nodes
                .get_mut(node)
                .ok_or(EngineEditError::NodeNotFound {
                    edit_index,
                    operation: OP,
                    node,
                })?;
            let node_type = target.get_type().to_string();
            match target.set_param_value(value) {
                Some(old) => old,
                None => {
                    return Err(EngineEditError::ParamEditTargetMismatch {
                        edit_index,
                        node,
                        node_type,
                    });
                }
            }
        };

        self.emit_event(EventKind::ParamChanged {
            param: node,
            old_value: old_value.clone(),
        });

        Ok(SetParamEffect {
            node,
            old_value,
            new_value,
        })
    }

    /// Validates a node target for metadata changes and emits the corresponding meta-changed event.
    pub(crate) fn apply_patch_meta(
        &mut self,
        edit_index: usize,
        node: NodeId,
        patch: NodeMetaPatch,
    ) -> Result<(), EngineEditError> {
        const OP: &str = "PatchMeta";

        if !self.nodes.contains(node) {
            return Err(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node,
            });
        }

        self.emit_event(EventKind::MetaChanged { node, patch });
        Ok(())
    }
}
