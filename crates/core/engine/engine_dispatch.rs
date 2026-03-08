use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::events::Event;
use crate::node::{EventPropagation, EventSubscription, Node, NodeId};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};

use super::{Engine, EngineEditError};

impl<T: Node> Engine<T> {
    pub(crate) fn apply_add_event_listener(&mut self, edit_index: usize, subscriber: NodeId, subscription: EventSubscription) -> Result<(), EngineEditError> {
        const OP: &str = "AddEventListener";

        if !self.nodes.contains(subscriber) {
            return Err(EngineEditError::NodeNotFound { edit_index, operation: OP, node: subscriber });
        }
        if !self.nodes.contains(subscription.node) {
            return Err(EngineEditError::NodeNotFound { edit_index, operation: OP, node: subscription.node });
        }

        self.event_listeners.entry(subscriber).or_default().insert(subscription);
        Ok(())
    }

    pub(crate) fn apply_remove_event_listener(&mut self, subscriber: NodeId, subscription: EventSubscription) {
        if let Some(subscriptions) = self.event_listeners.get_mut(&subscriber) {
            subscriptions.remove(&subscription);
            if subscriptions.is_empty() {
                self.event_listeners.remove(&subscriber);
            }
        }
    }

    pub(crate) fn purge_event_listeners_for_node(&mut self, node: NodeId) {
        self.event_listeners.remove(&node);
        for subscriptions in self.event_listeners.values_mut() {
            subscriptions.retain(|subscription| subscription.node != node);
        }
        self.event_listeners.retain(|_, subscriptions| !subscriptions.is_empty());
    }

    /// Precomputes per-node inbox payloads from currently buffered engine events.
    ///
    /// The returned vector preserves first-seen node order while events remain in
    /// engine emission order for each node.
    pub fn precompute_inbox_dispatch(&self) -> Vec<(NodeId, Vec<Event>)> {
        self.precompute_inbox_dispatch_since(0)
    }

    /// Precomputes per-node inbox payloads for events emitted at or after `start`.
    ///
    /// `start` is clamped to the current inbox length.
    pub(crate) fn precompute_inbox_dispatch_since(&self, start: usize) -> Vec<(NodeId, Vec<Event>)> {
        let mut index_by_node: HashMap<NodeId, usize> = HashMap::new();
        let mut per_node_events: Vec<(NodeId, Vec<Event>)> = Vec::new();

        let start = start.min(self.inbox.events.len());
        for event in self.inbox.events.iter().skip(start) {
            for recipient in self.route_event_recipients(event) {
                let index = match index_by_node.get(&recipient).copied() {
                    Some(index) => index,
                    None => {
                        let index = per_node_events.len();
                        per_node_events.push((recipient, Vec::new()));
                        index_by_node.insert(recipient, index);
                        index
                    }
                };
                per_node_events[index].1.push(event.clone());
            }
        }

        per_node_events
    }

    /// Runs only internal inbox preprocessing (no app-level `on_inbox`) for a precomputed batch.
    pub(crate) fn preprocess_precomputed_inbox(&mut self, phase: ExecutionPhase, per_node_events: Vec<(NodeId, Vec<Event>)>) -> Result<(), EngineEditError> {
        self.dispatch_precomputed_inbox_internal(phase, per_node_events, false)
    }

    /// Dispatches a precomputed per-node inbox batch.
    ///
    /// Edits requested by node callbacks are absorbed into the engine queue.
    pub fn dispatch_precomputed_inbox(&mut self, phase: ExecutionPhase, per_node_events: Vec<(NodeId, Vec<Event>)>) -> Result<(), EngineEditError> {
        self.dispatch_precomputed_inbox_internal(phase, per_node_events, true)
    }

    fn dispatch_precomputed_inbox_internal(&mut self, phase: ExecutionPhase, per_node_events: Vec<(NodeId, Vec<Event>)>, run_app_callbacks: bool) -> Result<(), EngineEditError> {
        let parameter_values: HashMap<NodeId, crate::parameter::ParamValue> = self.nodes.iter().filter_map(|(node_id, node)| node.engine_param_snapshot().map(|snapshot| (node_id, snapshot.value))).collect();
        let tree_snapshot = (!per_node_events.is_empty()).then(|| self.build_process_tree_snapshot());

        for (node_id, events) in per_node_events {
            if events.is_empty() {
                continue;
            }

            let mut ctx = ProcessCtx::new(phase, self.time);
            ctx.events = events;
            ctx.runtime_elapsed = self.runtime_elapsed;
            if let Some(tree_snapshot) = &tree_snapshot {
                ctx.set_tree_snapshot(Arc::clone(tree_snapshot));
            }

            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.engine_preprocess_inbox(&mut ctx);
                    let mut resolve = |param_id: NodeId| parameter_values.get(&param_id).cloned();
                    node.engine_sync_bound_param_handles(&mut resolve);
                    if run_app_callbacks {
                        node.on_inbox(&mut ctx);
                    }
                });
                self.absorb_edits(&mut ctx)?;
            }
        }

        Ok(())
    }

    /// Routes and dispatches all currently buffered inbox events to nodes.
    ///
    /// Successfully dispatched inbox events are cleared from `self.inbox`.
    pub fn dispatch_inbox(&mut self, phase: ExecutionPhase) -> Result<(), EngineEditError> {
        let per_node_events = self.precompute_inbox_dispatch();
        self.dispatch_precomputed_inbox(phase, per_node_events)?;
        self.inbox.clear();
        Ok(())
    }

    fn route_event_recipients(&self, event: &Event) -> Vec<NodeId> {
        let Some(origin) = event.kind.propagation_origin() else {
            return Vec::new();
        };

        if !self.nodes.contains(origin) {
            return Vec::new();
        }

        let ancestry_depths = self.origin_ancestry_depths(origin);
        let mut recipients = Vec::new();
        let mut dedupe = HashSet::new();

        self.collect_bubbling_recipients(event, origin, &mut recipients, &mut dedupe);
        self.collect_subscription_recipients(event, &ancestry_depths, &mut recipients, &mut dedupe);

        recipients
    }

    fn origin_ancestry_depths(&self, origin: NodeId) -> HashMap<NodeId, u32> {
        let mut out = HashMap::new();
        let mut current = Some(origin);
        let mut depth = 0u32;

        while let Some(node_id) = current {
            if out.insert(node_id, depth).is_some() {
                break;
            }

            current = self.nodes.get(node_id).and_then(|node| node.node_data().parent);
            depth = depth.saturating_add(1);
        }

        out
    }

    fn collect_bubbling_recipients(&self, event: &Event, origin: NodeId, recipients: &mut Vec<NodeId>, dedupe: &mut HashSet<NodeId>) {
        let mut current = origin;
        let mut depth = 0u32;
        let mut remaining_bubble = 0u32;

        loop {
            let Some(node) = self.nodes.get(current) else {
                break;
            };

            let effective_interest_depth = node.child_event_interest_depth(event).max(node.engine_child_event_interest_depth(event));
            let interested = depth == 0 || effective_interest_depth >= depth;
            let propagation = node.event_propagation(event, depth);

            if interested && matches!(propagation, EventPropagation::Notify | EventPropagation::Stop) && dedupe.insert(current) {
                recipients.push(current);
            }

            if propagation == EventPropagation::Stop {
                break;
            }

            remaining_bubble = remaining_bubble.saturating_add(node.bubble_event_depth(event));

            let Some(parent) = node.node_data().parent else {
                break;
            };
            let next_depth = depth.saturating_add(1);
            let parent_is_interested = self.nodes.get(parent).map(|parent| parent.child_event_interest_depth(event).max(parent.engine_child_event_interest_depth(event)) >= next_depth).unwrap_or(false);

            if remaining_bubble == 0 && !parent_is_interested {
                break;
            }

            if remaining_bubble > 0 {
                remaining_bubble -= 1;
            }

            current = parent;
            depth = next_depth;
        }
    }

    fn collect_subscription_recipients(&self, event: &Event, ancestry_depths: &HashMap<NodeId, u32>, recipients: &mut Vec<NodeId>, dedupe: &mut HashSet<NodeId>) {
        if self.event_listeners.is_empty() {
            return;
        }

        for (subscriber_id, subscriptions) in &self.event_listeners {
            let Some(subscriber) = self.nodes.get(*subscriber_id) else {
                continue;
            };
            let propagation = subscriber.event_propagation(event, 0);
            if propagation == EventPropagation::PassOn {
                continue;
            }

            let matches_runtime = subscriptions.iter().copied().any(|subscription| Self::subscription_matches_origin(ancestry_depths, subscription));
            let matches_subscription = matches_runtime;

            if matches_subscription && dedupe.insert(*subscriber_id) {
                recipients.push(*subscriber_id);
            }
        }
    }

    fn subscription_matches_origin(ancestry_depths: &HashMap<NodeId, u32>, subscription: EventSubscription) -> bool {
        ancestry_depths.get(&subscription.node).is_some_and(|depth| *depth <= subscription.max_depth)
    }
}
