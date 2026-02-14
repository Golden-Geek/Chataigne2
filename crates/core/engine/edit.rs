use crate::events::CustomEvent;
use crate::node::{EventSubscription, Node, NodeId, NodeMetaPatch};
use crate::parameter::{ParamValue, ParameterEventBehaviour};

/// Mutable operations queued and then applied by the engine.
pub enum Edit {
    /// Set a parameter value on an existing node.
    SetParam {
        /// Target node id.
        node: NodeId,
        /// New parameter value.
        value: ParamValue,
        /// Coalescing strategy requested by the parameter/node API.
        behaviour: ParameterEventBehaviour,
    },
    /// Insert a node under `parent`, optionally after a sibling.
    AddNode {
        /// Node instance to insert.
        node: Box<dyn Node>,
        /// Parent receiving the node.
        parent: NodeId,
        /// Optional sibling after which insertion occurs.
        prev_sibling: Option<NodeId>,
    },
    /// Replace an existing node with a new node value.
    ReplaceNode {
        /// Existing node id to replace.
        node: NodeId,
        /// Replacement node instance.
        new_node: Box<dyn Node>,
    },
    /// Remove a node (and its subtree).
    RemoveNode {
        /// Node id to remove.
        node: NodeId,
    },
    /// Move a node under a new parent.
    MoveNode {
        /// Node id to move.
        node: NodeId,
        /// Destination parent id.
        new_parent: NodeId,
        /// Optional sibling after which insertion occurs.
        new_prev_sibling: Option<NodeId>,
    },
    /// Apply a metadata patch to a node.
    PatchMeta {
        /// Node receiving the metadata patch.
        node: NodeId,
        /// Patch payload.
        patch: NodeMetaPatch,
    },
    /// Emit a custom event through the same edit pipeline.
    EmitCustomEvent {
        /// Custom event payload to emit.
        event: CustomEvent,
    },
    /// Requests a full scheduler reevaluation on the next runtime resolve pass.
    ReevaluateGraph,
    /// Adds or updates a runtime event listener for a subscriber node.
    AddEventListener {
        /// Listener owner node id.
        subscriber: NodeId,
        /// Subscription target and depth scope.
        subscription: EventSubscription,
    },
    /// Removes a runtime event listener for a subscriber node.
    RemoveEventListener {
        /// Listener owner node id.
        subscriber: NodeId,
        /// Subscription target and depth scope.
        subscription: EventSubscription,
    },
}

/// Wrapper for a queued edit entry.
pub struct EditRequest {
    /// Edit operation to be applied.
    pub edit: Edit,
}

/// FIFO queue of pending edit requests.
#[derive(Default)]
pub struct EditQueue {
    /// Pending edits in insertion order.
    pub pending: Vec<EditRequest>,
}

impl EditQueue {
    /// Creates an empty queue.
    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    /// Pushes a new edit at the back of the queue.
    pub fn push(&mut self, edit: Edit) {
        match edit {
            Edit::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Coalesce,
            } => {
                // Keep only the latest coalescable set for a given parameter id in this queue.
                self.pending.retain(|request| {
                    !matches!(
                        &request.edit,
                        Edit::SetParam {
                            node: existing_node,
                            behaviour: ParameterEventBehaviour::Coalesce,
                            ..
                        } if *existing_node == node
                    )
                });

                self.pending.push(EditRequest {
                    edit: Edit::SetParam {
                        node,
                        value,
                        behaviour: ParameterEventBehaviour::Coalesce,
                    },
                });
            }
            edit => self.pending.push(EditRequest { edit }),
        }
    }

    /// Drains all edits and leaves the queue empty.
    pub fn drain(&mut self) -> Vec<EditRequest> {
        std::mem::take(&mut self.pending)
    }
}
