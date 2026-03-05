use std::any::type_name;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::edit::{Edit, EditQueue, EditRequest};
use crate::events::Inbox;
use crate::node::{EventSubscription, *};
use crate::process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot, ProcessTreeSnapshot};
use serde::{Deserialize, Serialize};

/// Engine callback signature used to evaluate custom reference filters.
pub type ReferenceFilterFn<T> = dyn Fn(&Engine<T>, NodeId, NodeId, NodeId) -> bool + Send + Sync;

/// Edit-application entry point and queue-drain transaction orchestration.
#[path = "engine_apply.rs"]
mod engine_apply;
/// Parameter and metadata edit application helpers.
#[path = "engine_apply_param.rs"]
mod engine_apply_param;
/// Tree mutation, attachment, and topology validation helpers.
#[path = "engine_apply_tree.rs"]
mod engine_apply_tree;
/// Unified node-type catalog and blueprint-backed dynamic type helpers.
#[path = "engine_blueprints.rs"]
mod engine_blueprints;
/// UserContext and DynamicContext integration helpers.
#[path = "engine_contexts.rs"]
mod engine_contexts;
/// Parameter control-plane runtime evaluation helpers.
#[path = "engine_controls.rs"]
mod engine_controls;
/// Event bubbling and inbox dispatch orchestration.
#[path = "engine_dispatch.rs"]
mod engine_dispatch;
/// Engine edit error type definitions.
#[path = "engine_error.rs"]
mod engine_error;
/// Undo/redo history transaction and effect models.
#[path = "engine_history.rs"]
mod engine_history;
/// Project save/load support.
#[path = "engine_persistence.rs"]
mod engine_persistence;
/// UUID reference cache helpers.
#[path = "engine_refs.rs"]
mod engine_refs;
/// Runtime resolve/scheduling and ticking orchestration.
#[path = "engine_runtime.rs"]
mod engine_runtime;
#[cfg(test)]
#[path = "engine_tests.rs"]
mod engine_tests;
/// UI-facing event outbox helpers.
#[path = "engine_ui.rs"]
mod engine_ui;

/// Node storage implementation used by the engine.
pub mod node_store;
use node_store::NodeStore;

/// Error type returned when validating or applying edits.
pub use engine_error::EngineEditError;
/// Current project file format version.
pub use engine_persistence::PROJECT_FILE_VERSION;
/// Persisted project file DTO.
pub use engine_persistence::ProjectFile;
/// Persisted node metadata DTO.
pub use engine_persistence::ProjectNodeMeta;
/// Persisted node record DTO.
pub use engine_persistence::ProjectNodeRecord;
/// Project persistence error type.
pub use engine_persistence::ProjectPersistenceError;
/// Runtime error type returned by resolve/scheduling and tick execution.
pub use engine_runtime::EngineRuntimeError;
/// Per-node execution rule returned to the runtime scheduler.
pub use engine_runtime::NodeExecutionRule;
/// Per-node update frequency in hertz.
pub use engine_runtime::NodeUpdateRate;
/// Runtime safety and scheduling limits.
pub use engine_runtime::RuntimeLimits;

/// Logical time tracked by the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
    /// Applied edit transactions available for undo.
    undo_stack: Vec<engine_history::HistoryTransaction<T>>,
    /// Undone edit transactions available for redo.
    redo_stack: Vec<engine_history::HistoryTransaction<T>>,
    /// Currently active edit session boundary.
    active_edit_session: Option<engine_history::ActiveEditSession<T>>,
    /// Runtime schedule built by `resolve()`.
    runtime_schedule: engine_runtime::ScheduleMgr,
    /// Tracks whether runtime schedule requires a resolve pass.
    runtime_resolve_pending: bool,
    /// Runtime loop guardrails.
    runtime_limits: engine_runtime::RuntimeLimits,
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
    /// Re-entrancy depth for outer structural stabilization loops.
    ///
    /// When non-zero, add-node inline stabilization is deferred to the outer pass
    /// to avoid deep recursive `apply_edits` chains.
    pub(crate) stabilization_scope_depth: usize,
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
            time: EngineTime { tick: 0, micro: 0, seq: 0 },
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
            ui_event_log_capacity: engine_ui::DEFAULT_UI_EVENT_LOG_CAPACITY,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            active_edit_session: None,
            runtime_schedule: engine_runtime::ScheduleMgr::default(),
            runtime_resolve_pending: true,
            runtime_limits: engine_runtime::RuntimeLimits::default(),
            runtime_elapsed: Duration::ZERO,
            last_update_elapsed_by_node,
            param_change_counter: 0,
            param_last_change_counter: HashMap::new(),
            expression_runtime: HashMap::new(),
            stabilization_scope_depth: 0,
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
        let parent = self.nodes.get(sibling).and_then(|n| n.node_data().parent).unwrap_or(self.root);
        self.edits.push(Edit::AddNode {
            parent,
            prev_sibling: Some(sibling),
            node: Box::new(node),
        });
    }

    /// Queues insertion of a user-curated item after an existing sibling.
    pub fn add_user_item_after(&mut self, node: T, sibling: NodeId) {
        let parent = self.nodes.get(sibling).and_then(|n| n.node_data().parent).unwrap_or(self.root);
        self.edits.push(Edit::AddUserItem {
            parent,
            prev_sibling: Some(sibling),
            node: Box::new(node),
        });
    }

    /// Queues replacement of an existing node.
    pub fn replace_node(&mut self, node: NodeId, new_node: T) {
        self.edits.push(Edit::ReplaceNode { node, new_node: Box::new(new_node) });
    }

    /// Sets or replaces the default warning on `node`.
    pub fn set_node_warning(&mut self, node: NodeId, message: impl Into<String>) {
        self.set_node_warning_with(node, None, message, None);
    }

    /// Sets or replaces one warning by id on `node`.
    ///
    /// `warning_id = None` uses the default empty warning id.
    pub fn set_node_warning_with(&mut self, node: NodeId, warning_id: Option<&str>, message: impl Into<String>, detail: Option<&str>) {
        let warning = NodeWarning {
            id: warning_id.unwrap_or_default().to_string(),
            message: message.into(),
            detail: detail.map(str::to_string),
        };

        if let Some(existing_warning) = self.nodes.get(node).and_then(|entry| entry.node_data().meta.presentation.warning(Some(warning.id.as_str()))) {
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

        self.edits.push(Edit::ClearNodeWarning { node, warning_id: warning_id.map(str::to_string) });
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
                Edit::AddNode { node, parent, prev_sibling } => {
                    let provided_node_type = node.get_type().to_string();
                    let Some(node) = T::from_boxed_node(node) else {
                        return Err(EngineEditError::NodeTypeMismatch {
                            edit_index,
                            operation: "AddNode",
                            provided_node_type,
                            expected_engine_node_type: type_name::<T>(),
                        });
                    };

                    validated_edits.push(Edit::AddNode { node: Box::new(node), parent, prev_sibling });
                }
                Edit::AddUserItem { node, parent, prev_sibling } => {
                    let provided_node_type = node.get_type().to_string();
                    let Some(node) = T::from_boxed_node(node) else {
                        return Err(EngineEditError::NodeTypeMismatch {
                            edit_index,
                            operation: "AddUserItem",
                            provided_node_type,
                            expected_engine_node_type: type_name::<T>(),
                        });
                    };

                    validated_edits.push(Edit::AddUserItem { node: Box::new(node), parent, prev_sibling });
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

                    validated_edits.push(Edit::ReplaceNode { node, new_node: Box::new(new_node) });
                }
                Edit::SetNodeWarning { node, warning } => {
                    let Some(current_presentation) = staged_presentations.get(&node).cloned().or_else(|| self.nodes.get(node).map(|entry| entry.node_data().meta.presentation.clone())) else {
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
                    let Some(current_presentation) = staged_presentations.get(&node).cloned().or_else(|| self.nodes.get(node).map(|entry| entry.node_data().meta.presentation.clone())) else {
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
                    let Some(current_presentation) = staged_presentations.get(&node).cloned().or_else(|| self.nodes.get(node).map(|entry| entry.node_data().meta.presentation.clone())) else {
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

    pub(crate) fn build_process_tree_snapshot(&self) -> Arc<ProcessTreeSnapshot> {
        let mut nodes = HashMap::with_capacity(self.nodes.iter().count());
        for (node_id, node) in self.nodes.iter() {
            let node_data = node.node_data();
            let descriptor = node.engine_script_descriptor();
            let parameter_snapshot = node.engine_param_snapshot();
            nodes.insert(
                node_id,
                ProcessTreeNodeSnapshot {
                    id: node_id,
                    parent: node_data.parent,
                    first_child: node_data.first_child,
                    next_sibling: node_data.next_sibling,
                    node_type: node.get_type().to_string(),
                    decl_id: node_data.meta.decl_id.0.clone(),
                    short_name: node_data.meta.short_name.clone(),
                    label: node_data.meta.label.clone(),
                    enabled: node_data.meta.enabled,
                    child_count: 0,
                    param_value: parameter_snapshot.as_ref().map(|snapshot| snapshot.value.clone()),
                    param_constraints: parameter_snapshot.as_ref().map(|snapshot| snapshot.constraints.clone()),
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

        Arc::new(ProcessTreeSnapshot::new(self.root, nodes))
    }
}
