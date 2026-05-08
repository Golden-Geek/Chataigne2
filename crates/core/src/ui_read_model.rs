//! Immutable UI read projection for snapshots and replay.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

use crate::app::ProjectFileSpec;
use crate::engine::{Engine, EngineTime};
use crate::node::Node;
use crate::ui_sync::{
    UI_PROTOCOL_VERSION, UiEventBatch, UiEventDto, UiEventKind, UiProjectFileSpec, UiSnapshot, UiSubscriptionScope,
};

const DEFAULT_UI_READ_MODEL_EVENT_CAPACITY: usize = 8192;

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

/// Immutable UI projection used by HTTP snapshots and replay endpoints.
pub struct UiReadModel {
    current: RwLock<Arc<UiSnapshot>>,
    events: Mutex<VecDeque<UiEventDto>>,
    event_capacity: usize,
}

impl UiReadModel {
    /// Builds a read model from the current engine state.
    pub fn from_engine<T: Node>(engine: &Engine<T>, project_file: ProjectFileSpec) -> Self {
        Self {
            current: RwLock::new(Arc::new(snapshot_from_engine(engine, project_file))),
            events: Mutex::new(VecDeque::new()),
            event_capacity: DEFAULT_UI_READ_MODEL_EVENT_CAPACITY,
        }
    }

    /// Returns the current immutable snapshot.
    pub fn current_snapshot(&self) -> Arc<UiSnapshot> {
        self.current.read().expect("ui read model poisoned").clone()
    }

    /// Rebuilds the immutable snapshot from the live engine.
    pub fn replace_from_engine<T: Node>(
        &self,
        engine: &Engine<T>,
        project_file: ProjectFileSpec,
        reason: UiReadModelReplaceReason,
    ) {
        let snapshot = Arc::new(snapshot_from_engine(engine, project_file));
        *self.current.write().expect("ui read model poisoned") = snapshot;
        if matches!(
            reason,
            UiReadModelReplaceReason::ProjectReplaced | UiReadModelReplaceReason::Initial
        ) {
            self.events.lock().expect("ui read model event log poisoned").clear();
        }
    }

    /// Publishes newly-emitted engine events and refreshes the immutable snapshot.
    pub fn publish_engine_events_since<T: Node>(
        &self,
        engine: &Engine<T>,
        previous_event_len: usize,
        project_file: ProjectFileSpec,
    ) -> UiEventBatch {
        let from = previous_event_len
            .checked_sub(1)
            .and_then(|index| engine.ui_event_log().get(index))
            .map(|event| event.time);
        let batch = engine.ui_event_batch(from, UiSubscriptionScope::WholeGraph);
        if batch.events.is_empty() {
            return batch;
        }

        self.append_events(batch.events.iter().cloned());
        self.replace_from_engine(engine, project_file, UiReadModelReplaceReason::EngineEvents);
        batch
    }

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
        let events_guard = self.events.lock().expect("ui read model event log poisoned");
        let current_time = self.current_snapshot().at;
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

    /// Last retained event time, if any.
    pub fn first_retained_event_time(&self) -> Option<EngineTime> {
        self.events
            .lock()
            .expect("ui read model event log poisoned")
            .front()
            .map(|event| event.time)
    }

    fn append_events(&self, events: impl IntoIterator<Item = UiEventDto>) {
        let mut guard = self.events.lock().expect("ui read model event log poisoned");
        guard.extend(events);
        while guard.len() > self.event_capacity {
            guard.pop_front();
        }
    }
}

fn snapshot_from_engine<T: Node>(engine: &Engine<T>, project_file: ProjectFileSpec) -> UiSnapshot {
    let mut snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    snapshot.project_file = UiProjectFileSpec::from(project_file);
    snapshot
}

fn filter_snapshot_nodes(snapshot: &UiSnapshot, scope: UiSubscriptionScope) -> Vec<crate::ui_sync::UiNodeDto> {
    match scope {
        UiSubscriptionScope::WholeGraph => snapshot.nodes.clone(),
        UiSubscriptionScope::Subtree { root, max_depth } => {
            let mut out = Vec::new();
            let mut stack = vec![(root, 0u32)];
            while let Some((node_id, depth)) = stack.pop() {
                let Some(node) = snapshot.nodes.iter().find(|candidate| candidate.node_id == node_id) else {
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

fn event_candidate_nodes(event: &UiEventDto) -> Vec<crate::node::NodeId> {
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

fn snapshot_node_within_subtree(
    snapshot: &UiSnapshot,
    node: crate::node::NodeId,
    root: crate::node::NodeId,
    max_depth: u32,
) -> bool {
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

fn snapshot_parent(snapshot: &UiSnapshot, child: crate::node::NodeId) -> Option<crate::node::NodeId> {
    snapshot
        .nodes
        .iter()
        .find(|node| node.children.contains(&child))
        .map(|node| node.node_id)
}
