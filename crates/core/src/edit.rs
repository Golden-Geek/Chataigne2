use crate::events::CustomEvent;
use crate::node::{EventSubscription, Node, NodeId, NodeMetaPatch, NodeWarning};
use crate::parameter::{ParamValue, ParameterConstraints, ParameterEventBehaviour};
use crate::process_ctx::ProcessCtx;
use crate::script::ScriptNodeConfig;
use serde::{Deserialize, Serialize};

/// Deferred mutable node callback executed during edit application.
pub type NodeMutation = Box<dyn FnOnce(&mut dyn Node, &mut ProcessCtx) -> Result<(), String> + Send>;

/// Detached node subtree that can be inserted into the engine as one structural edit.
pub struct NodeTree {
    /// Root node for this subtree.
    pub node: Box<dyn Node>,
    /// Ordered child subtrees.
    pub children: Vec<NodeTree>,
}

impl NodeTree {
    /// Creates a node subtree from a typed root node.
    pub fn new<N: Node + 'static>(node: N) -> Self {
        Self {
            node: Box::new(node),
            children: Vec::new(),
        }
    }

    /// Creates a node subtree from a boxed root node.
    pub fn boxed(node: Box<dyn Node>) -> Self {
        Self {
            node,
            children: Vec::new(),
        }
    }

    /// Appends one child subtree.
    pub fn push_child(&mut self, child: NodeTree) {
        self.children.push(child);
    }

    /// Appends one child subtree and returns the updated tree.
    pub fn with_child(mut self, child: NodeTree) -> Self {
        self.push_child(child);
        self
    }

    /// Returns the runtime node type of the subtree root.
    pub fn node_type(&self) -> &str {
        self.node.get_type()
    }
}

/// Origin of an edit session boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditOrigin {
    /// Internal/runtime initiated edits.
    Runtime,
    /// UI/client initiated edits.
    Ui,
}

/// Mutable operations queued and then applied by the engine.
pub enum Edit {
    /// Begins a multi-intent edit session used as one undo/redo boundary.
    BeginEditSession {
        /// Source of the session request.
        origin: EditOrigin,
        /// Optional user-facing label describing the interaction.
        label: Option<String>,
        /// Client-provided id used to match begin/end.
        client_edit_id: String,
        /// Stable UI client instance id when the session originates from a browser client.
        ui_client_instance_id: Option<String>,
    },
    /// Ends a previously opened edit session.
    EndEditSession {
        /// Client-provided id expected to match the active session.
        client_edit_id: String,
    },
    /// Set a parameter value on an existing node.
    SetParam {
        /// Target node id.
        node: NodeId,
        /// New parameter value.
        value: ParamValue,
        /// Coalescing strategy requested by the parameter/node API.
        behaviour: ParameterEventBehaviour,
    },
    /// Replace the live constraints on an existing parameter node.
    SetParamConstraints {
        /// Target node id.
        node: NodeId,
        /// New runtime constraints.
        constraints: ParameterConstraints,
    },
    /// Sets one script-exposed property on a node.
    SetNodeScriptProperty {
        /// Target node id.
        node: NodeId,
        /// Property key as seen from scripts.
        property: String,
        /// Incoming script value converted to `ParamValue`.
        value: ParamValue,
    },
    /// Invokes one script-exposed method on a node.
    CallNodeScriptMethod {
        /// Target node id.
        node: NodeId,
        /// Method name as seen from scripts.
        method: String,
        /// Method arguments converted to `ParamValue`.
        args: Vec<ParamValue>,
    },
    /// Invokes one runtime Rust callback on a node.
    ///
    /// This is intended for strongly typed node-handle APIs that need to call
    /// methods on existing child nodes while preserving the edit pipeline.
    CallNodeMutation {
        /// Target node id.
        node: NodeId,
        /// Deferred typed callback to run against the target node.
        callback: NodeMutation,
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
    /// Insert a detached node subtree under `parent`, optionally after a sibling.
    AddNodeTree {
        /// Detached subtree to insert.
        tree: NodeTree,
        /// Parent receiving the subtree root.
        parent: NodeId,
        /// Optional sibling after which insertion occurs.
        prev_sibling: Option<NodeId>,
    },
    /// Insert a user-curated item node under `parent`, optionally after a sibling.
    AddUserItem {
        /// Node instance to insert.
        node: Box<dyn Node>,
        /// Parent receiving the node.
        parent: NodeId,
        /// Optional sibling after which insertion occurs.
        prev_sibling: Option<NodeId>,
    },
    /// Insert a user-curated blueprint instance under `parent`, optionally after a sibling.
    ///
    /// The actual runtime node is instantiated by the engine blueprint registry.
    CreateBlueprintInstance {
        /// Blueprint declaration id.
        blueprint_id: String,
        /// Parent receiving the instance root.
        parent: NodeId,
        /// Optional sibling after which insertion occurs.
        prev_sibling: Option<NodeId>,
        /// Optional explicit root label.
        label: Option<String>,
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
    /// Replace script configuration on a script-capable node.
    SetScriptConfig {
        /// Target node id.
        node: NodeId,
        /// New script configuration payload.
        config: ScriptNodeConfig,
        /// Force runtime reload even when config content is unchanged.
        force_reload: bool,
    },
    /// Sets or replaces one node warning by warning id.
    SetNodeWarning {
        /// Node receiving the warning.
        node: NodeId,
        /// Warning payload.
        warning: NodeWarning,
    },
    /// Clears one node warning by warning id, or all warnings when id is omitted.
    ClearNodeWarning {
        /// Node whose warnings are updated.
        node: NodeId,
        /// Warning id to clear. `None` clears all warnings.
        warning_id: Option<String>,
    },
    /// Sets how many descendant levels should be included when surfacing warnings.
    SetNodeChildWarningDepth {
        /// Node whose warning surfacing behavior is updated.
        node: NodeId,
        /// Maximum descendant depth shown by UI.
        max_depth: u32,
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
            Edit::RemoveNode { node } => {
                if !self.pending.iter().any(|request| {
                    matches!(
                        &request.edit,
                        Edit::RemoveNode { node: existing_node } if *existing_node == node
                    )
                }) {
                    self.pending.push(EditRequest {
                        edit: Edit::RemoveNode { node },
                    });
                }
            }
            edit => self.pending.push(EditRequest { edit }),
        }
    }

    /// Drains all edits and leaves the queue empty.
    pub fn drain(&mut self) -> Vec<EditRequest> {
        std::mem::take(&mut self.pending)
    }
}
