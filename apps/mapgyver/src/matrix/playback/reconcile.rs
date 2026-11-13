//! One-shot route invalidation enters the existing bounded actor operation service.
use super::*;
use golden_core::{
    node::{NodeId, NodeUuid},
    process_ctx::ProcessCtx,
    script::ScriptAppOperationRequest,
};

impl PlaybackControl {
    pub(crate) fn video_refresh_result(&self, ctx: &mut ProcessCtx, source: NodeId, event: &CustomEvent) {
        if event.topic != golden_core::script::APP_OPERATION_RESULT_TOPIC || event.origin != Some(source) {
            return;
        }
        let reply = &event.payload["args"][0];
        let Some(token) = reply["request_id"]
            .as_str()
            .and_then(|id| id.strip_prefix("matrix-video:"))
            .and_then(|id| id.parse().ok())
        else {
            return;
        };
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        if state.video_request == Some(token) {
            state.video_request = None;
        }
        if reply["status"] == "rejected" {
            ctx.set_node_warning_with(
                source,
                Some("video-policy"),
                reply["message"].as_str().unwrap_or("Video reconciliation rejected"),
                None,
            );
        }
    }
    pub(super) fn watch_videos(&self, state: &mut State) {
        let (Some(plan), Some(waker)) = (&state.video_plan, state.waker.clone()) else {
            return;
        };
        let weak = Arc::downgrade(&self.0);
        let notifier = golden_io::WorkerNotifier::new(move || {
            if let Some(control) = weak.upgrade() {
                control.video_dirty.store(true, Ordering::Release);
                let _ = waker.wake();
            }
        });
        state.video_subscriptions = plan.subscribe(notifier.clone());
        let reader = self.0.video_sources.reader();
        state
            .video_subscriptions
            .push(reader.guard_at(reader.current()).subscribe(notifier));
    }
    pub(crate) fn request_video_refresh(&self, ctx: &mut ProcessCtx, source: NodeId, root: NodeUuid) {
        if !self.0.video_dirty.load(Ordering::Acquire) {
            return;
        }
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        if state.video_request.is_some() || state.video_plan.is_none() {
            return;
        }
        let token = uuid::Uuid::new_v4();
        let result = (|| {
            let reservation = ScriptAppOperationRequest::reserve(
                &format!("matrix-video:{token}"),
                super::super::controls::NAMESPACE,
                "refresh-video",
            )?;
            let request = reservation
                .finish(serde_json::to_value((root, token)).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
            ctx.enqueue_app_operation(source, Arc::downgrade(&state.video_owner), request);
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => state.video_request = Some(token),
            Err(error) => ctx.set_node_warning_with(source, Some("video-policy"), error, None),
        }
    }
    pub(crate) fn claim_video_refresh(&self, token: uuid::Uuid) -> bool {
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        if state.video_request != Some(token) {
            return false;
        }
        state.video_request = None;
        self.0.video_dirty.store(false, Ordering::Release);
        true
    }
    pub(crate) fn refresh_videos(&self, engine: &mut AppEngine, root: NodeId) -> Result<(), String> {
        self.snapshot();
        let (Some(plan), selections) = ({
            let state = self.0.state.lock().expect("Matrix playback lock poisoned");
            (state.video_plan.clone(), state.videos.selections())
        }) else {
            return Ok(());
        };
        let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(root) else {
            return Err("Matrix Final is missing".into());
        };
        let owner = channel
            .node_data()
            .parent
            .and_then(|id| engine.nodes.get(id))
            .ok_or("Matrix owner is missing")?;
        let media = MediaId(owner.node_data().meta.uuid);
        let index = super::super::configuration::index(engine, media, self)?;
        let plan = plan.refresh(engine, &index.layers, selections)?;
        // Metadata only until this enclosing actor transaction commits.
        let event = CustomEvent::from_latest_payload("mapgyver.matrix.video.refresh", Some(root), &())
            .map_err(|error| error.to_string())?;
        engine.edits.push(Edit::EmitCustomEvent { event });
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        state.pending = Some(Pending {
            videos: Some(plan),
            desired: state.current.clone(),
            receipt: engine.edits.completion(),
            version: state.version,
            stop: None,
            refresh: true,
        });
        if let Some(waker) = &state.waker {
            let _ = waker.wake();
        }
        Ok(())
    }
}
