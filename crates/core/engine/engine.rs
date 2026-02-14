use std::any::type_name;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use crate::edit::{Edit, EditQueue, EditRequest};
use crate::events::Inbox;
use crate::node::{EventSubscription, *};
use crate::process_ctx::ProcessCtx;
use serde::{Deserialize, Serialize};

/// Edit-application entry point and queue-drain transaction orchestration.
#[path = "engine_apply.rs"]
mod engine_apply;
/// Parameter and metadata edit application helpers.
#[path = "engine_apply_param.rs"]
mod engine_apply_param;
/// Tree mutation, attachment, and topology validation helpers.
#[path = "engine_apply_tree.rs"]
mod engine_apply_tree;
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
    /// UI-facing append-only event log used for replay/subscription.
    ui_event_log: Vec<crate::events::Event>,
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
}

impl<T: Node> Engine<T> {
    /// Creates a new engine with `root` as the graph root node.
    pub fn new(root: T) -> Self {
        let mut nodes: NodeStore<T> = NodeStore::new();
        let root = nodes.insert(root);
        let mut last_update_elapsed_by_node = HashMap::new();
        last_update_elapsed_by_node.insert(root, Duration::ZERO);
        let (external_edits_tx, external_edits_rx) = mpsc::channel();

        Self {
            nodes,
            root,
            time: EngineTime { tick: 0, micro: 0, seq: 0 },
            inbox: Inbox::new(),
            edits: EditQueue::new(),
            external_edits_tx,
            external_edits_rx,
            event_listeners: HashMap::new(),
            ui_event_log: Vec::new(),
            ui_event_log_capacity: engine_ui::DEFAULT_UI_EVENT_LOG_CAPACITY,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            active_edit_session: None,
            runtime_schedule: engine_runtime::ScheduleMgr::default(),
            runtime_resolve_pending: true,
            runtime_limits: engine_runtime::RuntimeLimits::default(),
            runtime_elapsed: Duration::ZERO,
            last_update_elapsed_by_node,
        }
    }

    /// Queues insertion of a node under `parent` (or root when `None`).
    pub fn add_node(&mut self, node: T, parent: Option<NodeId>) {
        self.edits.push(Edit::AddNode {
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

    /// Queues replacement of an existing node.
    pub fn replace_node(&mut self, node: NodeId, new_node: T) {
        self.edits.push(Edit::ReplaceNode { node, new_node: Box::new(new_node) });
    }

    /// Returns a cloneable sender for queuing edits from external threads/tasks.
    ///
    /// Edits sent through this channel are merged into `self.edits` during
    /// `apply_edits()` and runtime tick stabilization passes.
    pub fn external_edit_sender(&self) -> Sender<Edit> {
        self.external_edits_tx.clone()
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
                edit => validated_edits.push(edit),
            }
        }

        for edit in validated_edits {
            self.edits.push(edit);
        }

        Ok(())
    }
}
