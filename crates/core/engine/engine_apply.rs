use std::any::type_name;

use crate::edit::Edit;
use crate::events::{Event, EventKind};
use crate::node::Node;

use super::engine_history::{HistoryStep, HistoryTransaction};
use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    /// Applies all pending edits in queue order and emits resulting events.
    ///
    /// # Examples
    /// ```rust
    /// use golden_core::engine::Engine;
    /// use golden_core::node::Container;
    ///
    /// let mut engine = Engine::new(Container::new("root".to_string()));
    /// engine.add_node(Container::new("child".to_string()), None);
    /// engine.apply_edits().expect("edit application should succeed");
    /// ```
    pub fn apply_edits(&mut self) -> Result<(), EngineEditError> {
        let mut transaction = HistoryTransaction::new();
        let mut applied_any_edit = false;

        for (edit_index, request) in self.edits.drain().into_iter().enumerate() {
            let outcome: Result<Option<HistoryStep<T>>, EngineEditError> = match request.edit {
                Edit::SetParam { node, value } => {
                    let effect = self.apply_set_param(edit_index, node, value)?;
                    Ok(Some(effect.into()))
                }
                Edit::AddNode { node, parent, prev_sibling } => {
                    let effect = self.apply_add_node(edit_index, node, parent, prev_sibling)?;
                    Ok(Some(effect.into()))
                }
                Edit::ReplaceNode { node, new_node } => {
                    let effect = self.apply_replace_node(edit_index, node, new_node)?;
                    Ok(Some(effect.into()))
                }
                Edit::RemoveNode { node } => {
                    let effect = self.apply_remove_node(edit_index, node)?;
                    Ok(Some(effect.into()))
                }
                Edit::MoveNode { node, new_parent, new_prev_sibling } => {
                    let effect = self.apply_move_node(edit_index, node, new_parent, new_prev_sibling)?;
                    Ok(Some(effect.into()))
                }
                Edit::PatchMeta { node, patch } => {
                    self.apply_patch_meta(edit_index, node, patch)?;
                    Ok(None)
                }
                Edit::EmitCustomEvent { event } => {
                    self.emit_event(EventKind::Custom(event));
                    Ok(None)
                }
                Edit::AddEventListener { subscriber, subscription } => {
                    self.apply_add_event_listener(edit_index, subscriber, subscription)?;
                    Ok(None)
                }
                Edit::RemoveEventListener { subscriber, subscription } => {
                    self.apply_remove_event_listener(subscriber, subscription);
                    Ok(None)
                }
            };

            match outcome {
                Ok(step) => {
                    applied_any_edit = true;
                    if let Some(step) = step {
                        transaction.push(step);
                    }
                }
                Err(err) => {
                    if applied_any_edit {
                        self.clear_redo_history();
                        self.push_undo_transaction(transaction);
                    }
                    return Err(err);
                }
            }
        }

        if applied_any_edit {
            self.clear_redo_history();
            self.push_undo_transaction(transaction);
        }

        Ok(())
    }

    /// Validates and downcasts a dynamically provided node to the engine node type `T`.
    ///
    /// Returns [`EngineEditError::NodeTypeMismatch`] when the node cannot be coerced.
    pub(crate) fn coerce_node_for_engine(&self, edit_index: usize, operation: &'static str, node: Box<dyn Node>) -> Result<T, EngineEditError> {
        let provided_node_type = node.get_type().to_string();
        T::from_boxed_node(node).ok_or(EngineEditError::NodeTypeMismatch {
            edit_index,
            operation,
            provided_node_type,
            expected_engine_node_type: type_name::<T>(),
        })
    }

    /// Pushes an event into the inbox and advances the per-tick event sequence counter.
    pub(crate) fn emit_event(&mut self, kind: EventKind) {
        if let EventKind::NodeDeleted { node } = &kind {
            self.purge_event_listeners_for_node(*node);
        }
        let time = self.time;
        self.inbox.push(Event { time, kind });
        self.time.seq = self.time.seq.saturating_add(1);
    }
}
