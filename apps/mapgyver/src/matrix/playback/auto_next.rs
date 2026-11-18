//! Cell advances share the Matrix wake deadline and run through its actor transaction.
use super::*;
use golden_core::{
    node::{NodeId, NodeUuid},
    process_ctx::ProcessCtx,
    script::ScriptAppOperationRequest,
};
use golden_io::{RevisionSubscription, WorkerNotifier};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub(in crate::matrix) enum AutoNext {
    Time(Duration),
    MediaEnd { include_transition: bool },
}

#[derive(Default)]
pub(super) struct Runtime {
    launched: HashMap<uuid::Uuid, Instant>,
    fired: HashSet<(LayerId, uuid::Uuid)>,
    subscriptions: Vec<RevisionSubscription>,
    request: Option<uuid::Uuid>,
    pub(super) deadline: Option<Instant>,
}

impl Runtime {
    pub(super) fn track_launches(&mut self, ids: &HashSet<uuid::Uuid>, now: Instant) {
        self.launched.retain(|id, _| ids.contains(id));
        self.fired.retain(|(_, id)| ids.contains(id));
        for &id in ids {
            self.launched.entry(id).or_insert(now);
        }
        if ids.is_empty() {
            self.disarm();
        }
    }

    pub(super) fn disarm(&mut self) {
        self.deadline = None;
        self.subscriptions.clear();
    }
}

impl Control {
    pub(super) fn wake_auto_next(&self) {
        self.auto_next_dirty.store(true, Ordering::Release);
        if let Some(waker) = self.auto_next_waker.get() {
            let _ = waker.wake();
        }
    }
}

impl PlaybackControl {
    pub(crate) fn request_auto_next(&self, ctx: &mut ProcessCtx, source: NodeId, root: NodeUuid) {
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        if !state.video_enabled || state.auto_next.launched.is_empty() || state.auto_next.request.is_some() {
            return;
        }
        if !self.0.auto_next_dirty.load(Ordering::Acquire)
            && state.auto_next.deadline.is_none_or(|at| at > Instant::now())
        {
            return;
        }
        state.auto_next.deadline = None;
        state.schedule_video_deadline();
        if state.video_request.is_some() || self.0.video_dirty.load(Ordering::Acquire) {
            // Route reconciliation will wake us with the committed Video uses.
            return;
        }
        let token = uuid::Uuid::new_v4();
        let request = ScriptAppOperationRequest::reserve(
            &format!("matrix-auto-next:{token}"),
            super::super::controls::NAMESPACE,
            "auto-next",
        )
        .and_then(|reservation| {
            reservation
                .finish(serde_json::json!([root, token]))
                .map_err(|error| error.to_string())
        });
        match request {
            Ok(request) => {
                state.auto_next.request = Some(token);
                ctx.enqueue_app_operation(source, Arc::downgrade(&state.video_owner), request);
            }
            Err(error) => {
                self.0.auto_next_dirty.store(false, Ordering::Release);
                ctx.set_node_warning_with(source, Some("auto-next"), error, None);
            }
        }
    }

    pub(crate) fn auto_next_result(&self, ctx: &mut ProcessCtx, source: NodeId, event: &CustomEvent) {
        if event.topic != golden_core::script::APP_OPERATION_RESULT_TOPIC || event.origin != Some(source) {
            return;
        }
        let reply = &event.payload["args"][0];
        let Some(token) = reply["request_id"]
            .as_str()
            .and_then(|id| id.strip_prefix("matrix-auto-next:"))
            .and_then(|id| id.parse().ok())
        else {
            return;
        };
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        if state.auto_next.request == Some(token) {
            state.auto_next.request = None;
        }
        if reply["status"] == "rejected" {
            ctx.set_node_warning_with(
                source,
                Some("auto-next"),
                reply["message"].as_str().unwrap_or("Cell Auto Next failed"),
                None,
            );
        } else {
            ctx.clear_node_warning(source, Some("auto-next"));
        }
    }

    fn select_advance(&self, engine: &AppEngine, media: MediaId) -> Result<Option<(LayerId, ColumnId)>, String> {
        let snapshot = self.snapshot();
        let config = super::super::configuration::index(engine, media, self)?;
        let view = (!config.auto_next.is_empty())
            .then(|| super::super::view::index(engine, media))
            .transpose()?;
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        self.0.auto_next_interested.store(view.is_some(), Ordering::Release);
        state.auto_next.disarm();
        if state.video_request.is_some() || self.0.video_dirty.load(Ordering::Acquire) {
            return Ok(None);
        }
        let Some(view) = view.filter(|_| state.video_enabled && !state.auto_next.launched.is_empty()) else {
            return Ok(None);
        };
        let notifier = {
            let weak = Arc::downgrade(&self.0);
            WorkerNotifier::new(move || {
                if let Some(control) = weak.upgrade() {
                    control.wake_auto_next();
                }
            })
        };
        let overrides: HashMap<_, _> = snapshot.layers.iter().map(|request| (request.layer, request)).collect();
        let now = Instant::now();
        let mut live = HashSet::new();
        let mut watched = HashSet::new();
        let mut next = None;
        for (&(layer, column), &settings) in &config.auto_next {
            let (active, launch) = overrides
                .get(&layer)
                .map_or((snapshot.desired_column, snapshot.launch_id), |row| {
                    (row.column, row.launch_id)
                });
            let Some(launch) = launch.filter(|_| active == Some(column)) else {
                continue;
            };
            if !super::super::view::navigation::column_enabled(engine, &view, column)
                || !engine
                    .nodes
                    .get(config.cell_nodes[&(layer, column)])
                    .is_some_and(|cell| crate::media::routing_enabled(engine, cell))
            {
                continue;
            }
            live.insert((layer, launch));
            if state.auto_next.fired.contains(&(layer, launch)) {
                continue;
            }
            let Some(target) = super::super::view::navigation::layer_cell_target(
                engine,
                &view,
                &config,
                layer,
                Some(column),
                MatrixNavigationDirection::Next,
            ) else {
                continue;
            };
            let deadline = match settings {
                AutoNext::Time(delay) => state
                    .auto_next
                    .launched
                    .get(&launch)
                    .and_then(|start| start.checked_add(delay)),
                AutoNext::MediaEnd { include_transition } => {
                    let Some(video) = state.videos.active_source(layer, launch, column).cloned() else {
                        continue;
                    };
                    if watched.insert(Arc::as_ptr(&video)) {
                        let revisions = video.revisions();
                        state
                            .auto_next
                            .subscriptions
                            .push(revisions.guard_at(revisions.current()).subscribe(notifier.clone()));
                    }
                    let lead = if include_transition {
                        Duration::from_millis(config.transition_for(layer, Some(target)).duration_ms.into())
                    } else {
                        Duration::ZERO
                    };
                    video
                        .sample()?
                        .finite_end_delay()
                        .and_then(|remaining| now.checked_add(remaining.saturating_sub(lead)))
                }
            };
            if let Some(at) = deadline {
                // Stable order for deadlines shared by a Matrix-wide launch.
                let candidate = (at, layer.0.0, target.0.0, launch);
                next = Some(next.map_or(candidate, |previous| candidate.min(previous)));
            }
        }
        state.auto_next.fired.retain(|identity| live.contains(identity));
        let Some((at, layer, column, launch)) = next else {
            return Ok(None);
        };
        if at > now {
            state.auto_next.deadline = Some(at);
            state.schedule_video_deadline();
            return Ok(None);
        }
        let layer = LayerId(NodeUuid(layer));
        state.auto_next.fired.insert((layer, launch));
        Ok(Some((layer, ColumnId(NodeUuid(column)))))
    }
}

pub(in crate::matrix) fn apply(
    engine: &mut AppEngine,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let (root, token): (NodeUuid, uuid::Uuid) = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let id = engine.node_id_by_uuid(root).ok_or("Matrix Final is missing")?;
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
        return Err("not a Matrix Final channel".into());
    };
    let control = channel.matrix_playback().ok_or("not a Matrix Final channel")?.clone();
    let media = channel
        .node_data()
        .parent
        .and_then(|parent| engine.nodes.get(parent))
        .map(|owner| MediaId(owner.node_data().meta.uuid))
        .ok_or("Matrix owner is missing")?;
    {
        let mut state = control.0.state.lock().expect("Matrix playback lock poisoned");
        if state.auto_next.request != Some(token) {
            return Err("Cell Auto Next requires an unconsumed live request".into());
        }
        state.auto_next.request = None;
        control.0.auto_next_dirty.store(false, Ordering::Release);
    }
    if let Some((layer, column)) = control.select_advance(engine, media)? {
        let result = super::launch_cell(engine, media, layer, column, LaunchMode::Configured);
        // Reconsider other Layers after this transaction commits or rejects.
        control.0.wake_auto_next();
        result?;
    }
    Ok(serde_json::Value::Null)
}
