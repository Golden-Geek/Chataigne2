use std::time::Duration;

use crate::edit::{Edit, EditOrigin, EditQueue};
use crate::engine::EngineTime;
use crate::events::{CustomEvent, Event};
use crate::node::{EventSubscription, Node, NodeId, NodeMetaPatch, NodeWarning};
use crate::parameter::{ParamValue, ParameterEventBehaviour};
use serde::Serialize;

/// Runtime phase of the current processing pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// Main engine tick.
    EngineTick,
    /// Additional stabilization pass after the main tick.
    EndOfTickStabilization,
}

/// Mutable context passed to node callbacks during processing.
///
/// # Examples
/// ```rust
/// use golden_core::engine::EngineTime;
/// use golden_core::process_ctx::{ExecutionPhase, ProcessCtx};
///
/// let time = EngineTime { tick: 1, micro: 0, seq: 0 };
/// let ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
/// assert!(ctx.events.is_empty());
/// ```
pub struct ProcessCtx {
    /// Phase currently being processed.
    pub phase: ExecutionPhase,
    /// Events visible to nodes for the current callback.
    pub events: Vec<Event>,
    /// Edits requested by nodes during callbacks.
    pub edits: EditQueue,
    /// Current engine timestamp for emitted operations.
    pub time: EngineTime,
    /// Time elapsed since this node's previous update callback.
    ///
    /// This value is meaningful in `Node::update`.
    pub delta_time: Duration,
}

impl ProcessCtx {
    /// Creates a processing context for a given phase and time.
    pub fn new(phase: ExecutionPhase, time: EngineTime) -> Self {
        Self {
            phase,
            events: Vec::new(),
            edits: EditQueue::new(),
            time,
            delta_time: Duration::ZERO,
        }
    }

    /// Queues a parameter update edit.
    pub fn set_param(&mut self, node: NodeId, value: ParamValue) {
        self.set_param_with_behaviour(node, value, ParameterEventBehaviour::Coalesce);
    }

    /// Begins an edit session for grouping subsequent intents into one undo boundary.
    pub fn begin_edit_session(&mut self, origin: EditOrigin, client_edit_id: impl Into<String>, label: Option<String>) {
        self.edits.push(Edit::BeginEditSession { origin, label, client_edit_id: client_edit_id.into() });
    }

    /// Ends an edit session previously opened with [`Self::begin_edit_session`].
    pub fn end_edit_session(&mut self, client_edit_id: impl Into<String>) {
        self.edits.push(Edit::EndEditSession { client_edit_id: client_edit_id.into() });
    }

    /// Queues a parameter update edit with an explicit coalescing strategy.
    pub fn set_param_with_behaviour(&mut self, node: NodeId, value: ParamValue, behaviour: ParameterEventBehaviour) {
        self.edits.push(Edit::SetParam { node, value, behaviour });
    }

    /// Sets or replaces the default warning on `node`.
    pub fn set_node_warning(&mut self, node: NodeId, message: impl Into<String>) {
        self.set_node_warning_with(node, None, message, None);
    }

    /// Sets or replaces one warning by id on `node`.
    ///
    /// `warning_id = None` uses the default empty warning id.
    pub fn set_node_warning_with(&mut self, node: NodeId, warning_id: Option<&str>, message: impl Into<String>, detail: Option<&str>) {
        self.edits.push(Edit::SetNodeWarning {
            node,
            warning: NodeWarning {
                id: warning_id.unwrap_or_default().to_string(),
                message: message.into(),
                detail: detail.map(str::to_string),
            },
        });
    }

    /// Clears one warning by id on `node`.
    ///
    /// `warning_id = None` clears all warnings on `node`.
    pub fn clear_node_warning(&mut self, node: NodeId, warning_id: Option<&str>) {
        self.edits.push(Edit::ClearNodeWarning { node, warning_id: warning_id.map(str::to_string) });
    }

    /// Clears all warnings on `node`.
    pub fn clear_all_node_warnings(&mut self, node: NodeId) {
        self.clear_node_warning(node, None);
    }

    /// Sets warning surfacing depth for descendant warnings on `node`.
    pub fn set_node_child_warning_depth(&mut self, node: NodeId, max_depth: u32) {
        self.edits.push(Edit::SetNodeChildWarningDepth { node, max_depth });
    }

    /// Queues a metadata patch for `node`.
    pub fn patch_node_meta(&mut self, node: NodeId, patch: NodeMetaPatch) {
        self.edits.push(Edit::PatchMeta { node, patch });
    }

    /// Queues insertion of a child node.
    pub fn add_child<N: Node + 'static>(&mut self, parent: NodeId, child: N, after: Option<NodeId>) {
        self.add_child_boxed(parent, Box::new(child), after);
    }

    /// Queues insertion of a boxed child node.
    pub fn add_child_boxed(&mut self, parent: NodeId, child: Box<dyn Node>, after: Option<NodeId>) {
        self.edits.push(Edit::AddNode { parent, prev_sibling: after, node: child });
    }

    /// Queues insertion of a typed user-curated item node.
    pub fn add_user_item<N: Node + 'static>(&mut self, parent: NodeId, child: N, after: Option<NodeId>) {
        self.add_user_item_boxed(parent, Box::new(child), after);
    }

    /// Queues insertion of a boxed user-curated item node.
    pub fn add_user_item_boxed(&mut self, parent: NodeId, child: Box<dyn Node>, after: Option<NodeId>) {
        self.edits.push(Edit::AddUserItem { parent, prev_sibling: after, node: child });
    }

    /// Queues replacement of a node by a typed node value.
    pub fn replace_node<N: Node + 'static>(&mut self, node: NodeId, new_node: N) {
        self.replace_node_boxed(node, Box::new(new_node));
    }

    /// Queues replacement of a node by a boxed node value.
    pub fn replace_node_boxed(&mut self, node: NodeId, new_node: Box<dyn Node>) {
        self.edits.push(Edit::ReplaceNode { node, new_node });
    }

    /// Queues a custom event to be emitted by the engine.
    pub fn emit_custom_event(&mut self, event: CustomEvent) {
        self.edits.push(Edit::EmitCustomEvent { event });
    }

    /// Requests a full graph execution-rule reevaluation.
    pub fn reevaluate_graph(&mut self) {
        self.edits.push(Edit::ReevaluateGraph);
    }

    /// Queues a direct listener subscription from `subscriber` to `target`.
    pub fn add_event_listener(&mut self, subscriber: NodeId, target: NodeId) {
        self.add_event_listener_subtree(subscriber, target, 0);
    }

    /// Queues a listener subscription from `subscriber` to `target` subtree.
    pub fn add_event_listener_subtree(&mut self, subscriber: NodeId, target: NodeId, max_depth: u32) {
        self.edits.push(Edit::AddEventListener {
            subscriber,
            subscription: EventSubscription::subtree(target, max_depth),
        });
    }

    /// Queues removal of a direct listener subscription from `subscriber` to `target`.
    pub fn remove_event_listener(&mut self, subscriber: NodeId, target: NodeId) {
        self.remove_event_listener_subtree(subscriber, target, 0);
    }

    /// Queues removal of a subtree listener subscription from `subscriber` to `target`.
    pub fn remove_event_listener_subtree(&mut self, subscriber: NodeId, target: NodeId, max_depth: u32) {
        self.edits.push(Edit::RemoveEventListener {
            subscriber,
            subscription: EventSubscription::subtree(target, max_depth),
        });
    }

    /// Serializes and queues a custom event payload.
    ///
    /// # Examples
    /// ```rust
    /// use golden_core::engine::EngineTime;
    /// use golden_core::process_ctx::{ExecutionPhase, ProcessCtx};
    ///
    /// let mut ctx = ProcessCtx::new(
    ///     ExecutionPhase::EngineTick,
    ///     EngineTime { tick: 0, micro: 0, seq: 0 },
    /// );
    ///
    /// ctx.emit_custom_payload("transport.play", None, &true)
    ///     .expect("payload should serialize");
    ///
    /// assert_eq!(ctx.edits.pending.len(), 1);
    /// ```
    pub fn emit_custom_payload<U: Serialize>(&mut self, topic: impl Into<String>, origin: Option<NodeId>, payload: &U) -> serde_json::Result<()> {
        let event = CustomEvent::from_payload(topic, origin, payload)?;
        self.emit_custom_event(event);
        Ok(())
    }

    /// Returns `true` when `param` changed in this callback frame.
    pub fn has_param_changed(&self, param: NodeId) -> bool {
        self.events.iter().any(|event| matches!(event.kind, crate::events::EventKind::ParamChanged { param: changed, .. } if changed == param))
    }

    /// Returns `true` when at least one structural event is present in this callback frame.
    pub fn has_structure_changed(&self) -> bool {
        self.events.iter().any(|event| {
            matches!(
                event.kind,
                crate::events::EventKind::ChildAdded { .. }
                    | crate::events::EventKind::ChildRemoved { .. }
                    | crate::events::EventKind::ChildReplaced { .. }
                    | crate::events::EventKind::ChildMoved { .. }
                    | crate::events::EventKind::ChildReordered { .. }
                    | crate::events::EventKind::NodeCreated { .. }
                    | crate::events::EventKind::NodeDeleted { .. }
            )
        })
    }
}
