use std::any::type_name;

use crate::edit::Edit;
use crate::events::{Event, EventKind};
use crate::node::Node;

use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    pub fn apply_edits(&mut self) -> Result<(), EngineEditError> {
        for (edit_index, request) in self.edits.drain().into_iter().enumerate() {
            match request.edit {
                Edit::SetParam { node, value } => self.apply_set_param(edit_index, node, value)?,
                Edit::AddNode {
                    node,
                    parent,
                    prev_sibling,
                } => self.apply_add_node(edit_index, node, parent, prev_sibling)?,
                Edit::ReplaceNode { node, new_node } => {
                    self.apply_replace_node(edit_index, node, new_node)?
                }
                Edit::RemoveNode { node } => self.apply_remove_node(edit_index, node)?,
                Edit::MoveNode {
                    node,
                    new_parent,
                    new_prev_sibling,
                } => self.apply_move_node(edit_index, node, new_parent, new_prev_sibling)?,
                Edit::PatchMeta { node, patch } => self.apply_patch_meta(edit_index, node, patch)?,
                Edit::EmitCustomEvent { event } => self.emit_event(EventKind::Custom(event)),
            }
        }

        Ok(())
    }

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

    pub(crate) fn emit_event(&mut self, kind: EventKind) {
        let time = self.time;
        self.inbox.push(Event { time, kind });
        self.time.seq = self.time.seq.saturating_add(1);
    }
}
