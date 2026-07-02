use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::node::NodeId;

/// Per-tick counters accumulated by engine internals.
///
/// Reset at the start of each `run_tick`; read via `Engine::tick_stats`.
#[derive(Default, Clone, Copy, Debug)]
pub struct TickStats {
    /// Number of nodes collected as due for scheduled updates this tick.
    pub nodes_due: usize,
    /// Number of `update()` callbacks actually invoked this tick.
    pub callbacks_fired: usize,
    /// Number of events pushed to the engine inbox this tick.
    pub events_emitted: usize,
    /// Number of edit requests applied via `apply_edits_without_history` this tick.
    pub edits_applied: usize,
    /// Number of stabilization passes completed this tick.
    pub stabilization_passes: usize,
    /// Number of times `get_or_build_tick_snapshot` constructed a new snapshot this tick.
    pub snapshot_rebuilds: usize,
    /// Number of full process-tree snapshots constructed this tick.
    pub snapshot_builds: usize,
    /// Time spent building full process-tree snapshots this tick.
    pub snapshot_build_ns: u128,
    /// Number of node records cloned into full process-tree snapshots this tick.
    pub snapshot_nodes_cloned: usize,
    /// Number of inbox events routed through listener/bubbling dispatch this tick.
    pub dispatch_events_routed: usize,
    /// Number of recipient deliveries produced by inbox routing this tick.
    pub dispatch_recipient_deliveries: usize,
    /// Largest recipient fanout observed for a single routed event this tick.
    pub dispatch_max_fanout: usize,
}

/// Pre-allocated scratch buffers reused across tick phases to avoid per-tick heap allocations.
///
/// Cleared at the start of each relevant phase. Buffers retain their allocated capacity between
/// ticks so the allocator amortizes cost after the first few busy ticks.
///
/// INVALIDATED BY: `run_tick` clears scheduled buffers before `run_scheduled_updates`.
/// Event-routing buffers are cleared at the start of each `route_event_recipients_into` call.
#[derive(Default)]
pub(crate) struct TickScratch {
    /// Due nodes collected by `collect_due_nodes_into`.
    pub(crate) due_nodes: Vec<NodeId>,
    /// Per-node firing count for the current scheduled-update pass.
    pub(crate) due_counts: HashMap<NodeId, usize>,
    /// Remaining delta tracking for multi-fire nodes within one scheduled pass.
    pub(crate) remaining_delta_by_node: HashMap<NodeId, Duration>,
    /// Per-node seen count within the current scheduled-update pass.
    pub(crate) seen_by_node: HashMap<NodeId, usize>,
    /// Recipient list for event routing; cleared per `route_event_recipients_into` call.
    pub(crate) recipients: Vec<NodeId>,
    /// Dedup set for event routing; cleared per `route_event_recipients_into` call.
    pub(crate) recipients_dedupe: HashSet<NodeId>,
    /// Ancestor depth map for subscription routing; cleared per `route_event_recipients_into` call.
    pub(crate) ancestry_depths: HashMap<NodeId, u32>,
    /// Per-tick performance counters; cleared at tick start via `clear_stats`.
    pub(crate) stats: TickStats,
}

impl TickScratch {
    pub(crate) fn clear_scheduled(&mut self) {
        self.due_nodes.clear();
        self.due_counts.clear();
        self.remaining_delta_by_node.clear();
        self.seen_by_node.clear();
    }

    pub(crate) fn clear_stats(&mut self) {
        self.stats = TickStats::default();
    }
}
