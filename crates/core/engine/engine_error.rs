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
    /// A `SetParam` edit value failed constraint normalization or validation.
    ParamConstraintViolation {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Target node id.
        node: NodeId,
        /// Runtime type name of the target node.
        node_type: String,
        /// Human-readable constraint failure message.
        message: String,
    },
    /// A `SetParamControlState` operation was rejected.
    ParamControlStateRejected {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Target node id.
        node: NodeId,
        /// Runtime type name of the target node.
        node_type: String,
        /// Human-readable rejection message.
        message: String,
    },
    /// A `SetScriptConfig` edit was rejected by the target node.
    ScriptConfigRejected {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Target node id.
        node: NodeId,
        /// Runtime type name of the target node.
        node_type: String,
        /// Human-readable rejection message.
        message: String,
    },
    /// A `SetNodeScriptProperty` edit was rejected by the target node.
    ScriptPropertyRejected {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Target node id.
        node: NodeId,
        /// Runtime type name of the target node.
        node_type: String,
        /// Human-readable rejection message.
        message: String,
    },
    /// A `CallNodeScriptMethod` edit was rejected by the target node.
    ScriptMethodRejected {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Target node id.
        node: NodeId,
        /// Runtime type name of the target node.
        node_type: String,
        /// Human-readable rejection message.
        message: String,
    },
    /// A runtime node-mutation callback was rejected by the target node.
    NodeMutationRejected {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Target node id.
        node: NodeId,
        /// Runtime type name of the target node.
        node_type: String,
        /// Human-readable rejection message.
        message: String,
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
    /// Attempted to start an edit session while one is already active.
    EditSessionAlreadyActive {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Requested session id.
        requested_client_edit_id: String,
        /// Currently active session id.
        active_client_edit_id: String,
    },
    /// Attempted to end an edit session while none is active.
    EditSessionNotActive {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Requested session id.
        requested_client_edit_id: String,
    },
    /// Attempted to end an edit session with a mismatched id.
    EditSessionIdMismatch {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Requested session id.
        requested_client_edit_id: String,
        /// Expected currently active id.
        active_client_edit_id: String,
    },
    /// A user-item operation targeted a location that is not inside a container.
    UserItemContainerRequired {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Parent targeted by the operation.
        parent: NodeId,
    },
    /// A container rejected an item kind during add/move validation.
    UserItemKindRejected {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Runtime node id of the container.
        container: NodeId,
        /// Runtime node type of the container.
        container_type: String,
        /// Runtime node type of the rejected item.
        item_type: String,
        /// Logical item kind requested by the item node.
        item_kind: String,
    },
    /// A requested user-item node type is not creatable by the target container.
    UserItemTypeUnavailable {
        /// Index of the edit in the drained queue.
        edit_index: usize,
        /// Operation name associated with this edit.
        operation: &'static str,
        /// Parent container id.
        parent: NodeId,
        /// Requested runtime node type.
        node_type: String,
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
                write!(f, "edit #{edit_index} ({operation}) carries node type '{provided_node_type}', expected engine node type {expected_engine_node_type}")
            }
            Self::ParamEditTargetMismatch { edit_index, node, node_type } => {
                write!(f, "edit #{edit_index} (SetParam) targets node {:?} of type '{node_type}', expected parameter node", node)
            }
            Self::ParamConstraintViolation { edit_index, node, node_type, message } => write!(f, "edit #{edit_index} (SetParam) rejected for node {:?} of type '{node_type}': {message}", node),
            Self::ParamControlStateRejected { edit_index, operation, node, node_type, message } => write!(f, "edit #{edit_index} ({operation}) rejected for node {:?} of type '{node_type}': {message}", node),
            Self::ScriptConfigRejected { edit_index, operation, node, node_type, message } => write!(f, "edit #{edit_index} ({operation}) rejected for node {:?} of type '{node_type}': {message}", node),
            Self::ScriptPropertyRejected { edit_index, operation, node, node_type, message } => write!(f, "edit #{edit_index} ({operation}) rejected for node {:?} of type '{node_type}': {message}", node),
            Self::ScriptMethodRejected { edit_index, operation, node, node_type, message } => write!(f, "edit #{edit_index} ({operation}) rejected for node {:?} of type '{node_type}': {message}", node),
            Self::NodeMutationRejected { edit_index, operation, node, node_type, message } => {
                write!(f, "edit #{edit_index} ({operation}) rejected for node {:?} of type '{node_type}': {message}", node)
            }
            Self::NodeNotFound { edit_index, operation, node } => write!(f, "edit #{edit_index} ({operation}) references missing node {:?}", node),
            Self::ParentNotFound { edit_index, operation, parent } => write!(f, "edit #{edit_index} ({operation}) references missing parent {:?}", parent),
            Self::SiblingNotFound { edit_index, operation, sibling } => write!(f, "edit #{edit_index} ({operation}) references missing sibling {:?}", sibling),
            Self::InvalidSiblingParent { edit_index, operation, parent, sibling, sibling_parent } => write!(f, "edit #{edit_index} ({operation}) uses sibling {:?} under parent {:?}, but sibling parent is {:?}", sibling, parent, sibling_parent),
            Self::InvalidSiblingReference { edit_index, operation, node, sibling } => write!(f, "edit #{edit_index} ({operation}) has invalid sibling reference: node {:?} cannot use itself as sibling {:?}", node, sibling),
            Self::CannotMutateRoot { edit_index, operation, node } => write!(f, "edit #{edit_index} ({operation}) cannot target root node {:?}", node),
            Self::CycleDetected { edit_index, operation, node, new_parent } => write!(f, "edit #{edit_index} ({operation}) would create a cycle by moving node {:?} under {:?}", node, new_parent),
            Self::EditSessionAlreadyActive {
                edit_index,
                requested_client_edit_id,
                active_client_edit_id,
            } => write!(f, "edit #{edit_index} (BeginEditSession) requested session '{requested_client_edit_id}' but '{active_client_edit_id}' is already active"),
            Self::EditSessionNotActive { edit_index, requested_client_edit_id } => write!(f, "edit #{edit_index} (EndEditSession) requested session '{requested_client_edit_id}' but no session is active"),
            Self::EditSessionIdMismatch {
                edit_index,
                requested_client_edit_id,
                active_client_edit_id,
            } => write!(f, "edit #{edit_index} (EndEditSession) requested session '{requested_client_edit_id}' but active session is '{active_client_edit_id}'"),
            Self::UserItemContainerRequired { edit_index, operation, parent } => {
                write!(f, "edit #{edit_index} ({operation}) requires a container target, but parent {:?} has no container in its ancestry", parent)
            }
            Self::UserItemKindRejected {
                edit_index,
                operation,
                container,
                container_type,
                item_type,
                item_kind,
            } => write!(f, "edit #{edit_index} ({operation}) rejected item type '{}' kind '{}' for container {:?} ('{}')", item_type, item_kind, container, container_type),
            Self::UserItemTypeUnavailable { edit_index, operation, parent, node_type } => write!(f, "edit #{edit_index} ({operation}) cannot create item type '{}' under parent {:?}", node_type, parent),
        }
    }
}

impl Error for EngineEditError {}
