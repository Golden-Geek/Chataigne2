use std::any::type_name;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::edit::{Edit, EditQueue, EditRequest, NodeTree};
use crate::events::Inbox;
use crate::node::{EventSubscription, *};
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
/// Persisted node metadata DTO.
pub use persistence::ProjectNodeMeta;
/// Persisted node record DTO.
pub use persistence::ProjectNodeRecord;
/// Project persistence error type.
pub use persistence::ProjectPersistenceError;
/// Runtime error type returned by resolve/scheduling and tick execution.
pub use runtime::EngineRuntimeError;
/// Per-node execution rule returned to the runtime scheduler.
pub use runtime::NodeExecutionRule;
/// Per-node update frequency in hertz.
pub use runtime::NodeUpdateRate;
/// Runtime safety and scheduling limits.
pub use runtime::RuntimeLimits;

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
    /// Runtime listener subscriptions keyed by subscriber node id.
    pub event_listeners: HashMap<NodeId, HashSet<EventSubscription>>,
    /// App-registered reference filters keyed by `ReferenceConstraints.custom_filter_key`.
    reference_filters: HashMap<String, Box<ReferenceFilterFn<T>>>,
    /// Unified catalog registry for blueprint-backed dynamic node types.
    blueprints: crate::blueprints::BlueprintRegistry<T>,
    /// User-defined lexical context scopes and resolver cache.
    user_contexts: crate::contexts::UserContextRegistry,
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
    /// Cached answer to "does any parameter have a non-Manual control or diagnostics?"
    /// Set to `None` whenever the tree structure or a control state changes, triggering
    /// a full rescan on the next `evaluate_parameter_controls` call.
    pub(crate) has_active_controls_cache: Option<bool>,
    /// Tree snapshot built at most once per tick and reused across all `CallNodeMutation`
    /// edits in that tick. Cleared at tick start and on any structural change.
    pub(crate) tick_tree_snapshot: Option<Arc<ProcessTreeSnapshot>>,
    /// Cached map of parameter node → current value, used by `run_scheduled_updates` so
    /// N due nodes share one O(N_nodes) scan per tick rather than one per node.
    /// Maintained incrementally via `apply_set_param`; rebuilt from scratch when
    /// `parameter_values_dirty` is true (set by `mark_schedule_dirty`).
    pub(crate) parameter_values_cache: HashMap<NodeId, ParamValue>,
    pub(crate) parameter_values_dirty: bool,
}

impl<T: Node> Engine<T> {
    /// Creates a new engine with `root` as the graph root node.
    pub fn new(root: T) -> Self {
        let mut nodes: NodeStore<T> = NodeStore::new();
        let root = nodes.insert(root);
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
            event_listeners: HashMap::new(),
            reference_filters: HashMap::new(),
            blueprints: crate::blueprints::BlueprintRegistry::new(),
            user_contexts: crate::contexts::UserContextRegistry::new(),
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
            has_active_controls_cache: None,
            tick_tree_snapshot: None,
            parameter_values_cache: HashMap::new(),
            parameter_values_dirty: true,
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
        {
            if existing_warning == &warning {
                return;
            }
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
        if let Some(entry) = self.nodes.get(node) {
            if entry.node_data().meta.presentation.show_child_warnings_max_depth == max_depth {
                return;
            }
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

    /// Returns a cached snapshot for the current tick, building it on first call.
    /// Used by `apply_call_node_mutation` so N mutations share one build per tick.
    pub(crate) fn get_or_build_tick_snapshot(&mut self) -> Arc<ProcessTreeSnapshot> {
        if let Some(snapshot) = &self.tick_tree_snapshot {
            return Arc::clone(snapshot);
        }
        let snapshot = self.build_process_tree_snapshot();
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
                    enabled: node_data.meta.enabled,
                    can_be_disabled: node_data.meta.can_be_disabled,
                    child_count: 0,
                    param_value: parameter_snapshot.as_ref().map(|snapshot| snapshot.value.clone()),
                    param_constraints: parameter_snapshot.as_ref().map(|snapshot| snapshot.constraints.clone()),
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
            while let Some((node_id, ancestors_enabled)) = stack.pop() {
                let (first_child, effective_enabled) = match nodes.get(&node_id) {
                    Some(node) => (node.first_child, ancestors_enabled && node.enabled),
                    None => continue,
                };

                if let Some(node) = nodes.get_mut(&node_id) {
                    node.enabled = effective_enabled;
                }

                let mut child = first_child;
                while let Some(child_id) = child {
                    let next_sibling = nodes.get(&child_id).and_then(|node| node.next_sibling);
                    stack.push((child_id, effective_enabled));
                    child = next_sibling;
                }
            }
        }

        Arc::new(ProcessTreeSnapshot::new(self.root, nodes))
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
