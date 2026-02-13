use std::error::Error;
use std::fmt;

use crate::node::NodeId;

/// Error returned when edit validation or application fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEditError {
    /// A node-carrying edit provided a node of the wrong runtime type.
    NodeTypeMismatch {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Runtime type name reported by the provided node.
        provided_node_type: String,
        /// Runtime type expected by the engine (`T`).
        expected_engine_node_type: &'static str,
    },
    /// A `SetParam` edit targeted a node that is not a parameter node.
    ParamEditTargetMismatch {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Target node id.
        node: NodeId,
        /// Runtime type name of the target node.
        node_type: String,
    },
    /// A node id referenced by an edit was not found.
    NodeNotFound {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Missing node id.
        node: NodeId,
    },
    /// A referenced parent id does not exist.
    ParentNotFound {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Missing parent id.
        parent: NodeId,
    },
    /// A referenced sibling id does not exist.
    SiblingNotFound {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Missing sibling id.
        sibling: NodeId,
    },
    /// The sibling used for insertion is not under the expected parent.
    InvalidSiblingParent {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Expected parent id.
        parent: NodeId,
        /// Sibling id provided by the edit.
        sibling: NodeId,
        /// Actual parent of `sibling` when known.
        sibling_parent: Option<NodeId>,
    },
    /// A sibling reference is structurally invalid (for example node == sibling).
    InvalidSiblingReference {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Node being modified.
        node: NodeId,
        /// Invalid sibling id.
        sibling: NodeId,
    },
    /// The requested operation cannot be applied to the root node.
    CannotMutateRoot {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Root node id that was targeted.
        node: NodeId,
    },
    /// A move operation would introduce a cycle in the tree.
    CycleDetected {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Node being moved.
        node: NodeId,
        /// Destination parent that would create a cycle.
        new_parent: NodeId,
    },
}

impl fmt::Display for EngineEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeTypeMismatch {
                edit_index,
                operation,
                provided_node_type,
                expected_engine_node_type,
            } => {
                write!(
                    f,
                    "edit #{edit_index} ({operation}) carries node type '{provided_node_type}', expected engine node type {expected_engine_node_type}"
                )
            }
            Self::ParamEditTargetMismatch {
                edit_index,
                node,
                node_type,
            } => {
                write!(
                    f,
                    "edit #{edit_index} (SetParam) targets node {:?} of type '{node_type}', expected parameter node",
                    node
                )
            }
            Self::NodeNotFound {
                edit_index,
                operation,
                node,
            } => write!(
                f,
                "edit #{edit_index} ({operation}) references missing node {:?}",
                node
            ),
            Self::ParentNotFound {
                edit_index,
                operation,
                parent,
            } => write!(
                f,
                "edit #{edit_index} ({operation}) references missing parent {:?}",
                parent
            ),
            Self::SiblingNotFound {
                edit_index,
                operation,
                sibling,
            } => write!(
                f,
                "edit #{edit_index} ({operation}) references missing sibling {:?}",
                sibling
            ),
            Self::InvalidSiblingParent {
                edit_index,
                operation,
                parent,
                sibling,
                sibling_parent,
            } => write!(
                f,
                "edit #{edit_index} ({operation}) uses sibling {:?} under parent {:?}, but sibling parent is {:?}",
                sibling, parent, sibling_parent
            ),
            Self::InvalidSiblingReference {
                edit_index,
                operation,
                node,
                sibling,
            } => write!(
                f,
                "edit #{edit_index} ({operation}) has invalid sibling reference: node {:?} cannot use itself as sibling {:?}",
                node, sibling
            ),
            Self::CannotMutateRoot {
                edit_index,
                operation,
                node,
            } => write!(
                f,
                "edit #{edit_index} ({operation}) cannot target root node {:?}",
                node
            ),
            Self::CycleDetected {
                edit_index,
                operation,
                node,
                new_parent,
            } => write!(
                f,
                "edit #{edit_index} ({operation}) would create a cycle by moving node {:?} under {:?}",
                node, new_parent
            ),
        }
    }
}

impl Error for EngineEditError {}
