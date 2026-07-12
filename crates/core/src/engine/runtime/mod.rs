pub(super) use std::collections::{BTreeMap, HashMap, HashSet};
pub(super) use std::error::Error;
pub(super) use std::fmt;
pub(super) use std::sync::{Arc, LazyLock};
pub(super) use std::thread;
pub(super) use std::time::{Duration, Instant};

pub(super) use crate::edit::Edit;
#[cfg(debug_assertions)]
pub(super) use crate::edit::EditRequest;
#[cfg(debug_assertions)]
pub(super) use crate::events::Event;
pub(super) use crate::events::EventKind;
pub(super) use crate::node::{Node, NodeId};
pub(super) use crate::process_ctx::{ExecutionPhase, ProcessCtx};

pub(super) use super::{Engine, EngineEditError};

pub(super) const PERF_LOG_TICK_THRESHOLD_MS: u128 = 8;
pub(super) const PERF_LOG_MIN_TICK_INTERVAL: u64 = 60;
pub(super) const STABILIZATION_WARN_PASSES: usize = 4;
pub(super) static PERF_TRACE_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var_os("CHATAIGNE_PERF_TRACE").is_some_and(|value| {
        let value = value.to_string_lossy();
        !matches!(value.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "off")
    })
});
#[cfg(debug_assertions)]
pub(super) const DEBUG_VERBOSE_STABILIZATION_THRESHOLD: usize = 100;
#[cfg(debug_assertions)]
pub(super) const DEBUG_VERBOSE_STABILIZATION_SAMPLE_LIMIT: usize = 8;

mod errors;
mod limits;
mod scheduled_updates;
mod scheduler;
mod stabilization;
mod tick;
mod trace;

pub use errors::EngineRuntimeError;
pub use limits::{
    DEFAULT_RUNTIME_LOOP_MAX_FREQUENCY_HZ, FixedStepConfig, NodeExecutionRule, NodeUpdateRate, RuntimeLimits,
    runtime_loop_interval_for_frequency_hz,
};
pub(crate) use scheduler::ScheduleMgr;

impl<T: Node> Engine<T> {
    pub(crate) fn mark_schedule_dirty(&mut self) {
        self.runtime_resolve_pending = true;
        self.mark_param_control_index_dirty();
        self.tick_tree_snapshot = None;
    }

    /// Inserts or refreshes the param cache entry for `node_id`.
    /// Cheap no-op when the node has no parameter snapshot.
    pub(crate) fn populate_param_cache_entry(&mut self, node_id: NodeId) {
        if let Some(node) = self.nodes.get(node_id)
            && let Some(snapshot) = node.engine_param_snapshot()
        {
            self.parameter_values_cache.insert(node_id, snapshot.value);
            self.mark_param_control_index_dirty();
        }
    }

    /// Removes the param cache entry for `node_id` (idempotent).
    pub(crate) fn purge_param_cache_entry(&mut self, node_id: NodeId) {
        self.parameter_values_cache.remove(&node_id);
        self.mark_param_control_index_dirty();
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

    /// Returns whether `node` is enabled.
    ///
    /// When `in_hierarchy` is `true`, uses the cached `effective_enabled` field on
    /// `NodeData` to avoid a parent-chain walk.
    ///
    /// INVALIDATED BY: AddNode/AddUserItem/AddNodeTree/AddUserItemTree
    ///   (init from parent), MoveNode (subtree recompute), PatchMeta when
    ///   `enabled` changes (subtree recompute).
    pub fn is_enabled(&self, node: NodeId, in_hierarchy: bool) -> bool {
        let Some(entry) = self.nodes.get(node) else {
            return false;
        };

        if !in_hierarchy {
            return entry.node_data().meta.enabled;
        }

        entry.node_data().effective_enabled
    }

    /// Returns the current global topological order used by runtime updates.
    pub fn schedule_topology(&self) -> &[NodeId] {
        self.runtime_schedule.topo_order()
    }

    /// Returns the node bucket for a given update rate when present.
    pub fn schedule_bucket_nodes(&self, rate_hz: NodeUpdateRate) -> Option<&[NodeId]> {
        self.runtime_schedule.bucket_nodes(rate_hz)
    }

    /// Returns the current fixed-step accumulator value.
    ///
    /// Only meaningful when `runtime_limits.fixed_step` is `Some`.
    pub fn tick_accumulator(&self) -> Duration {
        self.tick_accumulator
    }
}
