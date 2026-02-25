use crate::events::EventKind;
use crate::node::{Node, NodeId, NodeMetaPatch, NodeWarning};
use crate::parameter::{ParamValue, ParameterChangeCheck, ParameterEventBehaviour};

use super::engine_history::{PatchMetaEffect, SetParamEffect};
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Applies a parameter value update and returns the captured before/after effect for history.
    pub(crate) fn apply_set_param(&mut self, edit_index: usize, node: NodeId, value: ParamValue) -> Result<Option<SetParamEffect>, EngineEditError> {
        const OP: &str = "SetParam";
        let node_type_hint = self.nodes.get(node).map(|target| target.get_type().to_string()).unwrap_or_else(|| "unknown".to_string());
        let value = if let ParamValue::Reference(reference) = value {
            let normalized = self.normalize_reference_value_for_param(node, reference).map_err(|message| EngineEditError::ParamConstraintViolation {
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
            let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?;
            let node_type = target.get_type().to_string();
            let prepared_value = target.engine_prepare_param_value(value).map_err(|message| EngineEditError::ParamConstraintViolation {
                edit_index,
                node,
                node_type: node_type.clone(),
                message,
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
                    return Err(EngineEditError::ParamEditTargetMismatch { edit_index, node, node_type });
                }
            }
        };

        let should_emit = match change_check {
            Some(ParameterChangeCheck::None) => true,
            Some(ParameterChangeCheck::ValueChange) => matches!(&new_value, ParamValue::Trigger()) || old_value != new_value,
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

        Ok(Some(SetParamEffect {
            node,
            old_value,
            new_value,
            behaviour: ParameterEventBehaviour::Append,
            tick: self.time.tick,
        }))
    }

    /// Validates a node target for metadata changes and emits the corresponding meta-changed event.
    pub(crate) fn apply_patch_meta(&mut self, edit_index: usize, node: NodeId, patch: NodeMetaPatch) -> Result<PatchMetaEffect, EngineEditError> {
        const OP: &str = "PatchMeta";

        let target = self.nodes.get_mut(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?;
        let old_meta = target.node_data().meta.clone();
        patch.apply_to(&mut target.node_data_mut().meta);
        let new_meta = target.node_data().meta.clone();

        eprintln!("[gc-engine] apply_patch_meta node={:?} label='{}' enabled={} -> '{}'/{}", node, old_meta.label, old_meta.enabled, new_meta.label, new_meta.enabled);

        if old_meta.enabled != new_meta.enabled {
            self.mark_schedule_dirty();
        }

        self.emit_event(EventKind::MetaChanged { node, patch });
        Ok(PatchMetaEffect { node, old_meta, new_meta })
    }

    /// Sets/replaces one warning by id and emits `MetaChanged` when runtime metadata changed.
    pub(crate) fn apply_set_node_warning(&mut self, edit_index: usize, node: NodeId, warning: NodeWarning) -> Result<Option<PatchMetaEffect>, EngineEditError> {
        const OP: &str = "SetNodeWarning";

        let current_presentation = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?.node_data().meta.presentation.clone();

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
    pub(crate) fn apply_clear_node_warning(&mut self, edit_index: usize, node: NodeId, warning_id: Option<String>) -> Result<Option<PatchMetaEffect>, EngineEditError> {
        const OP: &str = "ClearNodeWarning";

        let current_presentation = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?.node_data().meta.presentation.clone();

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
    pub(crate) fn apply_set_node_child_warning_depth(&mut self, edit_index: usize, node: NodeId, max_depth: u32) -> Result<Option<PatchMetaEffect>, EngineEditError> {
        const OP: &str = "SetNodeChildWarningDepth";

        let current_presentation = self.nodes.get(node).ok_or(EngineEditError::NodeNotFound { edit_index, operation: OP, node })?.node_data().meta.presentation.clone();

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
}
