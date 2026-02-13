use std::error::Error;
use std::fmt;

use crate::node::NodeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEditError {
    NodeTypeMismatch {
        edit_index: usize,
        operation: &'static str,
        provided_node_type: String,
        expected_engine_node_type: &'static str,
    },
    ParamEditTargetMismatch {
        edit_index: usize,
        node: NodeId,
        node_type: String,
    },
    NodeNotFound {
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
    },
    ParentNotFound {
        edit_index: usize,
        operation: &'static str,
        parent: NodeId,
    },
    SiblingNotFound {
        edit_index: usize,
        operation: &'static str,
        sibling: NodeId,
    },
    InvalidSiblingParent {
        edit_index: usize,
        operation: &'static str,
        parent: NodeId,
        sibling: NodeId,
        sibling_parent: Option<NodeId>,
    },
    InvalidSiblingReference {
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
        sibling: NodeId,
    },
    CannotMutateRoot {
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
    },
    CycleDetected {
        edit_index: usize,
        operation: &'static str,
        node: NodeId,
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
