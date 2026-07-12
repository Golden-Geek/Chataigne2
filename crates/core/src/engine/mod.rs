use std::any::type_name;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::edit::{Edit, EditQueue, EditRequest, NodeTree};
use crate::events::Inbox;
use crate::node::*;
use crate::parameter::ParamValue;
use crate::process_ctx::{ExecutionPhase, ProcessCtx, ProcessTreeNodeSnapshot, ProcessTreeSnapshot};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Engine callback signature used to evaluate custom reference filters.
pub type ReferenceFilterFn<T> = dyn Fn(&Engine<T>, NodeId, NodeId, NodeId) -> bool + Send + Sync;

/// Edit-application entry point and queue-drain transaction orchestration.
mod apply;
/// Parameter and metadata edit application helpers.
mod apply_param;
/// Tree mutation, attachment, and topology validation helpers.
mod apply_tree;
/// Unified node-type catalog and blueprint-backed dynamic type helpers.
mod blueprints;
/// UserContext and DynamicContext integration helpers.
mod contexts;
/// Parameter control-plane runtime evaluation helpers.
mod controls;
/// Event bubbling and inbox dispatch orchestration.
mod dispatch;
/// Engine edit error type definitions.
mod error;
/// Undo/redo history transaction and effect models.
mod history;
/// Two-way listener index for O(tree_depth) subscription routing.
mod listener_index;
#[cfg(test)]
mod param_constraints_tests;
/// Project save/load support.
mod persistence;
/// UUID reference cache helpers.
mod refs;
/// Runtime resolve/scheduling and ticking orchestration.
mod runtime;
#[cfg(test)]
mod tests;
/// Pre-allocated scratch buffers reused across tick phases.
mod tick_scratch;
/// UI-facing event outbox helpers.
mod ui;

/// Node storage implementation used by the engine.
pub mod node_store;
use node_store::NodeStore;

/// Error type returned when validating or applying edits.
pub use error::EngineEditError;
/// Current project file format version.
pub use persistence::PROJECT_FILE_VERSION;
/// Persisted project file DTO.
pub use persistence::ProjectFile;
/// One recoverable project-load problem.
pub use persistence::ProjectLoadRecoveryProblem;
/// Report of recoverable problems skipped while loading a project file.
pub use persistence::ProjectLoadRecoveryReport;
/// Stage where a recoverable project-load problem happened.
pub use persistence::ProjectLoadRecoveryStage;
/// Persisted node metadata DTO.
pub use persistence::ProjectNodeMeta;
/// Persisted node record DTO.
pub use persistence::ProjectNodeRecord;
/// Project persistence error type.
pub use persistence::ProjectPersistenceError;
pub(crate) use persistence::remap_record_uuids as remap_project_record_uuids;
/// Default runtime loop frequency cap in hertz.
pub use runtime::DEFAULT_RUNTIME_LOOP_MAX_FREQUENCY_HZ;
/// Runtime error type returned by resolve/scheduling and tick execution.
pub use runtime::EngineRuntimeError;
/// Fixed-step accumulator configuration for `run_for` / `run_loop`.
pub use runtime::FixedStepConfig;
/// Per-node execution rule returned to the runtime scheduler.
pub use runtime::NodeExecutionRule;
/// Per-node update frequency in hertz.
pub use runtime::NodeUpdateRate;
/// Runtime safety and scheduling limits.
pub use runtime::RuntimeLimits;
/// Converts a frequency cap in hertz to a runtime loop interval.
pub use runtime::runtime_loop_interval_for_frequency_hz;
/// Per-tick performance counters returned by `Engine::tick_stats`.
pub use tick_scratch::TickStats;

/// Logical time tracked by the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
pub struct EngineTime {
    /// Monotonic engine tick counter. Increments only on EngineTick.
    pub tick: u64,

    /// Micro-step index within the same tick.
    /// 0 = main tick pass, 1.. = stabilisation rounds or flushImmediate rounds within that same tick.
    pub micro: u32,

    /// Total ordering within the same (tick, micro).
    pub seq: u32,
}

#[derive(Clone, Default)]
pub(crate) struct ExpressionControlRuntime {
    pub source_param: Option<NodeId>,
    pub dependencies: HashSet<NodeId>,
    pub subscriptions: HashSet<NodeId>,
    pub continuous: bool,
    pub last_eval_elapsed: Duration,
    pub source_expression: String,
    pub script_runtime: Option<Arc<Mutex<Box<dyn crate::script::ScriptRuntime>>>>,
}

/// Node engine storing graph state, pending edits, and emitted events.
pub struct Engine<T: Node> {
    /// Backing node store indexed by stable node ids.
    pub nodes: NodeStore<T>,
    /// Root node id.
    pub root: NodeId,
    /// Current engine logical time.
    pub time: EngineTime,
    /// Engine-owned event stream.
    pub inbox: Inbox,
    /// Pending edits to be applied.
    pub edits: EditQueue,
    /// Cross-thread sender used by external producers to enqueue edits.
    external_edits_tx: Sender<Edit>,
    /// Cross-thread receiver drained by the engine before edit application.
    external_edits_rx: Receiver<Edit>,
    /// Runtime listener subscriptions — two-way index for O(tree_depth) routing.
    pub(crate) event_listeners: listener_index::ListenerIndex,
    /// App-registered reference filters keyed by `ReferenceConstraints.custom_filter_key`.
    reference_filters: HashMap<String, Box<ReferenceFilterFn<T>>>,
    /// Unified catalog registry for blueprint-backed dynamic node types.
    blueprints: crate::blueprints::BlueprintRegistry<T>,
    /// User-defined lexical context scopes and resolver cache.
    user_contexts: crate::contexts::UserContextRegistry,
    /// Persistent node UUID -> runtime node id lookup maintained with node-store mutations.
    uuid_index: HashMap<NodeUuid, NodeId>,
    /// UI-facing append-only event log used for replay/subscription.
    ui_event_log: Vec<crate::events::Event>,
    /// Start index of retained events inside `ui_event_log`.
    ui_event_log_start: usize,
    /// Maximum number of events retained in `ui_event_log`.
    ui_event_log_capacity: usize,
    /// Project epoch used by UI graph transactions.
    ui_epoch: u64,
    /// Next UI graph transaction id within `ui_epoch`.
    next_ui_tx_id: u64,
    /// Monotonic UI graph version advanced once per graph transaction.
    ui_graph_version: u64,
    /// Logger sync cursor for projecting retained logger state into UI events.
    last_synced_logger_record_id: u64,
    /// Last repeat count observed for the synced logger cursor record.
    last_synced_logger_repeat_count: u32,
    /// Applied edit transactions available for undo.
    undo_stack: Vec<history::HistoryTransaction<T>>,
    /// Undone edit transactions available for redo.
    redo_stack: Vec<history::HistoryTransaction<T>>,
    /// Logical content-state id for the current graph relative to undo/redo history.
    current_history_state_id: u64,
    /// Next unique content-state id assigned to a history-visible graph state.
    next_history_state_id: u64,
    /// Currently active edit session boundary.
    active_edit_session: Option<history::ActiveEditSession<T>>,
    /// Runtime schedule built by `resolve()`.
    runtime_schedule: runtime::ScheduleMgr,
    /// Tracks whether runtime schedule requires a resolve pass.
    runtime_resolve_pending: bool,
    /// Runtime loop guardrails.
    runtime_limits: runtime::RuntimeLimits,
    /// Accumulated wall-clock runtime elapsed while ticking.
    runtime_elapsed: Duration,
    /// Last runtime timestamp at which each node received an update callback.
    last_update_elapsed_by_node: HashMap<NodeId, Duration>,
    /// Monotonic counter incremented on each successful parameter write.
    param_change_counter: u64,
    /// Last change counter observed for each parameter node.
    param_last_change_counter: HashMap<NodeId, u64>,
    /// Runtime state for expression-controlled parameters.
    expression_runtime: HashMap<NodeId, ExpressionControlRuntime>,
    /// Node-ready callbacks deferred by persistence until a host starts the loaded graph.
    pending_node_ready: Vec<(NodeId, NodeCreationContext)>,
    /// Re-entrancy depth for outer structural stabilization loops.
    ///
    /// When non-zero, add-node inline stabilization is deferred to the outer pass
    /// to avoid deep recursive `apply_edits` chains.
    pub(crate) stabilization_scope_depth: usize,
    /// Stable iteration list of parameter nodes with an active control or pending diagnostics.
    pub(crate) active_control_params: Vec<NodeId>,
    /// Membership companion for `active_control_params`.
    pub(crate) active_control_param_set: HashSet<NodeId>,
    /// Source parameter -> controlled parameter dependents, rebuilt with the active-control index.
    pub(crate) control_source_dependents: HashMap<NodeId, Vec<NodeId>>,
    /// Marks the active-control index stale after structure or control configuration changes.
    pub(crate) control_index_dirty: bool,
    /// Tree snapshot built at most once per tick and reused across all `CallNodeMutation`
    /// edits in that tick. Cleared at tick start and on any structural change.
    pub(crate) tick_tree_snapshot: Option<Arc<ProcessTreeSnapshot>>,
    /// Cached map of parameter node → current value, used by `run_scheduled_updates` so
    /// N due nodes share the same resolution table rather than rebuilding per node.
    ///
    /// INVALIDATED BY:
    ///   - AddNode / AddNodeTree / AddUserItem: `populate_param_cache_entry` after insert
    ///   - RemoveNode: `purge_param_cache_entry` for each node in subtree
    ///   - ReplaceNode: purge then populate around the in-place swap
    ///   - SetParam / SetParamConstraints: updated in `apply_set_param` and `emit_param_events_for_state_change`
    ///   - History undo/redo: populate/purge alongside every `nodes.reattach` / `nodes.detach`
    ///   - NodeCreated/NodeDeleted events during absorb_edits: scanned in run_scheduled_updates and dispatch
    pub(crate) parameter_values_cache: HashMap<NodeId, ParamValue>,
    /// Pre-allocated scratch buffers reused across tick phases to avoid per-tick heap allocations.
    ///
    /// INVALIDATED BY: cleared at the start of each phase that uses it; never persisted.
    pub(crate) tick_scratch: tick_scratch::TickScratch,
    /// Leftover wall-clock time not yet consumed by a full logical step.
    ///
    /// Only meaningful when `runtime_limits.fixed_step` is `Some`.
    /// Carries over between `run_for` calls to preserve sub-step precision.
    tick_accumulator: Duration,
    /// Number of frames where wall-clock elapsed exceeded `FixedStepConfig::max_catchup`.
    ///
    /// Each such frame is clamped, losing time to prevent the spiral-of-death.
    /// Only incremented when `runtime_limits.fixed_step` is `Some`.
    pub late_ticks: u64,
    /// Structural edits (AddNode, RemoveNode, MoveNode, etc.) queued during stabilization rounds.
    ///
    /// Stabilization must not apply structural edits — they change graph topology, which would
    /// reset the schedule and extend stabilization unpredictably. Structural edits that originate
    /// inside stabilization are stashed here and applied at the start of the next `run_tick`.
    ///
    /// INVALIDATED BY: drained into `edits.pending` at the start of each `run_tick`.
    pub(crate) deferred_structural_edits: Vec<crate::edit::Edit>,
    /// Last logical tick that emitted a user-visible performance warning.
    last_performance_log_tick: Option<u64>,
}

impl<T: Node> Engine<T> {
    /// Creates a new engine with `root` as the graph root node.
    pub fn new(root: T) -> Self {
        let mut nodes: NodeStore<T> = NodeStore::new();
        let root = nodes.insert(root);
        let mut uuid_index = HashMap::new();
        if let Some(node) = nodes.get(root) {
            uuid_index.insert(node.node_data().meta.uuid, root);
        }
        let mut parameter_values_cache = HashMap::new();
        if let Some(snapshot) = nodes.get(root).and_then(Node::engine_param_snapshot) {
            parameter_values_cache.insert(root, snapshot.value);
        }
        let mut last_update_elapsed_by_node = HashMap::new();
        last_update_elapsed_by_node.insert(root, Duration::ZERO);
        let (external_edits_tx, external_edits_rx) = mpsc::channel();

        let mut engine = Self {
            nodes,
            root,
            time: EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
            inbox: Inbox::new(),
            edits: EditQueue::new(),
            external_edits_tx,
            external_edits_rx,
            event_listeners: listener_index::ListenerIndex::default(),
            reference_filters: HashMap::new(),
            blueprints: crate::blueprints::BlueprintRegistry::new(),
            user_contexts: crate::contexts::UserContextRegistry::new(),
            uuid_index,
            ui_event_log: Vec::new(),
            ui_event_log_start: 0,
            ui_event_log_capacity: ui::DEFAULT_UI_EVENT_LOG_CAPACITY,
            ui_epoch: 0,
            next_ui_tx_id: 1,
            ui_graph_version: 0,
            last_synced_logger_record_id: 0,
            last_synced_logger_repeat_count: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_history_state_id: 0,
            next_history_state_id: 1,
            active_edit_session: None,
            runtime_schedule: runtime::ScheduleMgr::default(),
            runtime_resolve_pending: true,
            runtime_limits: runtime::RuntimeLimits::default(),
            runtime_elapsed: Duration::ZERO,
            last_update_elapsed_by_node,
            param_change_counter: 0,
            param_last_change_counter: HashMap::new(),
            expression_runtime: HashMap::new(),
            pending_node_ready: Vec::new(),
            stabilization_scope_depth: 0,
            active_control_params: Vec::new(),
            active_control_param_set: HashSet::new(),
            control_source_dependents: HashMap::new(),
            control_index_dirty: true,
            tick_tree_snapshot: None,
            parameter_values_cache,
            tick_scratch: tick_scratch::TickScratch::default(),
            tick_accumulator: Duration::ZERO,
            late_ticks: 0,
            deferred_structural_edits: Vec::new(),
            last_performance_log_tick: None,
        };
        engine.sync_missing_reference_warnings_silent();
        engine.rebuild_user_context_registry_from_nodes();
        engine
    }

    /// Queues insertion of a node under `parent` (or root when `None`).
    pub fn add_node(&mut self, node: T, parent: Option<NodeId>) {
        self.edits.push(Edit::AddNode {
            parent: parent.unwrap_or(self.root),
            node: Box::new(node),
            prev_sibling: None,
        });
    }

    /// Queues insertion of a user-curated item under `parent` (or root when `None`).
    pub fn add_user_item(&mut self, node: T, parent: Option<NodeId>) {
        self.edits.push(Edit::AddUserItem {
            parent: parent.unwrap_or(self.root),
            node: Box::new(node),
            prev_sibling: None,
        });
    }

    /// Queues insertion of a user-curated item subtree under `parent` (or root when `None`).
    pub fn add_user_item_tree(&mut self, tree: NodeTree, parent: Option<NodeId>) {
        self.edits.push(Edit::AddUserItemTree {
            parent: parent.unwrap_or(self.root),
            tree,
            prev_sibling: None,
        });
    }

    /// Queues insertion of a node after an existing sibling.
    pub fn add_node_after(&mut self, node: T, sibling: NodeId) {
        let parent = self
            .nodes
            .get(sibling)
            .and_then(|n| n.node_data().parent)
            .unwrap_or(self.root);
        self.edits.push(Edit::AddNode {
            parent,
            prev_sibling: Some(sibling),
            node: Box::new(node),
        });
    }

    /// Queues insertion of a user-curated item after an existing sibling.
    pub fn add_user_item_after(&mut self, node: T, sibling: NodeId) {
        let parent = self
            .nodes
            .get(sibling)
            .and_then(|n| n.node_data().parent)
            .unwrap_or(self.root);
        self.edits.push(Edit::AddUserItem {
            parent,
            prev_sibling: Some(sibling),
            node: Box::new(node),
        });
    }

    /// Queues insertion of a user-curated item subtree after an existing sibling.
    pub fn add_user_item_tree_after(&mut self, tree: NodeTree, sibling: NodeId) {
        let parent = self
            .nodes
            .get(sibling)
            .and_then(|n| n.node_data().parent)
            .unwrap_or(self.root);
        self.edits.push(Edit::AddUserItemTree {
            parent,
            prev_sibling: Some(sibling),
            tree,
        });
    }

    /// Queues replacement of an existing node.
    pub fn replace_node(&mut self, node: NodeId, new_node: T) {
        self.edits.push(Edit::ReplaceNode {
            node,
            new_node: Box::new(new_node),
        });
    }

    /// Sets or replaces the default warning on `node`.
    pub fn set_node_warning(&mut self, node: NodeId, message: impl Into<String>) {
        self.set_node_warning_with(node, None, message, None);
    }

    /// Sets or replaces one warning by id on `node`.
    ///
    /// `warning_id = None` uses the default empty warning id.
    pub fn set_node_warning_with(
        &mut self,
        node: NodeId,
        warning_id: Option<&str>,
        message: impl Into<String>,
        detail: Option<&str>,
    ) {
        let warning = NodeWarning {
            id: warning_id.unwrap_or_default().to_string(),
            message: message.into(),
            detail: detail.map(str::to_string),
        };

        if let Some(existing_warning) = self
            .nodes
            .get(node)
            .and_then(|entry| entry.node_data().meta.presentation.warning(Some(warning.id.as_str())))
            && existing_warning == &warning
        {
            return;
        }

        self.edits.push(Edit::SetNodeWarning { node, warning });
    }

    /// Clears one warning by id on `node`.
    ///
    /// `warning_id = None` clears all warnings on `node`.
    pub fn clear_node_warning(&mut self, node: NodeId, warning_id: Option<&str>) {
        if let Some(entry) = self.nodes.get(node) {
            let presentation = &entry.node_data().meta.presentation;
            let has_target_warning = match warning_id {
                Some(id) => presentation.warning(Some(id)).is_some(),
                None => !presentation.warnings.is_empty(),
            };
            if !has_target_warning {
                return;
            }
        }

        self.edits.push(Edit::ClearNodeWarning {
            node,
            warning_id: warning_id.map(str::to_string),
        });
    }

    /// Clears all warnings on `node`.
    pub fn clear_all_node_warnings(&mut self, node: NodeId) {
        self.clear_node_warning(node, None);
    }

    /// Sets child warning surfacing depth for `node`.
    pub fn set_node_child_warning_depth(&mut self, node: NodeId, max_depth: u32) {
        if let Some(entry) = self.nodes.get(node)
            && entry.node_data().meta.presentation.show_child_warnings_max_depth == max_depth
        {
            return;
        }

        self.edits.push(Edit::SetNodeChildWarningDepth { node, max_depth });
    }

    /// Returns a cloneable sender for queuing edits from external threads/tasks.
    ///
    /// Edits sent through this channel are merged into `self.edits` during
    /// `apply_edits()` and runtime tick stabilization passes.
    pub fn external_edit_sender(&self) -> Sender<Edit> {
        self.external_edits_tx.clone()
    }

    /// Registers a custom reference filter callback under `key`.
    ///
    /// The callback receives `(engine, parameter_node_id, root_node_id, candidate_node_id)`.
    pub fn register_reference_filter<F>(&mut self, key: impl Into<String>, filter: F)
    where
        F: Fn(&Engine<T>, NodeId, NodeId, NodeId) -> bool + Send + Sync + 'static,
    {
        self.reference_filters.insert(key.into(), Box::new(filter));
    }

    /// Removes a custom reference filter callback.
    pub fn unregister_reference_filter(&mut self, key: &str) -> Option<Box<ReferenceFilterFn<T>>> {
        self.reference_filters.remove(key)
    }

    /// Drains all externally queued edits into the engine edit queue.
    ///
    /// Returns how many external edit messages were drained.
    pub fn absorb_external_edits(&mut self) -> Result<usize, EngineEditError> {
        let mut queued_requests = Vec::new();
        while let Ok(edit) = self.external_edits_rx.try_recv() {
            queued_requests.push(EditRequest { edit });
        }

        let drained = queued_requests.len();
        self.absorb_edit_requests(queued_requests)?;
        Ok(drained)
    }

    /// Moves edits from a processing context into the engine queue.
    ///
    /// Node-bearing edits are validated to ensure node types match `T`.
    pub fn absorb_edits(&mut self, ctx: &mut ProcessCtx) -> Result<(), EngineEditError> {
        self.absorb_edit_requests(ctx.edits.drain())
    }

    fn absorb_edit_requests(&mut self, requests: Vec<EditRequest>) -> Result<(), EngineEditError> {
        let mut validated_edits = Vec::new();
        let mut staged_presentations: HashMap<NodeId, PresentationHint> = HashMap::new();

        for (edit_index, request) in requests.into_iter().enumerate() {
            match request.edit {
                Edit::AddNode {
                    node,
                    parent,
                    prev_sibling,
                } => {
                    let provided_node_type = node.get_type().to_string();
                    let Some(node) = T::from_boxed_node(node) else {
                        return Err(EngineEditError::NodeTypeMismatch {
                            edit_index,
                            operation: "AddNode",
                            provided_node_type,
                            expected_engine_node_type: type_name::<T>(),
                        });
                    };

                    validated_edits.push(Edit::AddNode {
                        node: Box::new(node),
                        parent,
                        prev_sibling,
                    });
                }
                Edit::AddNodeTree {
                    tree,
                    parent,
                    prev_sibling,
                } => {
                    let tree = self.coerce_node_tree_for_engine(edit_index, "AddNodeTree", tree)?;
                    validated_edits.push(Edit::AddNodeTree {
                        tree,
                        parent,
                        prev_sibling,
                    });
                }
                Edit::AddUserItemTree {
                    tree,
                    parent,
                    prev_sibling,
                } => {
                    let tree = self.coerce_node_tree_for_engine(edit_index, "AddUserItemTree", tree)?;
                    validated_edits.push(Edit::AddUserItemTree {
                        tree,
                        parent,
                        prev_sibling,
                    });
                }
                Edit::AddUserItem {
                    node,
                    parent,
                    prev_sibling,
                } => {
                    let provided_node_type = node.get_type().to_string();
                    let Some(node) = T::from_boxed_node(node) else {
                        return Err(EngineEditError::NodeTypeMismatch {
                            edit_index,
                            operation: "AddUserItem",
                            provided_node_type,
                            expected_engine_node_type: type_name::<T>(),
                        });
                    };

                    validated_edits.push(Edit::AddUserItem {
                        node: Box::new(node),
                        parent,
                        prev_sibling,
                    });
                }
                Edit::ReplaceNode { node, new_node } => {
                    let provided_node_type = new_node.get_type().to_string();
                    let Some(new_node) = T::from_boxed_node(new_node) else {
                        return Err(EngineEditError::NodeTypeMismatch {
                            edit_index,
                            operation: "ReplaceNode",
                            provided_node_type,
                            expected_engine_node_type: type_name::<T>(),
                        });
                    };

                    validated_edits.push(Edit::ReplaceNode {
                        node,
                        new_node: Box::new(new_node),
                    });
                }
                Edit::SetNodeWarning { node, warning } => {
                    let Some(current_presentation) = staged_presentations.get(&node).cloned().or_else(|| {
                        self.nodes
                            .get(node)
                            .map(|entry| entry.node_data().meta.presentation.clone())
                    }) else {
                        validated_edits.push(Edit::SetNodeWarning { node, warning });
                        continue;
                    };

                    let mut next_presentation = current_presentation.clone();
                    next_presentation.set_warning(warning.clone());
                    if next_presentation == current_presentation {
                        continue;
                    }

                    staged_presentations.insert(node, next_presentation);
                    validated_edits.push(Edit::SetNodeWarning { node, warning });
                }
                Edit::ClearNodeWarning { node, warning_id } => {
                    let Some(current_presentation) = staged_presentations.get(&node).cloned().or_else(|| {
                        self.nodes
                            .get(node)
                            .map(|entry| entry.node_data().meta.presentation.clone())
                    }) else {
                        validated_edits.push(Edit::ClearNodeWarning { node, warning_id });
                        continue;
                    };

                    let mut next_presentation = current_presentation.clone();
                    let changed = match warning_id.as_deref() {
                        Some(id) => next_presentation.clear_warning(Some(id)),
                        None => next_presentation.clear_warnings(),
                    };
                    if !changed {
                        continue;
                    }

                    staged_presentations.insert(node, next_presentation);
                    validated_edits.push(Edit::ClearNodeWarning { node, warning_id });
                }
                Edit::SetNodeChildWarningDepth { node, max_depth } => {
                    let Some(current_presentation) = staged_presentations.get(&node).cloned().or_else(|| {
                        self.nodes
                            .get(node)
                            .map(|entry| entry.node_data().meta.presentation.clone())
                    }) else {
                        validated_edits.push(Edit::SetNodeChildWarningDepth { node, max_depth });
                        continue;
                    };

                    let mut next_presentation = current_presentation.clone();
                    if !next_presentation.set_child_warning_depth(max_depth) {
                        continue;
                    }

                    staged_presentations.insert(node, next_presentation);
                    validated_edits.push(Edit::SetNodeChildWarningDepth { node, max_depth });
                }
                edit => validated_edits.push(edit),
            }
        }

        for edit in validated_edits {
            self.edits.push(edit);
        }

        Ok(())
    }

    fn coerce_node_tree_for_engine(
        &self,
        edit_index: usize,
        operation: &'static str,
        tree: NodeTree,
    ) -> Result<NodeTree, EngineEditError> {
        let provided_node_type = tree.node.get_type().to_string();
        let Some(node) = T::from_boxed_node(tree.node) else {
            return Err(EngineEditError::NodeTypeMismatch {
                edit_index,
                operation,
                provided_node_type,
                expected_engine_node_type: type_name::<T>(),
            });
        };

        let mut coerced = NodeTree::new(node);
        for child in tree.children {
            coerced.push_child(self.coerce_node_tree_for_engine(edit_index, operation, child)?);
        }
        Ok(coerced)
    }

    /// Returns per-tick counters accumulated during the most recent `run_tick`.
    ///
    /// Counters are reset at the start of each `run_tick` and updated by engine internals.
    pub fn tick_stats(&self) -> tick_scratch::TickStats {
        self.tick_scratch.stats
    }

    /// Returns a cached snapshot for the current tick, building it on first call.
    /// Used by `apply_call_node_mutation` so N mutations share one build per tick.
    pub(crate) fn get_or_build_tick_snapshot(&mut self) -> Arc<ProcessTreeSnapshot> {
        if let Some(snapshot) = &self.tick_tree_snapshot {
            return Arc::clone(snapshot);
        }
        self.tick_scratch.stats.snapshot_rebuilds += 1;
        self.tick_scratch.stats.snapshot_builds += 1;
        self.tick_scratch.stats.snapshot_nodes_cloned += self.nodes.len();
        let started = Instant::now();
        let snapshot = self.build_process_tree_snapshot();
        self.tick_scratch.stats.snapshot_build_ns += started.elapsed().as_nanos();
        self.tick_tree_snapshot = Some(Arc::clone(&snapshot));
        snapshot
    }

    pub(crate) fn build_process_tree_snapshot(&self) -> Arc<ProcessTreeSnapshot> {
        let mut nodes = HashMap::with_capacity(self.nodes.len());
        for (node_id, node) in self.nodes.iter() {
            let node_data = node.node_data();
            let descriptor = node.engine_script_descriptor();
            let parameter_snapshot = node.engine_param_snapshot();
            let dashboard_widget_target = node.engine_dashboard_widget_target_descriptor();
            nodes.insert(
                node_id,
                ProcessTreeNodeSnapshot {
                    id: node_id,
                    uuid: node_data.meta.uuid,
                    parent: node_data.parent,
                    first_child: node_data.first_child,
                    next_sibling: node_data.next_sibling,
                    node_type: node.get_type().to_string(),
                    decl_id: node_data.meta.decl_id.0.clone(),
                    short_name: node_data.meta.short_name.clone(),
                    label: node_data.meta.label.clone(),
                    tags: node_data.meta.tags.clone(),
                    presentation: node_data.meta.presentation.clone(),
                    enabled: node_data.meta.enabled,
                    can_be_disabled: node_data.meta.can_be_disabled,
                    child_count: 0,
                    param_value: parameter_snapshot.as_ref().map(|snapshot| snapshot.value.clone()),
                    param_constraints: parameter_snapshot.as_ref().map(|snapshot| snapshot.constraints.clone()),
                    param_control: parameter_snapshot.as_ref().map(|snapshot| snapshot.control.clone()),
                    dashboard_widget_target,
                    script_properties: descriptor.properties,
                    script_methods: descriptor.methods,
                },
            );
        }

        let parent_ids: Vec<NodeId> = nodes.values().filter_map(|node| node.parent).collect();
        for parent_id in parent_ids {
            if let Some(parent) = nodes.get_mut(&parent_id) {
                parent.child_count = parent.child_count.saturating_add(1);
            }
        }

        if nodes.contains_key(&self.root) {
            let mut stack = vec![(self.root, true)];
            let mut visited = HashSet::<NodeId>::new();
            while let Some((node_id, ancestors_enabled)) = stack.pop() {
                if !visited.insert(node_id) {
                    continue;
                }

                let (first_child, effective_enabled) = match nodes.get(&node_id) {
                    Some(node) => (node.first_child, ancestors_enabled && node.enabled),
                    None => continue,
                };

                if let Some(node) = nodes.get_mut(&node_id) {
                    node.enabled = effective_enabled;
                }

                let mut child = first_child;
                let mut sibling_chain = HashSet::<NodeId>::new();
                while let Some(child_id) = child {
                    if !sibling_chain.insert(child_id) {
                        break;
                    }
                    let next_sibling = nodes.get(&child_id).and_then(|node| node.next_sibling);
                    if nodes.contains_key(&child_id) {
                        stack.push((child_id, effective_enabled));
                    }
                    child = next_sibling;
                }
            }
        }

        Arc::new(ProcessTreeSnapshot::new(self.root, nodes))
    }

    /// Builds a read-only snapshot of the current node tree.
    ///
    /// This is the supported boundary for compilers and exporters that consume
    /// canonical Golden Core node subtrees without taking ownership of the engine.
    pub fn process_tree_snapshot(&self) -> Arc<ProcessTreeSnapshot> {
        self.build_process_tree_snapshot()
    }

    pub(crate) fn is_effectively_enabled(&self, node: NodeId) -> bool {
        let mut current = Some(node);
        while let Some(node_id) = current {
            let Some(entry) = self.nodes.get(node_id) else {
                return false;
            };
            if !entry.node_data().meta.enabled {
                return false;
            }
            current = entry.node_data().parent;
        }

        true
    }

    pub(crate) fn collect_subtree_node_ids(&self, root: NodeId) -> Vec<NodeId> {
        if !self.nodes.contains(root) {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(node_id) = stack.pop() {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };

            out.push(node_id);

            let mut child = node.node_data().first_child;
            while let Some(child_id) = child {
                let next_sibling = self
                    .nodes
                    .get(child_id)
                    .and_then(|entry| entry.node_data().next_sibling);
                stack.push(child_id);
                child = next_sibling;
            }
        }

        out
    }

    pub(crate) fn queue_effective_enabled_callbacks(
        &mut self,
        changes: &[(NodeId, bool)],
    ) -> Result<(), EngineEditError> {
        if changes.is_empty() {
            return Ok(());
        }

        let tree_snapshot = self.build_process_tree_snapshot();
        for (node_id, enabled) in changes {
            let Some(node) = self.nodes.get_mut(*node_id) else {
                continue;
            };

            node.node_data_mut().effective_enabled = *enabled;

            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, self.time);
            ctx.runtime_elapsed = self.runtime_elapsed;
            ctx.set_tree_snapshot(Arc::clone(&tree_snapshot));

            crate::logger::with_node_origin(*node_id, || {
                node.on_effective_enabled_changed(&mut ctx, *enabled);
            });

            self.absorb_edits(&mut ctx)?;
        }

        Ok(())
    }
}
