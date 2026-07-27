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

use crate::contexts::{UiUserContextsDto, UserContextValueType};
use crate::engine::{Engine, EngineTime};
use crate::events::CustomEventRetention;
use crate::node::{Node, NodeId};
use crate::parameter::{ParamValue, ParameterEventBehaviour};
use crate::ui_sync::{
    UI_PROTOCOL_VERSION, UI_USER_CONTEXT_ENTRY_TOPIC, UI_USER_CONTEXT_SCOPE_TOPIC, UiChildrenOrderPatch, UiEventBatch,
    UiEventDto, UiEventKind, UiGraphOp, UiHistoryState, UiLoggerState, UiNodeDataDto, UiNodeDto, UiNodeMetaPatch,
    UiProjectFileSpec, UiRuntimeStatsDto, UiSchemaView, UiSnapshot, UiSubscriptionScope,
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
    user_contexts: Option<UiUserContextsDto>,
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
    /// Highest event time discarded because the retained ring reached capacity.
    last_evicted_event_time: Mutex<Option<EngineTime>>,
    /// Head of the event log, always advances even when snapshot rebuild is skipped.
    latest_event_time: Mutex<Option<EngineTime>>,
    /// Latest runtime timing metrics sampled by the host loop.
    runtime_stats: Mutex<Option<UiRuntimeStatsDto>>,
    /// Incrementally maintained node state — patched from `GraphTransaction` ops.
    node_store: RwLock<HashMap<NodeId, UiNodeDto>>,
    /// Cheap non-node snapshot metadata.
    snapshot_header: Mutex<SnapshotHeader>,
    /// Schema rebuilt on full replace; only new node-types are added on incremental updates.
    snapshot_schema: Mutex<UiSchemaView>,
}

impl UiReadModel {
    /// Builds a read model from the current engine state (acquires no external locks).
    pub fn from_engine<T, P>(engine: &Engine<T>, project_file: P) -> Self
    where
        T: Node,
        P: Into<UiProjectFileSpec>,
    {
        let snapshot = snapshot_from_engine(engine, project_file.into());
        let at = snapshot.at;
        let latest_event_time = engine.ui_event_log().last().map(|event| event.time);
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
            last_evicted_event_time: Mutex::new(None),
            latest_event_time: Mutex::new(latest_event_time),
            runtime_stats: Mutex::new(None),
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

    /// Stores the latest runtime timing metrics sampled by the host loop.
    pub fn set_runtime_stats(&self, stats: UiRuntimeStatsDto) {
        *self.runtime_stats.lock().expect("ui read model poisoned") = Some(stats);
    }

    /// Returns the latest runtime timing metrics sampled by the host loop.
    pub fn runtime_stats(&self) -> Option<UiRuntimeStatsDto> {
        *self.runtime_stats.lock().expect("ui read model poisoned")
    }

    /// Rebuilds the entire model from the live engine (project load/replace or initial build).
    pub fn replace_from_engine<T, P>(&self, engine: &Engine<T>, project_file: P, reason: UiReadModelReplaceReason)
    where
        T: Node,
        P: Into<UiProjectFileSpec>,
    {
        let snapshot = Arc::new(snapshot_from_engine(engine, project_file.into()));
        let at = snapshot.at;
        let latest_event_time = engine.ui_event_log().last().map(|event| event.time);

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
            *self.latest_event_time.lock().expect("ui read model poisoned") = latest_event_time;
        }

        if matches!(
            reason,
            UiReadModelReplaceReason::ProjectReplaced | UiReadModelReplaceReason::Initial
        ) {
            self.events.lock().expect("ui read model event log poisoned").clear();
            *self
                .last_evicted_event_time
                .lock()
                .expect("ui read model eviction watermark poisoned") = None;
        }
    }

    /// Updates host-owned project-file metadata without touching graph projection state.
    pub fn set_project_file<P>(&self, project_file: P)
    where
        P: Into<UiProjectFileSpec>,
    {
        {
            let mut header = self.snapshot_header.lock().expect("ui read model poisoned");
            header.project_file = project_file.into();
        }
        self.rebuild_current_from_store();
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
        let user_contexts = batch
            .events
            .iter()
            .any(event_requires_user_context_refresh)
            .then(|| engine.ui_user_contexts());
        UiEventCapture {
            batch,
            history,
            user_contexts,
        }
    }

    /// Applies a previously collected capture **outside the engine lock**.
    ///
    /// All graph and parameter events update the `node_store`. Structural events also rebuild
    /// `current` from pre-built DTOs without traversing the engine. Pure parameter changes defer
    /// that O(N) rebuild; snapshot requests materialize their latest node DTOs from `node_store`.
    pub fn apply_event_capture(&self, capture: UiEventCapture) -> UiEventBatch {
        let UiEventCapture {
            mut batch,
            history,
            user_contexts,
        } = capture;
        let user_contexts_changed = user_contexts.is_some();
        batch.runtime = self.runtime_stats();
        {
            let mut header = self.snapshot_header.lock().expect("ui read model poisoned");
            header.history = history;
            if let Some(user_contexts) = user_contexts {
                header.user_contexts = user_contexts;
            }
        }
        if batch.events.is_empty() {
            return batch;
        }

        self.append_events(batch.events.iter().cloned());

        let has_structural = batch.events.iter().any(event_requires_snapshot_rebuild);

        apply_events_to_store(
            &mut self.node_store.write().expect("ui read model poisoned"),
            &batch.events,
        );

        if has_structural || user_contexts_changed {
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
            }

            self.rebuild_current_from_store();
        } else if let Some(t) = batch.events.iter().map(|e| e.time).max() {
            let mut guard = self.latest_event_time.lock().expect("ui read model poisoned");
            *guard = Some(guard.map_or(t, |existing| existing.max(t)));
        }

        batch
    }

    // -----------------------------------------------------------------------
    // Convenience wrapper (still acquires the engine reference).
    // -----------------------------------------------------------------------

    /// Convenience wrapper: collects from the engine then applies immediately.
    ///
    /// Prefer the [`collect_event_batch`] + [`apply_event_capture`] split on hot paths
    /// so the engine lock can be dropped before the O(N) snapshot assembly.
    pub fn publish_engine_events_since<T: Node>(
        &self,
        engine: &Engine<T>,
        previous_event_time: Option<EngineTime>,
    ) -> UiEventBatch {
        let capture = self.collect_event_batch(engine, previous_event_time);
        self.apply_event_capture(capture)
    }

    // -----------------------------------------------------------------------
    // Snapshot and replay
    // -----------------------------------------------------------------------

    /// Builds a snapshot payload for the requested scope without reading the live engine.
    pub fn snapshot_for_scope(&self, scope: UiSubscriptionScope) -> UiSnapshot {
        let snapshot = self.current_snapshot();
        let mut out = (*snapshot).clone();
        {
            // `current` intentionally avoids an O(N) rebuild for every runtime value
            // update. A snapshot is already an O(N) operation, so overlay the latest
            // parameter values, controls, and constraints here. Structural batches
            // keep `snapshot.nodes` and `node_store` membership in sync.
            let store = self.node_store.read().expect("ui read model poisoned");
            out.nodes = snapshot
                .nodes
                .iter()
                .filter_map(|node| store.get(&node.node_id).cloned())
                .collect();
        }
        {
            let header = self.snapshot_header.lock().expect("ui read model poisoned");
            out.at = header.at;
            out.history = header.history.clone();
            out.user_contexts = header.user_contexts.clone();
            out.project_file = header.project_file.clone();
        }
        if let Some(latest_event_time) = *self.latest_event_time.lock().expect("ui read model poisoned") {
            out.at = out.at.max(latest_event_time);
        }
        out.scope = scope.clone();
        out.nodes = filter_snapshot_nodes(&out, scope);
        out.protocol_version = UI_PROTOCOL_VERSION.to_string();
        out
    }

    /// Replays retained events newer than `from` for the requested scope.
    pub fn replay(&self, from: Option<EngineTime>, scope: UiSubscriptionScope) -> UiEventBatch {
        // Copy time out first to avoid holding latest_event_time while acquiring current.
        let latest_time = *self.latest_event_time.lock().expect("ui read model poisoned");
        let snapshot_time = self.current_snapshot().at;
        let current_time = latest_time.map_or(snapshot_time, |event_time| event_time.max(snapshot_time));
        let last_evicted = *self
            .last_evicted_event_time
            .lock()
            .expect("ui read model eviction watermark poisoned");
        let events_guard = self.events.lock().expect("ui read model event log poisoned");
        let mut events = Vec::new();

        if let Some(from) = from {
            if from > current_time {
                return make_resync_event_batch(Some(from), current_time, "cursor_ahead_of_server_time");
            }
            if last_evicted.is_some_and(|evicted_through| from < evicted_through) {
                return make_resync_event_batch(Some(from), current_time, "cursor_out_of_retention_window");
            }
        }

        let snapshot = self.current_snapshot();
        for event in events_guard.iter() {
            if from.is_some_and(|cursor| event.time <= cursor) {
                continue;
            }
            if let Some(scoped_event) = event_for_scope(&snapshot, &scope, event) {
                events.push(scoped_event);
            }
        }

        let to = events.last().map(|event| event.time);
        UiEventBatch {
            from,
            to,
            runtime: self.runtime_stats(),
            events,
        }
    }

    /// Conflates normal UI-feedback parameter value events to the latest value per parameter.
    ///
    /// This only affects UI replay payloads. The engine event log and script/watch-style event
    /// delivery remain lossless.
    pub fn coalesce_ui_feedback_events(&self, events: Vec<UiEventDto>) -> Vec<UiEventDto> {
        let store = self.node_store.read().expect("ui read model poisoned");
        let mut coalescer = UiFeedbackCoalescer::default();

        for event in events {
            if param_changed_event_is_ui_coalescable(&store, &event) {
                coalescer.push_coalescable(event);
            } else {
                coalescer.push_barrier(event);
            }
        }

        coalescer.finish()
    }

    /// Returns true when an event belongs to the coalescable UI value plane.
    pub fn event_is_coalescable_value(&self, event: &UiEventDto) -> bool {
        let store = self.node_store.read().expect("ui read model poisoned");
        param_changed_event_is_ui_coalescable(&store, event)
    }

    /// Highest event time discarded because the replay ring reached capacity.
    pub fn last_evicted_event_time(&self) -> Option<EngineTime> {
        *self
            .last_evicted_event_time
            .lock()
            .expect("ui read model eviction watermark poisoned")
    }

    /// Oldest event still represented in the replay ring.
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
        let mut evicted_through: Option<EngineTime> = None;
        {
            let store = self.node_store.read().expect("ui read model poisoned");
            let mut guard = self.events.lock().expect("ui read model event log poisoned");
            for event in events {
                append_retained_ui_event(&mut guard, &store, event);
                while guard.len() > self.event_capacity {
                    if let Some(evicted) = guard.pop_front() {
                        evicted_through = Some(evicted_through.map_or(evicted.time, |time| time.max(evicted.time)));
                    }
                }
            }
        }
        if let Some(evicted_through) = evicted_through {
            let mut watermark = self
                .last_evicted_event_time
                .lock()
                .expect("ui read model eviction watermark poisoned");
            *watermark = Some(watermark.map_or(evicted_through, |time| time.max(evicted_through)));
        }
    }

    #[cfg(test)]
    pub(crate) fn set_event_capacity_for_tests(&mut self, capacity: usize) {
        assert!(capacity > 0, "event capacity must be positive");
        self.event_capacity = capacity;
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

fn snapshot_from_engine<T: Node>(engine: &Engine<T>, project_file: UiProjectFileSpec) -> UiSnapshot {
    let mut snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    snapshot.project_file = project_file;
    snapshot
}

fn nodes_to_store(nodes: &[UiNodeDto]) -> HashMap<NodeId, UiNodeDto> {
    nodes.iter().map(|dto| (dto.node_id, dto.clone())).collect()
}

// ---------------------------------------------------------------------------
// Incremental node-store mutation
// ---------------------------------------------------------------------------

/// Applies graph transactions and standalone parameter changes to the node store.
fn apply_events_to_store(store: &mut HashMap<NodeId, UiNodeDto>, events: &[UiEventDto]) {
    for event in events {
        match &event.kind {
            UiEventKind::GraphTransaction { transaction } => {
                for op in transaction.ops.iter() {
                    apply_graph_op(store, op);
                }
            }
            UiEventKind::ParamChanged {
                param,
                old_value,
                new_value,
            } => {
                if let Some(UiNodeDto {
                    data: UiNodeDataDto::Parameter { param: param_dto },
                    ..
                }) = store.get_mut(param)
                {
                    if param_dto.default_value.is_none() && old_value != new_value {
                        param_dto.default_value = Some(old_value.clone());
                    }
                    param_dto.value.clone_from(new_value);
                    if param_dto.default_value.as_ref() == Some(new_value) {
                        param_dto.default_value = None;
                    }
                }
            }
            UiEventKind::ParamControlChanged { param, new_state, .. } => {
                if let Some(UiNodeDto {
                    data: UiNodeDataDto::Parameter { param: param_dto },
                    ..
                }) = store.get_mut(param)
                {
                    param_dto.control.clone_from(new_state);
                }
            }
            UiEventKind::ParamConstraintsChanged {
                param, new_constraints, ..
            } => {
                if let Some(UiNodeDto {
                    data: UiNodeDataDto::Parameter { param: param_dto },
                    ..
                }) = store.get_mut(param)
                {
                    param_dto.constraints.clone_from(new_constraints);
                }
            }
            _ => {}
        }
    }
}

fn apply_graph_op(store: &mut HashMap<NodeId, UiNodeDto>, op: &UiGraphOp) {
    match op {
        UiGraphOp::NodeCreated { snapshot, .. } => {
            store.insert(snapshot.node_id, snapshot.clone());
        }
        UiGraphOp::SubtreeInserted {
            nodes,
            parent,
            parent_children_after,
            ..
        } => {
            for node in nodes {
                store.insert(node.node_id, node.clone());
            }
            if let Some(parent_dto) = store.get_mut(parent) {
                parent_dto.children.clone_from(parent_children_after);
            }
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
        runtime: None,
        events: vec![UiEventDto {
            time,
            kind: UiEventKind::Custom {
                topic: "__transport.resync_required".to_string(),
                origin: None,
                payload: serde_json::json!({ "reason": reason }),
                retention: crate::events::CustomEventRetention::Replay,
            },
        }],
    }
}

#[derive(Default)]
struct UiFeedbackCoalescer {
    out: Vec<UiEventDto>,
    pending: Vec<UiEventDto>,
    pending_param_indices: HashMap<NodeId, usize>,
}

impl UiFeedbackCoalescer {
    fn push_coalescable(&mut self, event: UiEventDto) {
        let UiEventKind::ParamChanged {
            param,
            old_value: _,
            new_value: _,
        } = &event.kind
        else {
            return;
        };

        let param = *param;
        let mut event = event;
        if let Some(index) = self.pending_param_indices.get(&param).copied() {
            let previous = self.pending.remove(index);
            preserve_ui_param_changed_old_value(&mut event.kind, previous.kind);
            for value in self.pending_param_indices.values_mut() {
                if *value > index {
                    *value -= 1;
                }
            }
            self.pending_param_indices.remove(&param);
        }
        self.pending_param_indices.insert(param, self.pending.len());
        self.pending.push(event);
    }

    fn push_barrier(&mut self, event: UiEventDto) {
        self.flush_pending();
        self.out.push(event);
    }

    fn finish(mut self) -> Vec<UiEventDto> {
        self.flush_pending();
        self.out
    }

    fn flush_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        self.out.append(&mut self.pending);
        self.pending_param_indices.clear();
    }
}

fn append_retained_ui_event(events: &mut VecDeque<UiEventDto>, store: &HashMap<NodeId, UiNodeDto>, event: UiEventDto) {
    if let Some((topic, origin)) = latest_custom_event_key(&event) {
        if let Some(index) = (0..events.len())
            .rev()
            .find(|index| latest_custom_event_key(&events[*index]).is_some_and(|key| key == (topic, origin)))
        {
            events.remove(index);
        }
        events.push_back(event);
        return;
    }

    let Some(param) = coalescable_param_changed_event_param(store, &event) else {
        events.push_back(event);
        return;
    };

    let mut previous_index = None;
    for index in (0..events.len()).rev() {
        let Some(existing_param) = coalescable_param_changed_event_param(store, &events[index]) else {
            break;
        };
        if existing_param == param {
            previous_index = Some(index);
            break;
        }
    }

    let mut event = event;
    if let Some(index) = previous_index {
        if let Some(previous) = events.remove(index) {
            preserve_ui_param_changed_old_value(&mut event.kind, previous.kind);
        }
    }
    events.push_back(event);
}

fn latest_custom_event_key(event: &UiEventDto) -> Option<(&str, Option<NodeId>)> {
    let UiEventKind::Custom {
        topic,
        origin,
        retention: CustomEventRetention::Latest,
        ..
    } = &event.kind
    else {
        return None;
    };
    Some((topic.as_str(), *origin))
}

fn coalescable_param_changed_event_param(store: &HashMap<NodeId, UiNodeDto>, event: &UiEventDto) -> Option<NodeId> {
    let UiEventKind::ParamChanged { param, new_value, .. } = &event.kind else {
        return None;
    };

    if matches!(new_value, ParamValue::Trigger()) {
        return None;
    }

    let Some(node) = store.get(param) else {
        return None;
    };
    let UiNodeDataDto::Parameter { param: param_dto } = &node.data else {
        return None;
    };

    (param_dto.event_behaviour == ParameterEventBehaviour::Coalesce).then_some(*param)
}

fn preserve_ui_param_changed_old_value(new_kind: &mut UiEventKind, previous_kind: UiEventKind) {
    let (
        UiEventKind::ParamChanged {
            old_value: new_old_value,
            ..
        },
        UiEventKind::ParamChanged {
            old_value: previous_old_value,
            ..
        },
    ) = (new_kind, previous_kind)
    else {
        return;
    };

    *new_old_value = previous_old_value;
}

fn param_changed_event_is_ui_coalescable(store: &HashMap<NodeId, UiNodeDto>, event: &UiEventDto) -> bool {
    coalescable_param_changed_event_param(store, event).is_some()
}

fn event_for_scope(snapshot: &UiSnapshot, scope: &UiSubscriptionScope, event: &UiEventDto) -> Option<UiEventDto> {
    match (&event.kind, scope) {
        (_, UiSubscriptionScope::WholeGraph) => Some(event.clone()),
        (UiEventKind::GraphTransaction { transaction }, UiSubscriptionScope::Subtree { root, max_depth }) => {
            let ops: Vec<UiGraphOp> = transaction
                .ops
                .iter()
                .filter(|op| graph_op_matches_subtree(snapshot, op, *root, *max_depth))
                .cloned()
                .collect();
            if ops.is_empty() {
                return None;
            }

            let mut transaction = transaction.clone();
            transaction.ops = ops;
            Some(UiEventDto {
                time: event.time,
                kind: UiEventKind::GraphTransaction { transaction },
            })
        }
        _ => event_matches_scope(snapshot, scope, event).then(|| event.clone()),
    }
}

fn graph_op_matches_subtree(snapshot: &UiSnapshot, op: &UiGraphOp, root: NodeId, max_depth: u32) -> bool {
    match op {
        UiGraphOp::NodeCreated {
            snapshot: node, parent, ..
        } => {
            snapshot_node_within_subtree(snapshot, node.node_id, root, max_depth)
                || parent.is_some_and(|parent| snapshot_node_within_subtree(snapshot, parent, root, max_depth))
        }
        UiGraphOp::SubtreeInserted {
            root: inserted_root,
            parent,
            nodes,
            ..
        } => {
            snapshot_node_within_subtree(snapshot, *parent, root, max_depth)
                || snapshot_node_within_subtree(snapshot, *inserted_root, root, max_depth)
                || nodes
                    .iter()
                    .any(|node| snapshot_node_within_subtree(snapshot, node.node_id, root, max_depth))
        }
        UiGraphOp::SubtreeRemoved {
            root: removed_root,
            removed_ids,
            parent_after,
        } => {
            *removed_root == root
                || removed_ids.contains(&root)
                || parent_after
                    .as_ref()
                    .is_some_and(|patch| snapshot_node_within_subtree(snapshot, patch.parent, root, max_depth))
        }
        UiGraphOp::NodeMoved {
            node,
            old_parent,
            new_parent,
            old_parent_after,
            new_parent_after,
        } => {
            snapshot_node_within_subtree(snapshot, *node, root, max_depth)
                || old_parent.is_some_and(|parent| snapshot_node_within_subtree(snapshot, parent, root, max_depth))
                || new_parent.is_some_and(|parent| snapshot_node_within_subtree(snapshot, parent, root, max_depth))
                || old_parent_after
                    .as_ref()
                    .is_some_and(|patch| snapshot_node_within_subtree(snapshot, patch.parent, root, max_depth))
                || new_parent_after
                    .as_ref()
                    .is_some_and(|patch| snapshot_node_within_subtree(snapshot, patch.parent, root, max_depth))
        }
        UiGraphOp::ChildrenReordered { parent, .. } => snapshot_node_within_subtree(snapshot, *parent, root, max_depth),
        UiGraphOp::NodeMetaPatched { node, .. } => snapshot_node_within_subtree(snapshot, *node, root, max_depth),
        UiGraphOp::ParamPatched { node, param, .. } => {
            snapshot_node_within_subtree(snapshot, *node, root, max_depth)
                || snapshot_node_within_subtree(snapshot, *param, root, max_depth)
        }
        UiGraphOp::HistoryPatched { .. } | UiGraphOp::LoggerPatched { .. } => true,
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

fn event_requires_user_context_refresh(event: &UiEventDto) -> bool {
    if event_requires_snapshot_rebuild(event) {
        return true;
    }
    matches!(
        &event.kind,
        UiEventKind::ParamChanged {
            old_value,
            new_value,
            ..
        } if UserContextValueType::from_param_value(old_value)
            != UserContextValueType::from_param_value(new_value)
    ) || matches!(
        &event.kind,
        UiEventKind::Custom { topic, .. }
            if topic == UI_USER_CONTEXT_SCOPE_TOPIC || topic == UI_USER_CONTEXT_ENTRY_TOPIC
    )
}
