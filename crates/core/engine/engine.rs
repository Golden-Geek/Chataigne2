use std::any::type_name;

use crate::edit::{Edit, EditQueue};
use crate::events::Inbox;
use crate::node::*;
use crate::process_ctx::ProcessCtx;
use serde::{Deserialize, Serialize};

/// Edit-application entry point and queue-drain transaction orchestration.
#[path = "engine_apply.rs"]
mod engine_apply;
/// Parameter and metadata edit application helpers.
#[path = "engine_apply_param.rs"]
mod engine_apply_param;
/// Tree mutation, attachment, and topology validation helpers.
#[path = "engine_apply_tree.rs"]
mod engine_apply_tree;
/// Engine edit error type definitions.
#[path = "engine_error.rs"]
mod engine_error;
/// Undo/redo history transaction and effect models.
#[path = "engine_history.rs"]
mod engine_history;
#[cfg(test)]
#[path = "engine_tests.rs"]
mod engine_tests;

/// Node storage implementation used by the engine.
pub mod node_store;
use node_store::NodeStore;

/// Error type returned when validating or applying edits.
pub use engine_error::EngineEditError;

/// Logical time tracked by the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EngineTime {
    /// Monotonic engine tick counter. Increments only on EngineTick.
    pub tick: u64,

    /// Micro-step index within the same tick.
    /// 0 = main tick pass, 1.. = stabilisation rounds or flushImmediate rounds within that same tick.
    pub micro: u32,

    /// Total ordering within the same (tick, micro).
    pub seq: u32,
}

/// Node engine storing graph state, pending edits, and emitted events.
pub struct Engine<T: Node> {
    /// Backing node store indexed by stable node ids.
    pub nodes: NodeStore<T>,
    /// Root node id.
    pub root: NodeId,
    /// Current engine logical time.
    pub time: EngineTime,
    /// Engine-owned event stream.
    pub inbox: Inbox,
    /// Pending edits to be applied.
    pub edits: EditQueue,
    /// Applied edit transactions available for undo.
    undo_stack: Vec<engine_history::HistoryTransaction<T>>,
    /// Undone edit transactions available for redo.
    redo_stack: Vec<engine_history::HistoryTransaction<T>>,
}

impl<T: Node> Engine<T> {
    /// Creates a new engine with `root` as the graph root node.
    pub fn new(root: T) -> Self {
        let mut nodes: NodeStore<T> = NodeStore::new();
        let root = nodes.insert(root);

        Self {
            nodes,
            root,
            time: EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
            inbox: Inbox::new(),
            edits: EditQueue::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Queues insertion of a node under `parent` (or root when `None`).
    pub fn add_node(&mut self, node: T, parent: Option<NodeId>) {
        self.edits.push(Edit::AddNode {
            parent: parent.unwrap_or(self.root),
            node: Box::new(node),
            prev_sibling: None,
        });
    }

    /// Queues insertion of a node after an existing sibling.
    pub fn add_node_after(&mut self, node: T, sibling: NodeId) {
        let parent = self
            .nodes
            .get(sibling)
            .and_then(|n| n.node_data().parent)
            .unwrap_or(self.root);
        self.edits.push(Edit::AddNode {
            parent,
            prev_sibling: Some(sibling),
            node: Box::new(node),
        });
    }

    /// Queues replacement of an existing node.
    pub fn replace_node(&mut self, node: NodeId, new_node: T) {
        self.edits.push(Edit::ReplaceNode {
            node,
            new_node: Box::new(new_node),
        });
    }

    /// Moves edits from a processing context into the engine queue.
    ///
    /// Node-bearing edits are validated to ensure node types match `T`.
    pub fn absorb_edits(&mut self, ctx: &mut ProcessCtx) -> Result<(), EngineEditError> {
        let mut validated_edits = Vec::new();

        for (edit_index, request) in ctx.edits.drain().into_iter().enumerate() {
            match request.edit {
                Edit::AddNode {
                    node,
                    parent,
                    prev_sibling,
                } => {
                    let provided_node_type = node.get_type().to_string();
                    let Some(node) = T::from_boxed_node(node) else {
                        return Err(EngineEditError::NodeTypeMismatch {
                            edit_index,
                            operation: "AddNode",
                            provided_node_type,
                            expected_engine_node_type: type_name::<T>(),
                        });
                    };

                    validated_edits.push(Edit::AddNode {
                        node: Box::new(node),
                        parent,
                        prev_sibling,
                    });
                }
                Edit::ReplaceNode { node, new_node } => {
                    let provided_node_type = new_node.get_type().to_string();
                    let Some(new_node) = T::from_boxed_node(new_node) else {
                        return Err(EngineEditError::NodeTypeMismatch {
                            edit_index,
                            operation: "ReplaceNode",
                            provided_node_type,
                            expected_engine_node_type: type_name::<T>(),
                        });
                    };

                    validated_edits.push(Edit::ReplaceNode {
                        node,
                        new_node: Box::new(new_node),
                    });
                }
                edit => validated_edits.push(edit),
            }
        }

        for edit in validated_edits {
            self.edits.push(edit);
        }

        Ok(())
    }
}
