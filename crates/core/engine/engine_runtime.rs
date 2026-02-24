use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

use crate::edit::Edit;
use crate::node::{Node, NodeId};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};

use super::{Engine, EngineEditError};

const RUNTIME_LOOP_CAP_INTERVAL: Duration = Duration::from_micros(1_000);

/// Per-node update frequency in hertz.
pub type NodeUpdateRate = u32;

/// Scheduling contract returned by each node during graph resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeExecutionRule {
    /// Nodes that must run before this node when both execute in the same tick.
    pub dependencies: Vec<NodeId>,
    /// Desired update rate in hertz.
    ///
    /// `None` means this node does not request periodic updates.
    pub update_rate: Option<NodeUpdateRate>,
}

impl NodeExecutionRule {
    /// Returns a passive rule with no dependencies and no update rate.
    pub fn passive() -> Self {
        Self::default()
    }

    /// Returns a periodic rule with `rate_hz` and no dependencies.
    pub fn periodic(rate_hz: NodeUpdateRate) -> Self {
        Self { dependencies: Vec::new(), update_rate: Some(rate_hz) }
    }

    /// Replaces dependencies on this rule.
    pub fn with_dependencies<I>(mut self, dependencies: I) -> Self
    where
        I: IntoIterator<Item = NodeId>,
    {
        self.dependencies = dependencies.into_iter().collect();
        self
    }
}

impl Default for NodeExecutionRule {
    fn default() -> Self {
        Self { dependencies: Vec::new(), update_rate: None }
    }
}

/// Runtime guardrails used while ticking the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeLimits {
    /// Maximum number of stabilization rounds allowed in one tick.
    pub max_stabilization_passes_per_tick: usize,
    /// Maximum number of update callbacks allowed in one tick.
    pub max_update_callbacks_per_tick: usize,
    /// Maximum catch-up firings processed per bucket in one tick.
    pub max_bucket_catch_up_per_tick: u32,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_stabilization_passes_per_tick: 256,
            max_update_callbacks_per_tick: 100_000,
            max_bucket_catch_up_per_tick: 4,
        }
    }
}

/// Error produced by schedule resolution and runtime ticking.
#[derive(Debug)]
pub enum EngineRuntimeError {
    /// Wrapper for edit-application failures.
    Edit(EngineEditError),
    /// A node declared a dependency on a missing node id.
    MissingDependency {
        /// Node that declared the dependency.
        node: NodeId,
        /// Missing dependency id.
        dependency: NodeId,
    },
    /// Dependency graph contains at least one cycle.
    DependencyCycle {
        /// Nodes still holding unresolved in-degree after topological sorting.
        nodes: Vec<NodeId>,
    },
    /// Node requested an invalid zero-hertz update rate.
    InvalidUpdateRate {
        /// Node that returned an invalid rule.
        node: NodeId,
        /// Invalid rate value.
        rate_hz: NodeUpdateRate,
    },
    /// Runtime exceeded stabilization pass limit and likely entered an event/edit loop.
    InfiniteEventEditCycle {
        /// Tick number where the loop was detected.
        tick: u64,
        /// Number of passes attempted before aborting.
        passes: usize,
    },
    /// Runtime exceeded update callback budget for a single tick.
    UpdateBudgetExceeded {
        /// Tick number where the overflow happened.
        tick: u64,
        /// Callback count reached before aborting.
        callbacks: usize,
        /// Configured callback limit.
        limit: usize,
    },
}

impl fmt::Display for EngineRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Edit(err) => write!(f, "{err}"),
            Self::MissingDependency { node, dependency } => {
                write!(f, "node {:?} depends on missing node {:?}", node, dependency)
            }
            Self::DependencyCycle { nodes } => {
                write!(f, "dependency cycle detected among nodes {:?}", nodes)
            }
            Self::InvalidUpdateRate { node, rate_hz } => {
                write!(f, "node {:?} declared invalid update rate {}hz", node, rate_hz)
            }
            Self::InfiniteEventEditCycle { tick, passes } => {
                write!(f, "runtime aborted at tick {tick} after {passes} stabilization passes (possible event/edit cycle)")
            }
            Self::UpdateBudgetExceeded { tick, callbacks, limit } => {
                write!(f, "runtime aborted at tick {tick}: update callbacks {callbacks} exceeded limit {limit}")
            }
        }
    }
}

impl Error for EngineRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Edit(err) => Some(err),
            _ => None,
        }
    }
}

impl From<EngineEditError> for EngineRuntimeError {
    fn from(value: EngineEditError) -> Self {
        Self::Edit(value)
    }
}

#[derive(Default)]
pub(crate) struct ScheduleMgr {
    topo_order: Vec<NodeId>,
    buckets: Vec<ScheduleBucket>,
    bucket_by_node: HashMap<NodeId, usize>,
}

struct ScheduleBucket {
    rate_hz: NodeUpdateRate,
    interval: Duration,
    accumulator: Duration,
    nodes: Vec<NodeId>,
    due_count: u32,
}

impl ScheduleBucket {
    fn new(rate_hz: NodeUpdateRate, nodes: Vec<NodeId>) -> Self {
        let nanos = (1_000_000_000u64 / rate_hz as u64).max(1);
        Self {
            rate_hz,
            interval: Duration::from_nanos(nanos),
            accumulator: Duration::ZERO,
            nodes,
            due_count: 0,
        }
    }
}

impl ScheduleMgr {
    fn rebuild(&mut self, topo_order: Vec<NodeId>, rules: &HashMap<NodeId, NodeExecutionRule>) -> Result<(), EngineRuntimeError> {
        self.topo_order = topo_order;
        self.bucket_by_node.clear();
        self.buckets.clear();

        let mut buckets_by_rate: BTreeMap<NodeUpdateRate, Vec<NodeId>> = BTreeMap::new();
        for node_id in &self.topo_order {
            let Some(rule) = rules.get(node_id) else {
                continue;
            };

            let Some(rate_hz) = rule.update_rate else {
                continue;
            };

            if rate_hz == 0 {
                return Err(EngineRuntimeError::InvalidUpdateRate { node: *node_id, rate_hz });
            }

            buckets_by_rate.entry(rate_hz).or_default().push(*node_id);
        }

        for (rate_hz, nodes) in buckets_by_rate {
            let bucket_index = self.buckets.len();
            for node in &nodes {
                self.bucket_by_node.insert(*node, bucket_index);
            }
            self.buckets.push(ScheduleBucket::new(rate_hz, nodes));
        }

        Ok(())
    }

    fn collect_due_nodes(&mut self, elapsed: Duration, max_bucket_catch_up: u32) -> Vec<NodeId> {
        if self.buckets.is_empty() {
            return Vec::new();
        }

        let catch_up_limit = max_bucket_catch_up.max(1);
        let mut max_due = 0u32;

        for bucket in &mut self.buckets {
            bucket.accumulator = bucket.accumulator.saturating_add(elapsed);
            bucket.due_count = 0;

            while bucket.due_count < catch_up_limit && bucket.accumulator >= bucket.interval {
                bucket.accumulator = bucket.accumulator.saturating_sub(bucket.interval);
                bucket.due_count += 1;
            }

            max_due = max_due.max(bucket.due_count);
        }

        if max_due == 0 {
            return Vec::new();
        }

        let mut due_nodes = Vec::new();
        for round in 0..max_due {
            for node_id in &self.topo_order {
                let Some(bucket_index) = self.bucket_by_node.get(node_id).copied() else {
                    continue;
                };
                if self.buckets[bucket_index].due_count > round {
                    due_nodes.push(*node_id);
                }
            }
        }

        for bucket in &mut self.buckets {
            bucket.due_count = 0;
        }

        due_nodes
    }

    fn bucket_nodes(&self, rate_hz: NodeUpdateRate) -> Option<&[NodeId]> {
        self.buckets.iter().find(|bucket| bucket.rate_hz == rate_hz).map(|bucket| bucket.nodes.as_slice())
    }
}

impl<T: Node> Engine<T> {
    pub(crate) fn mark_schedule_dirty(&mut self) {
        self.runtime_resolve_pending = true;
    }

    /// Returns whether runtime schedule recomputation is pending.
    pub fn is_resolve_pending(&self) -> bool {
        self.runtime_resolve_pending
    }

    /// Returns current runtime guardrail configuration.
    pub fn runtime_limits(&self) -> RuntimeLimits {
        self.runtime_limits
    }

    /// Replaces runtime guardrail configuration.
    pub fn set_runtime_limits(&mut self, limits: RuntimeLimits) {
        self.runtime_limits = limits;
    }

    /// Queues a graph reevaluation request through the edit pipeline.
    pub fn request_graph_reevaluation(&mut self) {
        self.edits.push(Edit::ReevaluateGraph);
    }

    /// Resolves scheduler state when marked dirty.
    ///
    /// Returns `true` when resolution happened.
    pub fn resolve_if_needed(&mut self) -> Result<bool, EngineRuntimeError> {
        if !self.runtime_resolve_pending {
            return Ok(false);
        }

        self.resolve()?;
        Ok(true)
    }

    /// Rebuilds runtime scheduling from current node execution rules.
    pub fn resolve(&mut self) -> Result<(), EngineRuntimeError> {
        let previously_scheduled_nodes: HashSet<NodeId> = self.runtime_schedule.bucket_by_node.keys().copied().collect();
        let rules = self.collect_execution_rules();
        let topo_order = self.topological_sort(&rules)?;
        self.runtime_schedule.rebuild(topo_order, &rules)?;
        for node_id in self.runtime_schedule.bucket_by_node.keys().copied() {
            if !previously_scheduled_nodes.contains(&node_id) {
                self.last_update_elapsed_by_node.insert(node_id, self.runtime_elapsed);
            }
        }
        self.runtime_resolve_pending = false;
        Ok(())
    }

    /// Returns whether `node` is enabled.
    ///
    /// When `in_hierarchy` is `true`, all ancestors (including self) must be enabled.
    pub fn is_enabled(&self, node: NodeId, in_hierarchy: bool) -> bool {
        let Some(entry) = self.nodes.get(node) else {
            return false;
        };

        if !in_hierarchy {
            return entry.node_data().meta.enabled;
        }

        let mut cursor = Some(node);
        while let Some(current) = cursor {
            let Some(current_entry) = self.nodes.get(current) else {
                return false;
            };

            if !current_entry.node_data().meta.enabled {
                return false;
            }

            cursor = current_entry.node_data().parent;
        }

        true
    }

    /// Returns the current global topological order used by runtime updates.
    pub fn schedule_topology(&self) -> &[NodeId] {
        &self.runtime_schedule.topo_order
    }

    /// Returns the node bucket for a given update rate when present.
    pub fn schedule_bucket_nodes(&self, rate_hz: NodeUpdateRate) -> Option<&[NodeId]> {
        self.runtime_schedule.bucket_nodes(rate_hz)
    }

    /// Executes one runtime tick with an elapsed wall-clock delta.
    pub fn run_tick(&mut self, elapsed: Duration) -> Result<(), EngineRuntimeError> {
        self.resolve_if_needed()?;

        self.time.tick = self.time.tick.saturating_add(1);
        self.time.micro = 0;
        self.time.seq = 0;
        self.runtime_elapsed = self.runtime_elapsed.saturating_add(elapsed);

        self.absorb_external_edits()?;
        if !self.edits.pending.is_empty() {
            self.apply_edits_without_history()?;
            self.resolve_if_needed()?;
        }

        self.run_scheduled_updates(elapsed)?;

        self.run_stabilization_rounds()?;
        self.sync_logger_ui_events();
        Ok(())
    }

    /// Runs the runtime loop for `duration`, capped at 1000hz.
    pub fn run_for(&mut self, duration: Duration) -> Result<(), EngineRuntimeError> {
        let start = Instant::now();
        let mut previous_tick_start = start;

        while start.elapsed() < duration {
            let tick_start = Instant::now();
            let elapsed = tick_start.saturating_duration_since(previous_tick_start);
            previous_tick_start = tick_start;

            self.run_tick(elapsed)?;

            let tick_elapsed = tick_start.elapsed();
            if tick_elapsed < RUNTIME_LOOP_CAP_INTERVAL {
                let sleep_for = RUNTIME_LOOP_CAP_INTERVAL - tick_elapsed;
                let remaining_total = duration.saturating_sub(start.elapsed());
                if !remaining_total.is_zero() {
                    thread::sleep(sleep_for.min(remaining_total));
                }
            }
        }

        Ok(())
    }

    /// Runs the runtime loop indefinitely, capped at 1000hz.
    pub fn run_loop(&mut self) -> Result<(), EngineRuntimeError> {
        let mut previous_tick_start = Instant::now();

        loop {
            let tick_start = Instant::now();
            let elapsed = tick_start.saturating_duration_since(previous_tick_start);
            previous_tick_start = tick_start;

            self.run_tick(elapsed)?;

            let tick_elapsed = tick_start.elapsed();
            if tick_elapsed < RUNTIME_LOOP_CAP_INTERVAL {
                thread::sleep(RUNTIME_LOOP_CAP_INTERVAL - tick_elapsed);
            }
        }
    }

    fn collect_execution_rules(&self) -> HashMap<NodeId, NodeExecutionRule> {
        self.nodes.iter().filter_map(|(node_id, node)| self.is_enabled(node_id, true).then_some((node_id, node.execution_rule()))).collect()
    }

    fn topological_sort(&self, rules: &HashMap<NodeId, NodeExecutionRule>) -> Result<Vec<NodeId>, EngineRuntimeError> {
        let mut indegree: HashMap<NodeId, usize> = self.nodes.keys().map(|node_id| (node_id, 0usize)).collect();
        let mut outgoing: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        for (node_id, rule) in rules {
            let mut dedupe = HashSet::new();
            for dependency in &rule.dependencies {
                if !indegree.contains_key(dependency) {
                    return Err(EngineRuntimeError::MissingDependency { node: *node_id, dependency: *dependency });
                }
                if !dedupe.insert(*dependency) {
                    continue;
                }

                outgoing.entry(*dependency).or_default().push(*node_id);
                if let Some(indegree_value) = indegree.get_mut(node_id) {
                    *indegree_value += 1;
                }
            }
        }

        let mut ready: BTreeSet<u64> = indegree.iter().filter_map(|(node_id, indegree)| (*indegree == 0).then_some(node_id.0)).collect();

        let mut sorted = Vec::with_capacity(indegree.len());

        while let Some(next) = ready.iter().next().copied() {
            ready.remove(&next);
            let node_id = NodeId(next);
            sorted.push(node_id);

            if let Some(dependents) = outgoing.get(&node_id) {
                for dependent in dependents {
                    if let Some(indegree_value) = indegree.get_mut(dependent) {
                        *indegree_value -= 1;
                        if *indegree_value == 0 {
                            ready.insert(dependent.0);
                        }
                    }
                }
            }
        }

        if sorted.len() == indegree.len() {
            return Ok(sorted);
        }

        let mut cycle_nodes: Vec<NodeId> = indegree.into_iter().filter_map(|(node, indegree)| (indegree > 0).then_some(node)).collect();
        cycle_nodes.sort_by_key(|node| node.0);

        Err(EngineRuntimeError::DependencyCycle { nodes: cycle_nodes })
    }

    fn run_scheduled_updates(&mut self, elapsed: Duration) -> Result<(), EngineRuntimeError> {
        let due_nodes = self.runtime_schedule.collect_due_nodes(elapsed, self.runtime_limits.max_bucket_catch_up_per_tick);
        let mut callback_count = 0usize;
        let mut due_counts: HashMap<NodeId, usize> = HashMap::new();
        let mut remaining_delta_by_node: HashMap<NodeId, Duration> = HashMap::new();
        let mut seen_by_node: HashMap<NodeId, usize> = HashMap::new();

        for node_id in &due_nodes {
            *due_counts.entry(*node_id).or_default() += 1;
        }

        for node_id in due_counts.keys() {
            let previous = self.last_update_elapsed_by_node.get(node_id).copied().unwrap_or(Duration::ZERO);
            remaining_delta_by_node.insert(*node_id, self.runtime_elapsed.saturating_sub(previous));
        }

        for node_id in due_nodes {
            if !self.is_enabled(node_id, true) {
                continue;
            }

            let seen = seen_by_node.entry(node_id).or_default();
            *seen += 1;

            let total_occurrences = due_counts.get(&node_id).copied().unwrap_or(1);
            let remaining_occurrences = total_occurrences.saturating_sub(*seen - 1).max(1);
            let remaining_delta = remaining_delta_by_node.entry(node_id).or_insert(Duration::ZERO);

            let delta_time = if remaining_occurrences == 1 {
                let dt = *remaining_delta;
                *remaining_delta = Duration::ZERO;
                dt
            } else {
                let dt = *remaining_delta / remaining_occurrences as u32;
                *remaining_delta = remaining_delta.saturating_sub(dt);
                dt
            };

            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.delta_time = delta_time;
            let mut did_update = false;
            if let Some(node) = self.nodes.get_mut(node_id) {
                crate::logger::with_node_origin(node_id, || {
                    node.update(&mut ctx);
                });
                did_update = true;
                callback_count += 1;
                if callback_count > self.runtime_limits.max_update_callbacks_per_tick {
                    return Err(EngineRuntimeError::UpdateBudgetExceeded {
                        tick: self.time.tick,
                        callbacks: callback_count,
                        limit: self.runtime_limits.max_update_callbacks_per_tick,
                    });
                }
            }
            self.absorb_edits(&mut ctx)?;

            if did_update {
                let last = self.last_update_elapsed_by_node.entry(node_id).or_insert(Duration::ZERO);
                *last = last.saturating_add(delta_time);
            }
        }

        Ok(())
    }

    fn run_stabilization_rounds(&mut self) -> Result<(), EngineRuntimeError> {
        let mut pass = 0usize;

        loop {
            self.absorb_external_edits()?;

            if self.inbox.events.is_empty() && self.edits.pending.is_empty() {
                break;
            }

            if pass >= self.runtime_limits.max_stabilization_passes_per_tick {
                return Err(EngineRuntimeError::InfiniteEventEditCycle { tick: self.time.tick, passes: pass });
            }

            self.time.micro = (pass as u32).saturating_add(1);
            self.time.seq = 0;

            if !self.inbox.events.is_empty() {
                self.dispatch_inbox(ExecutionPhase::EndOfTickStabilization)?;
            }

            if !self.edits.pending.is_empty() {
                self.apply_edits_without_history()?;
                self.resolve_if_needed()?;
            }

            pass += 1;
        }

        Ok(())
    }
}
