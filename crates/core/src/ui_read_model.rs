//! Immutable UI read projection for snapshots and replay.
//!
//! The model owns three layers of state, all updated without holding the live engine lock:
//!
//! 1. `node_store`  — incremental `NodeId → UiNodeDto` map patched from `GraphTransaction` ops.
//! 2. `snapshot_header` — cheap engine metadata (history, user-contexts, project-file, time).
//! 3. `current`     — fully assembled `UiSnapshot` rebuilt from the two layers above.
//!
//! Hot paths (runtime tick, intent) use [`UiReadModel::collect_event_batch`] inside the engine
//! lock followed by [`UiReadModel::apply_event_capture`] outside it, so the O(N) snapshot
//! assembly never blocks the engine mutex.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

use crate::app::ProjectFileSpec;
use crate::contexts::UiUserContextsDto;
use crate::engine::{Engine, EngineTime};
use crate::node::{Node, NodeId};
use crate::ui_sync::{
    UI_PROTOCOL_VERSION, UiChildrenOrderPatch, UiEventBatch, UiEventDto, UiEventKind, UiGraphOp, UiHistoryState,
    UiLoggerState, UiNodeDataDto, UiNodeDto, UiNodeMetaPatch, UiProjectFileSpec, UiSchemaView, UiSnapshot,
    UiSubscriptionScope,
};

const DEFAULT_UI_READ_MODEL_EVENT_CAPACITY: usize = 8192;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Non-node snapshot metadata captured cheaply from the engine.
struct SnapshotHeader {
    at: EngineTime,
    history: UiHistoryState,
    user_contexts: UiUserContextsDto,
    project_file: UiProjectFileSpec,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Reason a read-model snapshot was rebuilt from the live engine.
pub enum UiReadModelReplaceReason {
    /// Initial projection built before the UI host starts accepting requests.
    Initial,
    /// Incremental engine events were published and the immutable snapshot was refreshed.
    EngineEvents,
    /// The live project was replaced by load/new.
    ProjectReplaced,
}

/// Cheap capture produced inside the engine lock and consumed outside it.
pub struct UiEventCapture {
    batch: UiEventBatch,
    history: UiHistoryState,
    user_contexts: UiUserContextsDto,
}

impl UiEventCapture {
    /// Returns the event batch without consuming the capture.
    pub fn batch(&self) -> &UiEventBatch {
        &self.batch
    }
}

/// Immutable UI projection used by HTTP snapshots and WebSocket replay.
pub struct UiReadModel {
    /// Pre-assembled snapshot served by HTTP and used by `replay()`.
    current: RwLock<Arc<UiSnapshot>>,
    /// Retained event ring for WS replay.
    events: Mutex<VecDeque<UiEventDto>>,
    event_capacity: usize,
    /// Head of the event log, always advances even when snapshot rebuild is skipped.
    latest_event_time: Mutex<Option<EngineTime>>,
    /// Incrementally maintained node state — patched from `GraphTransaction` ops.
    node_store: RwLock<HashMap<NodeId, UiNodeDto>>,
    /// Cheap non-node snapshot metadata.
    snapshot_header: Mutex<SnapshotHeader>,
    /// Schema rebuilt on full replace; only new node-types are added on incremental updates.
    snapshot_schema: Mutex<UiSchemaView>,
}

impl UiReadModel {
    /// Builds a read model from the current engine state (acquires no external locks).
    pub fn from_engine<T: Node>(engine: &Engine<T>, project_file: ProjectFileSpec) -> Self {
        let snapshot = snapshot_from_engine(engine, project_file.clone());
        let at = snapshot.at;
        let node_store = nodes_to_store(&snapshot.nodes);
        let schema = snapshot.schema.clone();
        let header = SnapshotHeader {
            at,
            history: snapshot.history.clone(),
            user_contexts: snapshot.user_contexts.clone(),
            project_file: snapshot.project_file.clone(),
        };
        Self {
            current: RwLock::new(Arc::new(snapshot)),
            events: Mutex::new(VecDeque::new()),
            event_capacity: DEFAULT_UI_READ_MODEL_EVENT_CAPACITY,
            latest_event_time: Mutex::new(Some(at)),
            node_store: RwLock::new(node_store),
            snapshot_header: Mutex::new(header),
            snapshot_schema: Mutex::new(schema),
        }
    }

    /// Returns the current immutable snapshot.
    pub fn current_snapshot(&self) -> Arc<UiSnapshot> {
        self.current.read().expect("ui read model poisoned").clone()
    }

    /// Returns the latest known event time without acquiring the snapshot lock.
    pub fn current_event_time(&self) -> Option<EngineTime> {
        *self.latest_event_time.lock().expect("ui read model poisoned")
    }

    /// Rebuilds the entire model from the live engine (project load/replace or initial build).
    pub fn replace_from_engine<T: Node>(
        &self,
        engine: &Engine<T>,
        project_file: ProjectFileSpec,
        reason: UiReadModelReplaceReason,
    ) {
        let snapshot = Arc::new(snapshot_from_engine(engine, project_file));
        let at = snapshot.at;

        // Update all layers atomically-ish (each lock briefly held).
        {
            let new_store = nodes_to_store(&snapshot.nodes);
            *self.node_store.write().expect("ui read model poisoned") = new_store;
        }
        {
            let mut header = self.snapshot_header.lock().expect("ui read model poisoned");
            header.at = at;
            header.history = snapshot.history.clone();
            header.user_contexts = snapshot.user_contexts.clone();
            header.project_file = snapshot.project_file.clone();
        }
        {
            *self.snapshot_schema.lock().expect("ui read model poisoned") = snapshot.schema.clone();
        }
        {
            *self.current.write().expect("ui read model poisoned") = snapshot;
        }
        {
            *self.latest_event_time.lock().expect("ui read model poisoned") = Some(at);
        }

        if matches!(
            reason,
            UiReadModelReplaceReason::ProjectReplaced | UiReadModelReplaceReason::Initial
        ) {
            self.events.lock().expect("ui read model event log poisoned").clear();
        }
    }

    // -----------------------------------------------------------------------
    // Two-step publish: collect inside lock, apply outside lock.
    // -----------------------------------------------------------------------

    /// Collects new events and cheap metadata **while the engine lock is held**.
    /// Pass the latest retained event time captured before the mutation/tick started,
    /// not a retained-slice length, so log compaction cannot move the cursor forward.
    /// Drop the engine lock, then call [`apply_event_capture`].
    pub fn collect_event_batch<T: Node>(
        &self,
        engine: &Engine<T>,
        previous_event_time: Option<EngineTime>,
    ) -> UiEventCapture {
        let batch = engine.ui_event_batch(previous_event_time, UiSubscriptionScope::WholeGraph);
        let history = engine.ui_history_state();
        let user_contexts = engine.ui_user_contexts();
        UiEventCapture {
            batch,
            history,
            user_contexts,
        }
    }

    /// Applies a previously collected capture **outside the engine lock**.
    ///
    /// Structural events (graph transactions, node/child changes, meta) update the `node_store`
    /// and rebuild `current` from pre-built DTOs — no engine traversal needed.
    /// Pure value-change events (params, custom) only advance `latest_event_time`.
    pub fn apply_event_capture(&self, capture: UiEventCapture, project_file: ProjectFileSpec) -> UiEventBatch {
        let UiEventCapture {
            batch,
            history,
            user_contexts,
        } = capture;
        if batch.events.is_empty() {
            return batch;
        }

        self.append_events(batch.events.iter().cloned());

        let has_structural = batch.events.iter().any(event_requires_snapshot_rebuild);

        if has_structural {
            apply_ops_from_events(
                &mut self.node_store.write().expect("ui read model poisoned"),
                &batch.events,
            );

            let at = batch.events.iter().map(|e| e.time).max().unwrap_or_else(|| {
                self.latest_event_time
                    .lock()
                    .expect("ui read model poisoned")
                    .unwrap_or(EngineTime {
                        tick: 0,
                        micro: 0,
                        seq: 0,
                    })
            });

            {
                let mut header = self.snapshot_header.lock().expect("ui read model poisoned");
                header.at = at;
                header.history = history;
                header.user_contexts = user_contexts;
                header.project_file = UiProjectFileSpec::from(project_file);
            }

            self.rebuild_current_from_store();
        } else if let Some(t) = batch.events.iter().map(|e| e.time).max() {
            let mut guard = self.latest_event_time.lock().expect("ui read model poisoned");
            *guard = Some(guard.map_or(t, |existing| existing.max(t)));
        }

        batch
    }

    // -----------------------------------------------------------------------
    // Backward-compatible convenience wrapper (still acquires engine reference).
    // -----------------------------------------------------------------------

    /// Convenience wrapper: collects from the engine then applies immediately.
    ///
    /// Prefer the [`collect_event_batch`] + [`apply_event_capture`] split on hot paths
    /// so the engine lock can be dropped before the O(N) snapshot assembly.
    pub fn publish_engine_events_since<T: Node>(
        &self,
        engine: &Engine<T>,
        previous_event_time: Option<EngineTime>,
        project_file: ProjectFileSpec,
    ) -> UiEventBatch {
        let capture = self.collect_event_batch(engine, previous_event_time);
        self.apply_event_capture(capture, project_file)
    }

    // -----------------------------------------------------------------------
    // Snapshot and replay
    // -----------------------------------------------------------------------

    /// Builds a snapshot payload for the requested scope without reading the live engine.
    pub fn snapshot_for_scope(&self, scope: UiSubscriptionScope) -> UiSnapshot {
        let snapshot = self.current_snapshot();
        let mut out = (*snapshot).clone();
        out.scope = scope.clone();
        out.nodes = filter_snapshot_nodes(&snapshot, scope);
        out.protocol_version = UI_PROTOCOL_VERSION.to_string();
        out
    }

    /// Replays retained events newer than `from` for the requested scope.
    pub fn replay(&self, from: Option<EngineTime>, scope: UiSubscriptionScope) -> UiEventBatch {
        // Copy time out first to avoid holding latest_event_time while acquiring current.
        let latest_time = *self.latest_event_time.lock().expect("ui read model poisoned");
        let current_time = latest_time.unwrap_or_else(|| self.current_snapshot().at);
        let events_guard = self.events.lock().expect("ui read model event log poisoned");
        let first_retained = events_guard.front().map(|event| event.time);
        let mut events = Vec::new();

        if let Some(from) = from {
            if from > current_time {
                return make_resync_event_batch(Some(from), current_time, "cursor_ahead_of_server_time");
            }
            if let Some(first_time) = first_retained {
                if from < first_time {
                    return make_resync_event_batch(Some(from), current_time, "cursor_out_of_retention_window");
                }
            }
        }

        let snapshot = self.current_snapshot();
        for event in events_guard.iter() {
            if from.is_some_and(|cursor| event.time <= cursor) {
                continue;
            }
            if event_matches_scope(&snapshot, &scope, event) {
                events.push(event.clone());
            }
        }

        let to = events.last().map(|event| event.time);
        UiEventBatch { from, to, events }
    }

    /// First retained event time, if any.
    pub fn first_retained_event_time(&self) -> Option<EngineTime> {
        self.events
            .lock()
            .expect("ui read model event log poisoned")
            .front()
            .map(|event| event.time)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn append_events(&self, events: impl IntoIterator<Item = UiEventDto>) {
        let mut guard = self.events.lock().expect("ui read model event log poisoned");
        guard.extend(events);
        while guard.len() > self.event_capacity {
            guard.pop_front();
        }
    }

    /// Rebuilds `current` from `node_store` + `snapshot_header` + `snapshot_schema`.
    /// All three source locks are acquired and released individually — no overlap.
    fn rebuild_current_from_store(&self) {
        let nodes: Vec<UiNodeDto> = self
            .node_store
            .read()
            .expect("ui read model poisoned")
            .values()
            .cloned()
            .collect();

        let (at, history, user_contexts, project_file) = {
            let h = self.snapshot_header.lock().expect("ui read model poisoned");
            (h.at, h.history.clone(), h.user_contexts.clone(), h.project_file.clone())
        };

        let schema = self.snapshot_schema.lock().expect("ui read model poisoned").clone();

        let snapshot = Arc::new(UiSnapshot {
            protocol_version: UI_PROTOCOL_VERSION.to_string(),
            scope: UiSubscriptionScope::WholeGraph,
            at,
            nodes,
            schema,
            history,
            logger: UiLoggerState {
                max_entries: crate::logger::max_entries(),
                records: crate::logger::records(),
            },
            project_file,
            user_contexts,
        });

        *self.current.write().expect("ui read model poisoned") = snapshot;
        let mut time_guard = self.latest_event_time.lock().expect("ui read model poisoned");
        *time_guard = Some(time_guard.map_or(at, |existing| existing.max(at)));
    }
}

// ---------------------------------------------------------------------------
// Full-snapshot helpers (only for initial build / project replace)
// ---------------------------------------------------------------------------

fn snapshot_from_engine<T: Node>(engine: &Engine<T>, project_file: ProjectFileSpec) -> UiSnapshot {
    let mut snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    snapshot.project_file = UiProjectFileSpec::from(project_file);
    snapshot
}

fn nodes_to_store(nodes: &[UiNodeDto]) -> HashMap<NodeId, UiNodeDto> {
    nodes.iter().map(|dto| (dto.node_id, dto.clone())).collect()
}

// ---------------------------------------------------------------------------
// Incremental node-store mutation
// ---------------------------------------------------------------------------

/// Applies `GraphTransaction` ops from the event batch to the node store.
fn apply_ops_from_events(store: &mut HashMap<NodeId, UiNodeDto>, events: &[UiEventDto]) {
    for event in events {
        if let UiEventKind::GraphTransaction { transaction } = &event.kind {
            for op in transaction.ops.iter() {
                apply_graph_op(store, op);
            }
        }
    }
}

fn apply_graph_op(store: &mut HashMap<NodeId, UiNodeDto>, op: &UiGraphOp) {
    match op {
        UiGraphOp::NodeCreated { snapshot, .. } => {
            store.insert(snapshot.node_id, snapshot.clone());
        }
        UiGraphOp::SubtreeRemoved {
            removed_ids,
            parent_after,
            ..
        } => {
            for id in removed_ids {
                store.remove(id);
            }
            apply_children_order(store, parent_after.as_ref());
        }
        UiGraphOp::NodeMoved {
            old_parent_after,
            new_parent_after,
            ..
        } => {
            apply_children_order(store, old_parent_after.as_ref());
            apply_children_order(store, new_parent_after.as_ref());
        }
        UiGraphOp::ChildrenReordered { parent, children } => {
            if let Some(node) = store.get_mut(parent) {
                node.children.clone_from(children);
            }
        }
        UiGraphOp::NodeMetaPatched { node, patch } => {
            if let Some(dto) = store.get_mut(node) {
                apply_meta_patch(&mut dto.meta, patch);
            }
        }
        UiGraphOp::ParamPatched { param, patch, .. } => {
            if let Some(dto) = store.get_mut(param) {
                if let UiNodeDataDto::Parameter { param: param_dto } = &mut dto.data {
                    if let Some(value) = &patch.value {
                        param_dto.value = value.clone();
                    }
                    if let Some(control) = &patch.control {
                        param_dto.control = control.clone();
                    }
                    if let Some(constraints) = &patch.constraints {
                        param_dto.constraints = constraints.clone();
                    }
                }
            }
        }
        // These ops update history/logger state tracked elsewhere; no node-store mutation needed.
        UiGraphOp::HistoryPatched { .. } | UiGraphOp::LoggerPatched { .. } => {}
    }
}

fn apply_children_order(store: &mut HashMap<NodeId, UiNodeDto>, patch: Option<&UiChildrenOrderPatch>) {
    if let Some(patch) = patch {
        if let Some(node) = store.get_mut(&patch.parent) {
            node.children.clone_from(&patch.children);
        }
    }
}

fn apply_meta_patch(meta: &mut crate::ui_sync::UiNodeMetaDto, patch: &UiNodeMetaPatch) {
    if let Some(label) = &patch.label {
        meta.label.clone_from(label);
    }
    if let Some(short_name) = &patch.short_name {
        meta.short_name.clone_from(short_name);
    }
    if let Some(enabled) = patch.enabled {
        meta.enabled = enabled;
    }
    if let Some(can_be_disabled) = patch.can_be_disabled {
        meta.can_be_disabled = can_be_disabled;
    }
    if let Some(description) = &patch.description {
        meta.description.clone_from(description);
    }
    if let Some(user_permissions) = &patch.user_permissions {
        meta.user_permissions.clone_from(user_permissions);
    }
    if let Some(tags) = &patch.tags {
        meta.tags.clone_from(tags);
    }
    if let Some(presentation) = &patch.presentation {
        meta.presentation.clone_from(presentation);
    }
}

// ---------------------------------------------------------------------------
// Snapshot scope filtering (for HTTP endpoint)
// ---------------------------------------------------------------------------

fn filter_snapshot_nodes(snapshot: &UiSnapshot, scope: UiSubscriptionScope) -> Vec<UiNodeDto> {
    match scope {
        UiSubscriptionScope::WholeGraph => snapshot.nodes.clone(),
        UiSubscriptionScope::Subtree { root, max_depth } => {
            let mut out = Vec::new();
            let mut stack = vec![(root, 0u32)];
            while let Some((node_id, depth)) = stack.pop() {
                let Some(node) = snapshot.nodes.iter().find(|c| c.node_id == node_id) else {
                    continue;
                };
                out.push(node.clone());
                if depth >= max_depth {
                    continue;
                }
                for child in node.children.iter().rev() {
                    stack.push((*child, depth.saturating_add(1)));
                }
            }
            out
        }
    }
}

// ---------------------------------------------------------------------------
// Replay helpers
// ---------------------------------------------------------------------------

fn make_resync_event_batch(from: Option<EngineTime>, time: EngineTime, reason: &str) -> UiEventBatch {
    UiEventBatch {
        from,
        to: Some(time),
        events: vec![UiEventDto {
            time,
            kind: UiEventKind::Custom {
                topic: "__transport.resync_required".to_string(),
                origin: None,
                payload: serde_json::json!({ "reason": reason }),
            },
        }],
    }
}

fn event_matches_scope(snapshot: &UiSnapshot, scope: &UiSubscriptionScope, event: &UiEventDto) -> bool {
    match scope {
        UiSubscriptionScope::WholeGraph => true,
        UiSubscriptionScope::Subtree { root, max_depth } => {
            if matches!(event.kind, UiEventKind::GraphTransaction { .. }) {
                return true;
            }
            event_candidate_nodes(event)
                .into_iter()
                .any(|node| snapshot_node_within_subtree(snapshot, node, *root, *max_depth))
        }
    }
}

fn event_candidate_nodes(event: &UiEventDto) -> Vec<NodeId> {
    match &event.kind {
        UiEventKind::GraphTransaction { .. } => Vec::new(),
        UiEventKind::ParamChanged { param, .. } => vec![*param],
        UiEventKind::ParamControlChanged { param, .. } => vec![*param],
        UiEventKind::ParamConstraintsChanged { param, .. } => vec![*param],
        UiEventKind::ChildAdded { parent, child, .. } => vec![*parent, *child],
        UiEventKind::ChildRemoved { parent, child } => vec![*parent, *child],
        UiEventKind::ChildReplaced { parent, old, new, .. } => vec![*parent, *old, *new],
        UiEventKind::ChildMoved {
            child,
            old_parent,
            new_parent,
            ..
        } => vec![*child, *old_parent, *new_parent],
        UiEventKind::ChildReordered { parent, child, .. } => vec![*parent, *child],
        UiEventKind::NodeCreated { node, .. } => vec![*node],
        UiEventKind::NodeDeleted { node } => vec![*node],
        UiEventKind::MetaChanged { node, .. } => vec![*node],
        UiEventKind::Custom { origin, .. } => origin.into_iter().copied().collect(),
    }
}

fn snapshot_node_within_subtree(snapshot: &UiSnapshot, node: NodeId, root: NodeId, max_depth: u32) -> bool {
    if node == root {
        return true;
    }
    let mut depth = 0u32;
    let mut current = node;
    while depth < max_depth {
        let Some(parent) = snapshot_parent(snapshot, current) else {
            return false;
        };
        if parent == root {
            return true;
        }
        current = parent;
        depth = depth.saturating_add(1);
    }
    false
}

fn snapshot_parent(snapshot: &UiSnapshot, child: NodeId) -> Option<NodeId> {
    snapshot
        .nodes
        .iter()
        .find(|node| node.children.contains(&child))
        .map(|node| node.node_id)
}

// ---------------------------------------------------------------------------
// Structural-event predicate
// ---------------------------------------------------------------------------

/// Returns true when an event requires a snapshot rebuild (node structure or metadata changed).
/// Pure value-change events (params, custom) can be skipped to reduce per-tick cost.
fn event_requires_snapshot_rebuild(event: &UiEventDto) -> bool {
    matches!(
        event.kind,
        UiEventKind::GraphTransaction { .. }
            | UiEventKind::NodeCreated { .. }
            | UiEventKind::NodeDeleted { .. }
            | UiEventKind::ChildAdded { .. }
            | UiEventKind::ChildRemoved { .. }
            | UiEventKind::ChildReplaced { .. }
            | UiEventKind::ChildMoved { .. }
            | UiEventKind::ChildReordered { .. }
            | UiEventKind::MetaChanged { .. }
    )
}
