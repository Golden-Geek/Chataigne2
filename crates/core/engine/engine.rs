use std::any::type_name;

use crate::edit::{Edit, EditQueue};
use crate::events::Inbox;
use crate::node::*;
use crate::process_ctx::ProcessCtx;
use serde::{Deserialize, Serialize};

#[path = "engine_apply.rs"]
mod engine_apply;
#[path = "engine_apply_param.rs"]
mod engine_apply_param;
#[path = "engine_apply_tree.rs"]
mod engine_apply_tree;
#[path = "engine_error.rs"]
mod engine_error;
#[cfg(test)]
#[path = "engine_tests.rs"]
mod engine_tests;

pub mod node_store;
use node_store::NodeStore;

pub use engine_error::EngineEditError;

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

pub struct Engine<T: Node> {
    pub nodes: NodeStore<T>,
    pub root: NodeId,
    pub time: EngineTime,
    pub inbox: Inbox,
    pub edits: EditQueue,
}

impl<T: Node> Engine<T> {
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
        }
    }

    pub fn add_node(&mut self, node: T, parent: Option<NodeId>) {
        self.edits.push(Edit::AddNode {
            parent: parent.unwrap_or(self.root),
            node: Box::new(node),
            prev_sibling: None,
        });
    }

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

    pub fn replace_node(&mut self, node: NodeId, new_node: T) {
        self.edits.push(Edit::ReplaceNode {
            node,
            new_node: Box::new(new_node),
        });
    }

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
