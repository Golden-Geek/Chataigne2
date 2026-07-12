use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use crate::events::{Event, EventFrame, EventKind};
use crate::node::{EventPropagation, EventSubscription, Node, NodeId};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};

use super::{Engine, EngineEditError};

static DISPATCH_PERF_TRACE_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var_os("CHATAIGNE_PERF_TRACE").is_some_and(|value| {
        let value = value.to_string_lossy();
        !matches!(value.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "off")
    })
});

impl<T: Node> Engine<T> {
    pub(crate) fn apply_add_event_listener(
        &mut self,
        edit_index: usize,
        subscriber: NodeId,
        subscription: EventSubscription,
    ) -> Result<(), EngineEditError> {
        const OP: &str = "AddEventListener";

        if !self.nodes.contains(subscriber) {
            return Err(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node: subscriber,
            });
        }
        if !self.nodes.contains(subscription.node) {
            return Err(EngineEditError::NodeNotFound {
                edit_index,
                operation: OP,
                node: subscription.node,
            });
        }

        self.event_listeners.add(subscriber, subscription);
        Ok(())
    }

    pub(crate) fn apply_remove_event_listener(&mut self, subscriber: NodeId, subscription: EventSubscription) {
        self.event_listeners.remove(subscriber, subscription);
    }

    pub(crate) fn purge_event_listeners_for_node(&mut self, node: NodeId) {
        self.event_listeners.purge_for_node(node);
    }

    /// Precomputes per-node inbox payloads from currently buffered engine events.
    ///
    /// The returned vector preserves first-seen node order while events remain in
    /// engine emission order for each node.
    pub fn precompute_inbox_dispatch(&mut self) -> Vec<(NodeId, EventFrame)> {
        self.precompute_inbox_dispatch_since(0)
    }

    /// Precomputes per-node inbox payloads for events emitted at or after `start`.
    ///
    /// `start` is clamped to the current inbox length. Uses `tick_scratch` routing buffers
    /// to avoid per-event heap allocations on the tick path.
    pub(crate) fn precompute_inbox_dispatch_since(&mut self, start: usize) -> Vec<(NodeId, EventFrame)> {
        let mut index_by_node: HashMap<NodeId, usize> = HashMap::new();
        let mut per_node_events: Vec<(NodeId, EventFrame)> = Vec::new();

        // Take routing scratch buffers out so &self is freely available inside the loop.
        let mut recipients = std::mem::take(&mut self.tick_scratch.recipients);
        let mut dedupe = std::mem::take(&mut self.tick_scratch.recipients_dedupe);
        let mut ancestry_depths = std::mem::take(&mut self.tick_scratch.ancestry_depths);

        let event_count = self.inbox.events.len();
        let start = start.min(event_count);
        for i in start..event_count {
            let event = &self.inbox.events[i];
            self.route_event_recipients_into(event, &mut recipients, &mut dedupe, &mut ancestry_depths);
            let event = Arc::new(event.clone());
            self.tick_scratch.stats.dispatch_events_routed += 1;
            self.tick_scratch.stats.dispatch_recipient_deliveries += recipients.len();
            self.tick_scratch.stats.dispatch_max_fanout =
                self.tick_scratch.stats.dispatch_max_fanout.max(recipients.len());
            for &recipient in &recipients {
                let index = match index_by_node.get(&recipient).copied() {
                    Some(index) => index,
                    None => {
                        let index = per_node_events.len();
                        per_node_events.push((recipient, EventFrame::new()));
                        index_by_node.insert(recipient, index);
                        index
                    }
                };
                per_node_events[index].1.push_shared(Arc::clone(&event));
            }
        }

        // Return routing scratch buffers.
        recipients.clear();
        dedupe.clear();
        ancestry_depths.clear();
        self.tick_scratch.recipients = recipients;
        self.tick_scratch.recipients_dedupe = dedupe;
        self.tick_scratch.ancestry_depths = ancestry_depths;

        per_node_events
    }

    /// Runs only internal inbox preprocessing (no app-level `on_inbox`) for a precomputed batch.
    pub(crate) fn preprocess_precomputed_inbox(
        &mut self,
        phase: ExecutionPhase,
        per_node_events: Vec<(NodeId, EventFrame)>,
    ) -> Result<(), EngineEditError> {
        self.dispatch_precomputed_inbox_internal(phase, per_node_events, false)
    }

    /// Expands only macro-generated declared-child structure for a precomputed batch.
    ///
    /// Unlike regular internal preprocessing, this path does not synchronize runtime
    /// parameter state and cannot invoke application inbox callbacks. It is used by
    /// declaration-only persistence baselines where runtime lifecycle and IO must stay dormant.
    pub(crate) fn materialize_declared_precomputed_inbox(
        &mut self,
        phase: ExecutionPhase,
        per_node_events: Vec<(NodeId, EventFrame)>,
    ) -> Result<(), EngineEditError> {
        if per_node_events.is_empty() {
            return Ok(());
        }

        let tree_snapshot = self.build_process_tree_snapshot();
        for (node_id, events) in per_node_events {
            if events.is_empty() {
                continue;
            }

            let mut ctx = ProcessCtx::new(phase, self.time);
            ctx.events = events;
            ctx.runtime_elapsed = self.runtime_elapsed;
            ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.engine_materialize_declared_inbox(&mut ctx);
                });
                self.absorb_edits(&mut ctx)?;
            }
        }

        Ok(())
    }

    /// Dispatches a precomputed per-node inbox batch.
    ///
    /// Edits requested by node callbacks are absorbed into the engine queue.
    pub fn dispatch_precomputed_inbox(
        &mut self,
        phase: ExecutionPhase,
        per_node_events: Vec<(NodeId, EventFrame)>,
    ) -> Result<(), EngineEditError> {
        self.dispatch_precomputed_inbox_internal(phase, per_node_events, true)
    }

    fn dispatch_precomputed_inbox_internal(
        &mut self,
        phase: ExecutionPhase,
        per_node_events: Vec<(NodeId, EventFrame)>,
        run_app_callbacks: bool,
    ) -> Result<(), EngineEditError> {
        let trace = *DISPATCH_PERF_TRACE_ENABLED;
        let dispatch_start = trace.then(Instant::now);
        let mut trace_by_type: Option<HashMap<String, (usize, usize, u128)>> = trace.then(HashMap::new);

        // Take ownership to avoid borrow conflicts with the &mut self calls below.
        let mut parameter_values = std::mem::take(&mut self.parameter_values_cache);
        let mut snapshot_requesters: Option<HashMap<String, usize>> = trace.then(HashMap::new);
        let needs_tree_snapshot = run_app_callbacks
            && per_node_events.iter().any(|(node_id, events)| {
                let requires = self
                    .nodes
                    .get(*node_id)
                    .is_some_and(|node| node.inbox_requires_tree_snapshot(events));
                if requires {
                    if let Some(snapshot_requesters) = snapshot_requesters.as_mut() {
                        let node_type = self
                            .nodes
                            .get(*node_id)
                            .map(|node| node.get_type().to_owned())
                            .unwrap_or_else(|| "<missing>".to_owned());
                        let event_kinds = events
                            .iter()
                            .take(4)
                            .map(|event| match &event.kind {
                                EventKind::ParamChanged { .. } => "ParamChanged",
                                EventKind::ParamControlChanged { .. } => "ParamControlChanged",
                                EventKind::ParamConstraintsChanged { .. } => "ParamConstraintsChanged",
                                EventKind::ChildAdded { .. } => "ChildAdded",
                                EventKind::ChildRemoved { .. } => "ChildRemoved",
                                EventKind::ChildReplaced { .. } => "ChildReplaced",
                                EventKind::ChildMoved { .. } => "ChildMoved",
                                EventKind::ChildReordered { .. } => "ChildReordered",
                                EventKind::NodeCreated { .. } => "NodeCreated",
                                EventKind::NodeDeleted { .. } => "NodeDeleted",
                                EventKind::MetaChanged { .. } => "MetaChanged",
                                EventKind::GraphTransaction { .. } => "GraphTransaction",
                                EventKind::Custom(_) => "Custom",
                            })
                            .collect::<Vec<_>>()
                            .join("+");
                        *snapshot_requesters
                            .entry(format!("{node_type}[{event_kinds}]"))
                            .or_default() += 1;
                    }
                }
                requires
            });
        let snapshot_start = trace.then(Instant::now);
        let tree_snapshot = needs_tree_snapshot.then(|| {
            self.tick_scratch.stats.snapshot_builds += 1;
            self.tick_scratch.stats.snapshot_nodes_cloned += self.nodes.len();
            let started = Instant::now();
            let snapshot = self.build_process_tree_snapshot();
            self.tick_scratch.stats.snapshot_build_ns += started.elapsed().as_nanos();
            snapshot
        });
        let snapshot_us = snapshot_start.map(|start| start.elapsed().as_micros());

        for (node_id, events) in per_node_events {
            if events.is_empty() {
                continue;
            }

            let trace_node_type = trace.then(|| {
                self.nodes
                    .get(node_id)
                    .map(|node| node.get_type().to_owned())
                    .unwrap_or_else(|| "<missing>".to_owned())
            });
            let trace_event_count = events.len();

            let mut ctx = ProcessCtx::new(phase, self.time);
            ctx.events = events;
            ctx.runtime_elapsed = self.runtime_elapsed;
            if let Some(tree_snapshot) = &tree_snapshot {
                ctx.set_tree_snapshot(Arc::clone(tree_snapshot));
            }

            let events_before = self.inbox.events.len();
            if let Some(node) = self.nodes.get_mut(node_id) {
                let node_start = trace.then(Instant::now);
                crate::logger::with_node_origin(node_id, || {
                    node.engine_preprocess_inbox(&mut ctx);
                    let mut resolve = |param_id: NodeId| parameter_values.get(&param_id).cloned();
                    node.engine_sync_bound_param_handles(&mut resolve);
                    if run_app_callbacks {
                        node.on_inbox(&mut ctx);
                    }
                });
                if let (Some(node_type), Some(start), Some(trace_by_type)) =
                    (trace_node_type, node_start, trace_by_type.as_mut())
                {
                    let entry = trace_by_type.entry(node_type).or_insert((0, 0, 0));
                    entry.0 += 1;
                    entry.1 += trace_event_count;
                    entry.2 += start.elapsed().as_micros();
                }
                self.absorb_edits(&mut ctx)?;
            }
            for event in self.inbox.events.iter().skip(events_before) {
                match &event.kind {
                    EventKind::ParamChanged { param, new_value, .. } => {
                        parameter_values.insert(*param, new_value.clone());
                    }
                    EventKind::NodeCreated { node } => {
                        if let Some(n) = self.nodes.get(*node) {
                            if let Some(snapshot) = n.engine_param_snapshot() {
                                parameter_values.insert(*node, snapshot.value);
                            }
                        }
                    }
                    EventKind::NodeDeleted { node } => {
                        parameter_values.remove(node);
                    }
                    _ => {}
                }
            }
        }

        // Return the cache so it can be reused next call (same as run_scheduled_updates).
        self.parameter_values_cache = parameter_values;

        if let (Some(start), Some(trace_by_type), Some(snapshot_requesters)) =
            (dispatch_start, trace_by_type, snapshot_requesters)
        {
            let elapsed_ms = start.elapsed().as_millis();
            if elapsed_ms > 0 {
                let mut entries: Vec<_> = trace_by_type.into_iter().collect();
                entries.sort_by(|left, right| right.1.2.cmp(&left.1.2));
                let summary = entries
                    .into_iter()
                    .take(8)
                    .map(|(node_type, (nodes, events, micros))| {
                        format!("{node_type}:nodes={nodes},events={events},us={micros}")
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                let mut snapshot_requesters: Vec<_> = snapshot_requesters.into_iter().collect();
                snapshot_requesters.sort_by(|left, right| right.1.cmp(&left.1));
                let snapshot_requesters = snapshot_requesters
                    .into_iter()
                    .take(8)
                    .map(|(node_type, count)| format!("{node_type}:{count}"))
                    .collect::<Vec<_>>()
                    .join(",");
                eprintln!(
                    "[engine] dispatch_profile total_ms={} snapshot={} snapshot_us={} snapshot_requesters={} recipients={}",
                    elapsed_ms,
                    needs_tree_snapshot,
                    snapshot_us.unwrap_or(0),
                    snapshot_requesters,
                    summary
                );
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

    /// Routes `event` and returns owned recipient list. For non-tick callers that don't hold
    /// a `TickScratch`; allocates fresh containers each call.
    pub(crate) fn route_event_recipients(&self, event: &Event) -> Vec<NodeId> {
        let mut recipients = Vec::new();
        let mut dedupe = HashSet::new();
        let mut ancestry_depths = HashMap::new();
        self.route_event_recipients_into(event, &mut recipients, &mut dedupe, &mut ancestry_depths);
        recipients
    }

    /// Routes `event` into caller-provided scratch buffers to avoid per-event allocation.
    ///
    /// Clears all three buffers before writing. On return, `recipients` contains the ordered
    /// recipient list.
    pub(crate) fn route_event_recipients_into(
        &self,
        event: &Event,
        recipients: &mut Vec<NodeId>,
        dedupe: &mut HashSet<NodeId>,
        ancestry_depths: &mut HashMap<NodeId, u32>,
    ) {
        recipients.clear();
        dedupe.clear();

        let Some(origin) = event.kind.propagation_origin() else {
            return;
        };

        if !self.nodes.contains(origin) {
            return;
        }

        self.origin_ancestry_depths_into(origin, ancestry_depths);
        self.collect_bubbling_recipients(event, origin, recipients, dedupe);
        self.collect_subscription_recipients(event, ancestry_depths, recipients, dedupe);
    }

    fn origin_ancestry_depths_into(&self, origin: NodeId, out: &mut HashMap<NodeId, u32>) {
        out.clear();
        let mut current = Some(origin);
        let mut depth = 0u32;

        while let Some(node_id) = current {
            if out.insert(node_id, depth).is_some() {
                break;
            }

            current = self.nodes.get(node_id).and_then(|node| node.node_data().parent);
            depth = depth.saturating_add(1);
        }
    }

    fn collect_bubbling_recipients(
        &self,
        event: &Event,
        origin: NodeId,
        recipients: &mut Vec<NodeId>,
        dedupe: &mut HashSet<NodeId>,
    ) {
        let mut current = origin;
        let mut depth = 0u32;
        let mut remaining_bubble = 0u32;

        loop {
            let Some(node) = self.nodes.get(current) else {
                break;
            };

            let effective_interest_depth = node
                .child_event_interest_depth(event)
                .max(node.engine_child_event_interest_depth(event));
            let interested = depth == 0 || effective_interest_depth >= depth;
            let propagation = node.event_propagation(event, depth);

            if interested
                && matches!(propagation, EventPropagation::Notify | EventPropagation::Stop)
                && dedupe.insert(current)
            {
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
            let parent_is_interested = self
                .nodes
                .get(parent)
                .map(|parent| {
                    parent
                        .child_event_interest_depth(event)
                        .max(parent.engine_child_event_interest_depth(event))
                        >= next_depth
                })
                .unwrap_or(false);

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

    fn collect_subscription_recipients(
        &self,
        event: &Event,
        ancestry_depths: &HashMap<NodeId, u32>,
        recipients: &mut Vec<NodeId>,
        dedupe: &mut HashSet<NodeId>,
    ) {
        if self.event_listeners.is_empty() {
            return;
        }

        for (&origin, &depth) in ancestry_depths {
            for &(subscriber_id, max_depth) in self.event_listeners.listeners_for_origin(origin) {
                if depth > max_depth || dedupe.contains(&subscriber_id) {
                    continue;
                }
                let Some(subscriber) = self.nodes.get(subscriber_id) else {
                    continue;
                };
                if subscriber.event_propagation(event, 0) == EventPropagation::PassOn {
                    continue;
                }
                if dedupe.insert(subscriber_id) {
                    recipients.push(subscriber_id);
                }
            }
        }
    }
}
