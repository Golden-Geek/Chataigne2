use std::time::Duration;

use crate::node::NodeId;

/// Per-node update frequency in hertz.
pub type NodeUpdateRate = u32;

/// Default cap used by raw engine runtime loops when no app preference overrides it.
pub const DEFAULT_RUNTIME_LOOP_MAX_FREQUENCY_HZ: NodeUpdateRate = 1_000;

/// Converts a frequency cap in hertz to the minimum interval between runtime loop iterations.
pub fn runtime_loop_interval_for_frequency_hz(frequency_hz: NodeUpdateRate) -> Duration {
    let frequency_hz = frequency_hz.max(1);
    Duration::from_nanos((1_000_000_000u64 / u64::from(frequency_hz)).max(1))
}

/// Configuration for the fixed-step accumulator used by `run_for` / `run_loop`.
///
/// When present in `RuntimeLimits`, wall-clock time is accumulated and drained
/// in exact `step`-sized logical ticks rather than passing raw frame elapsed to `run_tick`.
/// This decouples logical timing precision from frame-rate jitter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedStepConfig {
    /// Logical tick size, e.g. `Duration::from_micros(5_000)` for 200 Hz.
    pub step: Duration,
    /// Maximum wall-clock time absorbed per frame before clamping.
    ///
    /// Prevents the spiral-of-death when a frame takes far longer than `step`.
    /// Frames that exceed this limit increment `Engine::late_ticks`.
    pub max_catchup: Duration,
}

impl Default for FixedStepConfig {
    fn default() -> Self {
        Self {
            step: Duration::from_micros(5_000), // 200 Hz
            max_catchup: Duration::from_millis(100),
        }
    }
}

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
        Self {
            dependencies: Vec::new(),
            update_rate: Some(rate_hz),
        }
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
        Self {
            dependencies: Vec::new(),
            update_rate: None,
        }
    }
}

/// Runtime guardrails used while ticking the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeLimits {
    /// Maximum number of entries allowed on one user-context multiplex axis.
    pub max_user_context_items_per_axis: usize,
    /// Maximum number of new nodes accepted from one multiplex reconcile request.
    pub max_user_context_new_nodes_per_reconcile: usize,
    /// Maximum structural multiplex reconcile edits queued in one engine tick.
    pub max_user_context_reconcile_edits_per_tick: usize,
    /// Maximum number of stabilization rounds allowed in one tick.
    pub max_stabilization_passes_per_tick: usize,
    /// Maximum number of update callbacks allowed in one tick.
    pub max_update_callbacks_per_tick: usize,
    /// Maximum catch-up firings processed per bucket in one tick.
    pub max_bucket_catch_up_per_tick: u32,
    /// Maximum raw runtime loop frequency in hertz.
    pub max_loop_frequency_hz: NodeUpdateRate,
    /// When `Some`, `run_for` and `run_loop` use the fixed-step accumulator pattern:
    /// wall-clock time is accumulated and drained in exact `step`-sized logical ticks.
    ///
    /// When `None` (default), raw frame elapsed is passed directly to `run_tick`.
    pub fixed_step: Option<FixedStepConfig>,
}

impl RuntimeLimits {
    /// Returns the sleep interval implied by `max_loop_frequency_hz`.
    pub fn loop_cap_interval(&self) -> Duration {
        runtime_loop_interval_for_frequency_hz(self.max_loop_frequency_hz)
    }
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_user_context_items_per_axis: 4_096,
            max_user_context_new_nodes_per_reconcile: 16_384,
            max_user_context_reconcile_edits_per_tick: 256,
            max_stabilization_passes_per_tick: 16,
            max_update_callbacks_per_tick: 100_000,
            max_bucket_catch_up_per_tick: 4,
            max_loop_frequency_hz: DEFAULT_RUNTIME_LOOP_MAX_FREQUENCY_HZ,
            fixed_step: None,
        }
    }
}
