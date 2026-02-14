use std::time::Duration;

use crate::edit::{Edit, EditQueue};
use crate::engine::EngineTime;
use crate::events::{CustomEvent, Event};
use crate::node::{EventSubscription, Node, NodeId};
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

    /// Queues a parameter update edit with an explicit coalescing strategy.
    pub fn set_param_with_behaviour(&mut self, node: NodeId, value: ParamValue, behaviour: ParameterEventBehaviour) {
        self.edits.push(Edit::SetParam { node, value, behaviour });
    }

    /// Queues insertion of a child node.
    pub fn add_child<N: Node + 'static>(&mut self, parent: NodeId, child: N, after: Option<NodeId>) {
        self.add_child_boxed(parent, Box::new(child), after);
    }

    /// Queues insertion of a boxed child node.
    pub fn add_child_boxed(&mut self, parent: NodeId, child: Box<dyn Node>, after: Option<NodeId>) {
        self.edits.push(Edit::AddNode {
            parent,
            prev_sibling: after,
            node: child,
        });
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
}
