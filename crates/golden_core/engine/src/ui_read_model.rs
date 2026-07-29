//! Immutable UI read projection for snapshots and replay.
//!
//! The model owns three layers of state, maintained independently from the actor-owned engine:
//!
//! 1. `projection` - incremental node, parent, schema, and snapshot-header state.
//! 2. `events` - the retained, time-indexed replay log.
//! 3. A lazy immutable whole-graph snapshot cache inside `projection`.
//!
//! Hot paths (runtime tick, intent) use [`UiReadModel::collect_event_batch`] after mutating the
//! actor-owned engine, end that engine borrow, and call [`UiReadModel::apply_event_capture`] before
//! the same actor turn completes. Incremental publication invalidates the snapshot cache in
//! O(changed nodes); only an explicit snapshot consumer pays for O(N) whole-graph materialization.

mod projection;
mod retained_events;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::contexts::{UiUserContextsDto, UserContextValueType};
use crate::engine::{Engine, EngineTime};
use crate::node::{Node, NodeId};
use crate::ui_sync::{
    UI_PROTOCOL_VERSION, UI_USER_CONTEXT_ENTRY_TOPIC, UI_USER_CONTEXT_SCOPE_TOPIC, UiEventBatch, UiEventDto,
    UiEventKind, UiGraphOp, UiHistoryState, UiNodeDto, UiProjectFileSpec, UiRuntimeStatsDto, UiSnapshot,
    UiSubscriptionScope,
};
use projection::{
    ProjectionState, SnapshotHeader, apply_events, nodes_to_store, parents_from_nodes, scoped_snapshot,
    snapshot_from_projection,
};
use retained_events::{RetainedEventLog, event_is_coalescable_value};

const DEFAULT_UI_READ_MODEL_EVENT_CAPACITY: usize = 8192;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Reason a read-model snapshot was rebuilt from the live engine.
pub enum UiReadModelReplaceReason {
    /// Initial projection built before the UI host starts accepting requests.
    Initial,
    /// Incremental engine events were published and the immutable snapshot cache was invalidated.
    EngineEvents,
    /// The live project was replaced by load/new.
    ProjectReplaced,
}

/// Cheap capture produced from the actor-owned engine and published before its mutation turn ends.
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

/// UI-feedback events split after coalescing under one projection read.
pub struct UiFeedbackEventPartition {
    /// Latest coalescable value changes, ready for the throttled value plane.
    pub values: Vec<UiEventDto>,
    /// Reliable, preview, catalog, and observation events requiring transport classification.
    pub other: Vec<UiEventDto>,
}

/// Immutable UI projection used by HTTP snapshots and WebSocket replay.
pub struct UiReadModel {
    /// Incrementally maintained projection and lazy immutable snapshot cache.
    projection: RwLock<ProjectionState>,
    /// Serializes publication so projection and replay-ring order cannot diverge.
    publication: Mutex<()>,
    /// Retained, time-indexed event log for WS replay.
    events: Mutex<RetainedEventLog>,
    event_capacity: usize,
    /// Highest event time discarded because the retained ring reached capacity.
    last_evicted_event_time: Mutex<Option<EngineTime>>,
    /// Head of the event log, always advances even when snapshot rebuild is skipped.
    latest_event_time: Mutex<Option<EngineTime>>,
    /// Latest runtime timing metrics sampled by the host loop.
    runtime_stats: Mutex<Option<UiRuntimeStatsDto>>,
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
        let nodes = nodes_to_store(&snapshot.nodes);
        let parents = parents_from_nodes(nodes.values());
        let schema = snapshot.schema.clone();
        let header = SnapshotHeader {
            at,
            history: snapshot.history.clone(),
            user_contexts: snapshot.user_contexts.clone(),
            project_file: snapshot.project_file.clone(),
        };
        Self {
            projection: RwLock::new(ProjectionState {
                nodes,
                parents,
                header,
                schema,
                cached_snapshot: Arc::new(snapshot),
                snapshot_dirty: false,
            }),
            publication: Mutex::new(()),
            events: Mutex::new(RetainedEventLog::default()),
            event_capacity: DEFAULT_UI_READ_MODEL_EVENT_CAPACITY,
            last_evicted_event_time: Mutex::new(None),
            latest_event_time: Mutex::new(latest_event_time),
            runtime_stats: Mutex::new(None),
        }
    }

    /// Returns the current immutable whole-graph snapshot, materializing it on demand.
    pub fn current_snapshot(&self) -> Arc<UiSnapshot> {
        {
            let projection = self.projection.read().expect("ui read model poisoned");
            if !projection.snapshot_dirty {
                return projection.cached_snapshot.clone();
            }
        }

        let mut projection = self.projection.write().expect("ui read model poisoned");
        if projection.snapshot_dirty {
            projection.cached_snapshot = Arc::new(snapshot_from_projection(&projection));
            projection.snapshot_dirty = false;
        }
        projection.cached_snapshot.clone()
    }

    /// Returns the latest known event time.
    pub fn current_event_time(&self) -> Option<EngineTime> {
        *self.latest_event_time.lock().expect("ui read model poisoned")
    }

    /// Returns the newest complete projection boundary without materializing a snapshot.
    pub fn current_revision(&self) -> EngineTime {
        let snapshot_time = self.projection.read().expect("ui read model poisoned").header.at;
        self.current_event_time()
            .map_or(snapshot_time, |event_time| event_time.max(snapshot_time))
    }

    /// Returns current host-owned project metadata without materializing a snapshot.
    pub fn current_project_file(&self) -> UiProjectFileSpec {
        self.projection
            .read()
            .expect("ui read model poisoned")
            .header
            .project_file
            .clone()
    }

    /// Returns current user-context metadata without materializing a snapshot.
    pub fn current_user_contexts(&self) -> UiUserContextsDto {
        self.projection
            .read()
            .expect("ui read model poisoned")
            .header
            .user_contexts
            .clone()
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
        let _publication = self.publication.lock().expect("ui read model poisoned");
        let snapshot = Arc::new(snapshot_from_engine(engine, project_file.into()));
        let at = snapshot.at;
        let latest_event_time = engine.ui_event_log().last().map(|event| event.time);
        {
            let nodes = nodes_to_store(&snapshot.nodes);
            let parents = parents_from_nodes(nodes.values());
            *self.projection.write().expect("ui read model poisoned") = ProjectionState {
                nodes,
                parents,
                header: SnapshotHeader {
                    at,
                    history: snapshot.history.clone(),
                    user_contexts: snapshot.user_contexts.clone(),
                    project_file: snapshot.project_file.clone(),
                },
                schema: snapshot.schema.clone(),
                cached_snapshot: snapshot,
                snapshot_dirty: false,
            };
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
    ///
    /// The publication guard serializes this update with capture application and project
    /// replacement, including for callers outside [`crate::application::ProductionRuntime`].
    pub fn set_project_file<P>(&self, project_file: P)
    where
        P: Into<UiProjectFileSpec>,
    {
        let _publication = self.publication.lock().expect("ui read model poisoned");
        let mut projection = self.projection.write().expect("ui read model poisoned");
        projection.header.project_file = project_file.into();
        projection.snapshot_dirty = true;
    }

    // -----------------------------------------------------------------------
    // Two-step publish: collect from the actor-owned engine, then publish in the same actor turn.
    // -----------------------------------------------------------------------

    /// Collects new events and cheap metadata while the caller exclusively owns the engine.
    /// Pass the latest retained event time captured before the mutation/tick started,
    /// not a retained-slice length, so log compaction cannot move the cursor forward.
    /// End the engine borrow, then call [`apply_event_capture`] before releasing the
    /// caller's mutation-ordering boundary.
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

    /// Applies a previously collected capture after the live-engine borrow is no longer needed.
    ///
    /// Graph and parameter events update the projection incrementally. Publication only
    /// invalidates the immutable whole-graph snapshot cache; a snapshot consumer materializes it.
    pub fn apply_event_capture(&self, capture: UiEventCapture) -> UiEventBatch {
        let _publication = self.publication.lock().expect("ui read model poisoned");
        let UiEventCapture {
            mut batch,
            history,
            user_contexts,
        } = capture;
        batch.runtime = self.runtime_stats();
        {
            let mut projection = self.projection.write().expect("ui read model poisoned");
            if projection.header.history != history {
                projection.header.history = history;
                projection.snapshot_dirty = true;
            }
            if let Some(user_contexts) = user_contexts {
                if projection.header.user_contexts != user_contexts {
                    projection.header.user_contexts = user_contexts;
                    projection.snapshot_dirty = true;
                }
            }
        }
        if batch.events.is_empty() {
            return batch;
        }

        let batch_time = batch.events.iter().map(|event| event.time).max();
        {
            let mut projection = self.projection.write().expect("ui read model poisoned");
            apply_events(&mut projection, &batch.events);
            if let Some(time) = batch_time {
                projection.header.at = projection.header.at.max(time);
            }
            projection.snapshot_dirty = true;
        }

        self.append_events(batch.events.iter().cloned());

        if let Some(t) = batch_time {
            let mut guard = self.latest_event_time.lock().expect("ui read model poisoned");
            *guard = Some(guard.map_or(t, |existing| existing.max(t)));
        }

        batch
    }

    // -----------------------------------------------------------------------
    // Convenience wrapper for callers that already hold an engine reference.
    // -----------------------------------------------------------------------

    /// Convenience wrapper: collects from the engine then applies immediately.
    ///
    /// Prefer the [`collect_event_batch`] + [`apply_event_capture`] split on hot paths so
    /// publication does not overlap a live-engine borrow, while both steps remain inside the
    /// same mutation-ordering boundary.
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
        if matches!(scope, UiSubscriptionScope::WholeGraph) {
            let mut snapshot = (*self.current_snapshot()).clone();
            snapshot.protocol_version = UI_PROTOCOL_VERSION.to_string();
            return snapshot;
        }

        let projection = self.projection.read().expect("ui read model poisoned");
        scoped_snapshot(&projection, scope)
    }

    /// Replays retained events newer than `from` for the requested scope.
    pub fn replay(&self, from: Option<EngineTime>, scope: UiSubscriptionScope) -> UiEventBatch {
        let events_guard = self.events.lock().expect("ui read model event log poisoned");
        let current_time = self.current_revision();
        let last_evicted = *self
            .last_evicted_event_time
            .lock()
            .expect("ui read model eviction watermark poisoned");

        if let Some(from) = from {
            if from > current_time {
                return make_resync_event_batch(Some(from), current_time, "cursor_ahead_of_server_time");
            }
            if last_evicted.is_some_and(|evicted_through| from < evicted_through) {
                return make_resync_event_batch(Some(from), current_time, "cursor_out_of_retention_window");
            }
        }

        let retained = events_guard.events_after(from);
        let to = retained.last().map(|event| event.time);
        drop(events_guard);

        let events = match &scope {
            UiSubscriptionScope::WholeGraph => retained,
            UiSubscriptionScope::Subtree { .. } => {
                let projection = self.projection.read().expect("ui read model poisoned");
                retained
                    .iter()
                    .filter_map(|event| event_for_scope(&projection.parents, &scope, event))
                    .collect()
            }
        };

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
        let projection = self.projection.read().expect("ui read model poisoned");
        coalesce_ui_feedback_events(&projection.nodes, events)
    }

    /// Coalesces and partitions one visible batch while holding one projection read.
    ///
    /// Transport dispatch uses this instead of reacquiring the projection lock for
    /// every event while deciding whether it belongs to the value plane.
    pub fn partition_ui_feedback_events(&self, events: Vec<UiEventDto>) -> UiFeedbackEventPartition {
        let projection = self.projection.read().expect("ui read model poisoned");
        let coalesced = coalesce_ui_feedback_events(&projection.nodes, events);
        let mut values = Vec::new();
        let mut other = Vec::new();
        for event in coalesced {
            if event_is_coalescable_value(&projection.nodes, &event) {
                values.push(event);
            } else {
                other.push(event);
            }
        }
        UiFeedbackEventPartition { values, other }
    }

    /// Returns true when an event belongs to the coalescable UI value plane.
    pub fn event_is_coalescable_value(&self, event: &UiEventDto) -> bool {
        let projection = self.projection.read().expect("ui read model poisoned");
        event_is_coalescable_value(&projection.nodes, event)
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
            .first_event_time()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn append_events(&self, events: impl IntoIterator<Item = UiEventDto>) {
        let mut evicted_through: Option<EngineTime> = None;
        {
            let mut guard = self.events.lock().expect("ui read model event log poisoned");
            let projection = self.projection.read().expect("ui read model poisoned");
            for event in events {
                if let Some(evicted) = guard.append(&projection.nodes, event, self.event_capacity) {
                    evicted_through = Some(evicted_through.map_or(evicted, |time| time.max(evicted)));
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
    }

    #[cfg(test)]
    pub(crate) fn set_event_capacity_for_tests(&mut self, capacity: usize) {
        assert!(capacity > 0, "event capacity must be positive");
        self.event_capacity = capacity;
    }

    #[cfg(test)]
    pub(crate) fn snapshot_cache_is_dirty_for_tests(&self) -> bool {
        self.projection.read().expect("ui read model poisoned").snapshot_dirty
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
    pending: Vec<Option<UiEventDto>>,
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
            if let Some(previous) = self.pending[index].take() {
                preserve_ui_param_changed_old_value(&mut event.kind, previous.kind);
            }
        }
        self.pending_param_indices.insert(param, self.pending.len());
        self.pending.push(Some(event));
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
        self.out.extend(self.pending.drain(..).flatten());
        self.pending_param_indices.clear();
    }
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

fn coalesce_ui_feedback_events(nodes: &HashMap<NodeId, UiNodeDto>, events: Vec<UiEventDto>) -> Vec<UiEventDto> {
    let mut coalescer = UiFeedbackCoalescer::default();
    for event in events {
        if event_is_coalescable_value(nodes, &event) {
            coalescer.push_coalescable(event);
        } else {
            coalescer.push_barrier(event);
        }
    }
    coalescer.finish()
}

fn event_for_scope(
    parents: &HashMap<NodeId, NodeId>,
    scope: &UiSubscriptionScope,
    event: &UiEventDto,
) -> Option<UiEventDto> {
    match (&event.kind, scope) {
        (_, UiSubscriptionScope::WholeGraph) => Some(event.clone()),
        (UiEventKind::GraphTransaction { transaction }, UiSubscriptionScope::Subtree { root, max_depth }) => {
            let ops: Vec<UiGraphOp> = transaction
                .ops
                .iter()
                .filter(|op| graph_op_matches_subtree(parents, op, *root, *max_depth))
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
        _ => event_matches_scope(parents, scope, event).then(|| event.clone()),
    }
}

fn graph_op_matches_subtree(parents: &HashMap<NodeId, NodeId>, op: &UiGraphOp, root: NodeId, max_depth: u32) -> bool {
    match op {
        UiGraphOp::NodeCreated {
            snapshot: node, parent, ..
        } => {
            node_within_subtree(parents, node.node_id, root, max_depth)
                || parent.is_some_and(|parent| node_within_subtree(parents, parent, root, max_depth))
        }
        UiGraphOp::SubtreeInserted {
            root: inserted_root,
            parent,
            nodes,
            ..
        } => {
            node_within_subtree(parents, *parent, root, max_depth)
                || node_within_subtree(parents, *inserted_root, root, max_depth)
                || nodes
                    .iter()
                    .any(|node| node_within_subtree(parents, node.node_id, root, max_depth))
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
                    .is_some_and(|patch| node_within_subtree(parents, patch.parent, root, max_depth))
        }
        UiGraphOp::NodeMoved {
            node,
            old_parent,
            new_parent,
            old_parent_after,
            new_parent_after,
        } => {
            node_within_subtree(parents, *node, root, max_depth)
                || old_parent.is_some_and(|parent| node_within_subtree(parents, parent, root, max_depth))
                || new_parent.is_some_and(|parent| node_within_subtree(parents, parent, root, max_depth))
                || old_parent_after
                    .as_ref()
                    .is_some_and(|patch| node_within_subtree(parents, patch.parent, root, max_depth))
                || new_parent_after
                    .as_ref()
                    .is_some_and(|patch| node_within_subtree(parents, patch.parent, root, max_depth))
        }
        UiGraphOp::ChildrenReordered { parent, .. } => node_within_subtree(parents, *parent, root, max_depth),
        UiGraphOp::NodeMetaPatched { node, .. } => node_within_subtree(parents, *node, root, max_depth),
        UiGraphOp::ParamPatched { node, param, .. } => {
            node_within_subtree(parents, *node, root, max_depth)
                || node_within_subtree(parents, *param, root, max_depth)
        }
        UiGraphOp::HistoryPatched { .. } | UiGraphOp::LoggerPatched { .. } => true,
    }
}

fn event_matches_scope(parents: &HashMap<NodeId, NodeId>, scope: &UiSubscriptionScope, event: &UiEventDto) -> bool {
    match scope {
        UiSubscriptionScope::WholeGraph => true,
        UiSubscriptionScope::Subtree { root, max_depth } => {
            if matches!(event.kind, UiEventKind::GraphTransaction { .. }) {
                return true;
            }
            event_candidate_nodes(event)
                .into_iter()
                .any(|node| node_within_subtree(parents, node, *root, *max_depth))
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
        UiEventKind::Custom { origin, .. } => origin.iter().copied().collect(),
    }
}

fn node_within_subtree(parents: &HashMap<NodeId, NodeId>, node: NodeId, root: NodeId, max_depth: u32) -> bool {
    if node == root {
        return true;
    }
    let max_steps = usize::try_from(max_depth).unwrap_or(usize::MAX).min(parents.len());
    let mut depth = 0usize;
    let mut current = node;
    while depth < max_steps {
        let Some(parent) = parents.get(&current).copied() else {
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
