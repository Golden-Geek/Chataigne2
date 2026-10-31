//! One latest ephemeral request, gated by the existing atomic edit-completion receipt.
use super::projection::RenderIndex;
use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{ChannelReference, ChannelSource, MATRIX_PLAYBACK_EVENT, MediaEditResult, MediaId},
};
use golden_core::{
    edit::{Edit, EditCompletion, EditCompletionStatus},
    events::CustomEvent,
    node::Node,
};
use golden_io::{RevisionGuard, RevisionSource};
use std::sync::{
    Arc, Mutex, OnceLock, Weak,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub(crate) struct PlaybackControl(Arc<Control>);
#[derive(Default)]
struct Control {
    state: Mutex<State>,
    revision: RevisionSource,
    // Runtime launches must not change the authoring epoch used by clip binding admission.
    authored: RevisionSource,
    columns: RevisionSource,
    logical: RevisionSource,
    application_parent: OnceLock<Weak<RevisionSource>>,
    dirty: AtomicBool,
}
#[derive(Default)]
struct State {
    current: MatrixPlaybackSnapshot,
    pending: Option<(MatrixPlaybackSnapshot, EditCompletion)>,
    index: Option<Arc<RenderIndex>>,
    timeline: super::timeline::Timeline,
    view: Option<(u64, Arc<super::view::ViewIndex>)>,
    application_columns: Option<(u64, Arc<[ColumnSnapshot]>)>,
}

#[derive(Clone)]
pub(crate) struct RuntimeBinding(Weak<Control>);
impl RuntimeBinding {
    pub fn invalidate_applications(&self) {
        if let Some(control) = self.0.upgrade() {
            PlaybackControl(control).invalidate_applications();
        }
    }
    pub fn version(&self) -> u64 {
        self.0
            .upgrade()
            .map_or(0, |control| control.authored.reader().current())
    }
    pub fn invalidate(&self) {
        if let Some(control) = self.0.upgrade() {
            control.authored.advance();
            control.dirty.store(true, Ordering::Release);
            control.revision.advance();
        }
    }
}

impl PlaybackControl {
    pub(crate) fn bind_application_parent(&self, source: &Arc<RevisionSource>) {
        let _ = self.0.application_parent.set(Arc::downgrade(source));
    }
    fn notify_applications(&self) {
        self.0.logical.advance();
        if let Some(source) = self.0.application_parent.get().and_then(Weak::upgrade) {
            source.advance();
        }
    }
    pub(crate) fn invalidate_applications(&self) {
        self.0.columns.advance();
        self.notify_applications();
    }
    pub(crate) fn application_guard(&self) -> RevisionGuard {
        let reader = self.0.logical.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn application_columns(&self) -> Option<Arc<[ColumnSnapshot]>> {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .application_columns
            .as_ref()
            .filter(|(revision, _)| *revision == self.0.columns.reader().current())
            .map(|(_, columns)| columns.clone())
    }
    pub(crate) fn cache_application_columns(&self, columns: Arc<[ColumnSnapshot]>) {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .application_columns = Some((self.0.columns.reader().current(), columns));
    }
    pub(crate) fn capture_applications(
        &self,
        time: golden_time::TimeSnapshot,
    ) -> Result<super::applications::CapturedApplications, String> {
        let Some(index) = self.cached() else {
            return Err("Matrix app index is stale".into());
        };
        self.with_frame(Some(time), |timeline, frame, playback| {
            timeline
                .applications
                .capture(&index, frame, playback, self.authoring_revision())
        })
        .map(|captured| captured.unwrap_or_default())
    }
    pub fn view(&self) -> Option<Arc<super::view::ViewIndex>> {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .view
            .as_ref()
            .filter(|(revision, _)| *revision == self.authoring_revision())
            .map(|(_, index)| index.clone())
    }
    pub fn cache_view(&self, index: Arc<super::view::ViewIndex>) {
        self.0.state.lock().expect("Matrix playback lock poisoned").view = Some((self.authoring_revision(), index));
    }
    pub fn authoring_revision(&self) -> u64 {
        self.0.authored.reader().current()
    }
    pub fn settle(&self, time: golden_time::TimeSnapshot) {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .timeline
            .settle(time);
    }
    pub fn with_frame<T>(
        &self,
        time: Option<golden_time::TimeSnapshot>,
        capture: impl FnOnce(
            &mut super::timeline::Timeline,
            &super::timeline::Frame,
            &MatrixPlaybackSnapshot,
        ) -> Result<T, String>,
    ) -> Result<Option<T>, String> {
        let desired = self.snapshot();
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        let Some(frame) = state
            .timeline
            .frame(&desired, time, self.0.authored.reader().current())?
        else {
            return Ok(None);
        };
        capture(&mut state.timeline, &frame, &desired).map(Some)
    }
    pub(crate) fn retained_edges(&self) -> Vec<(ChannelReference, ChannelReference)> {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .timeline
            .bindings
            .edges()
    }
    fn admits(&self, column: ColumnId) -> bool {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .timeline
            .accepts(column, self.0.authored.reader().current())
    }
    pub fn snapshot(&self) -> MatrixPlaybackSnapshot {
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        if let Some((_, receipt)) = &state.pending {
            match receipt.status() {
                EditCompletionStatus::Applied => {
                    state.current = state.pending.take().unwrap().0;
                    self.0.revision.advance();
                }
                EditCompletionStatus::Failed => {
                    state.pending = None;
                }
                EditCompletionStatus::Pending => {}
            }
        }
        state.current.clone()
    }
    pub fn stage(&self, desired: MatrixPlaybackSnapshot, receipt: EditCompletion) {
        // Preserve an earlier committed request even when the next request later fails.
        self.snapshot();
        self.0.state.lock().expect("Matrix playback lock poisoned").pending = Some((desired, receipt));
        // Conservative invalidation may also trigger recapture after an aborted request.
        self.0.revision.advance();
        self.notify_applications();
    }
    pub fn guard(&self) -> RevisionGuard {
        let reader = self.0.revision.reader();
        reader.guard_at(reader.current())
    }
    pub fn clear(&self) {
        *self.0.state.lock().expect("Matrix playback lock poisoned") = State::default();
        self.0.dirty.store(true, Ordering::Release);
        self.0.revision.advance();
        self.invalidate_applications();
    }
    pub fn invalidate(&self) {
        self.binding().invalidate();
    }
    pub fn binding(&self) -> RuntimeBinding {
        RuntimeBinding(Arc::downgrade(&self.0))
    }
    pub fn cached(&self) -> Option<Arc<RenderIndex>> {
        if self.0.dirty.load(Ordering::Acquire) {
            None
        } else {
            self.0
                .state
                .lock()
                .expect("Matrix playback lock poisoned")
                .index
                .clone()
        }
    }
    pub fn cache(&self, index: Arc<RenderIndex>) {
        self.0.state.lock().expect("Matrix playback lock poisoned").index = Some(index);
        self.0.dirty.store(false, Ordering::Release);
    }
}

pub(super) fn launch(
    engine: &mut AppEngine,
    media: MediaId,
    column: ColumnId,
    mode: LaunchMode,
) -> Result<MediaEditResult, String> {
    // Column parentage resolves the Final channel in O(1), independently of grid size.
    let id = engine.node_id_by_uuid(column.0).ok_or("Matrix column is missing")?;
    let Some(AppNode::MatrixElement(node)) = engine.nodes.get(id) else {
        return Err("reference is not a Matrix column".into());
    };
    let ElementDefinition::Column {
        definition: column_definition,
    } = node.definition()
    else {
        return Err("reference is not a Matrix column".into());
    };
    let parent = node.node_data().parent.ok_or("Matrix column has no owner")?;
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(parent) else {
        return Err("Matrix Final channel is missing".into());
    };
    let ChannelSource::Matrix {
        output: MatrixOutput::Final { definition },
    } = &root.definition().source
    else {
        return Err("column is outside its Matrix Final channel".into());
    };
    let Some(AppNode::Media(owner)) = root.node_data().parent.and_then(|id| engine.nodes.get(id)) else {
        return Err("Matrix media is missing".into());
    };
    if owner.node_data().meta.uuid != media.0 {
        return Err("column belongs to another Matrix".into());
    }
    let transition = column_definition.transition.unwrap_or(definition.transition);
    let control = root
        .matrix_playback()
        .ok_or("Matrix playback owner is missing")?
        .clone();
    if mode == LaunchMode::Configured && transition.duration_ms != 0 && !control.admits(column) {
        return Err("Matrix interrupted mix retains 32 runtime clips; use Cut or wait for completion".into());
    }
    let desired = MatrixPlaybackSnapshot {
        desired_column: Some(column),
        launch_id: Some(uuid::Uuid::new_v4()),
        transition: if mode == LaunchMode::Cut {
            TransitionDefinition {
                duration_ms: 0,
                ..transition
            }
        } else {
            transition
        },
    };
    let event = CustomEvent::from_latest_payload(MATRIX_PLAYBACK_EVENT, Some(parent), &desired)
        .map_err(|error| error.to_string())?;
    engine.edits.push(Edit::EmitCustomEvent { event });
    let receipt = engine.edits.completion();
    control.stage(desired.clone(), receipt);
    Ok(MediaEditResult::MatrixLaunched { snapshot: desired })
}

pub(crate) fn unload(engine: &mut AppEngine, media: MediaId) -> Result<MediaEditResult, String> {
    let parent = root(engine, media)?;
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(parent) else {
        unreachable!()
    };
    let control = node.matrix_playback().ok_or("Matrix playback is missing")?.clone();
    let desired = MatrixPlaybackSnapshot {
        desired_column: None,
        launch_id: Some(uuid::Uuid::new_v4()),
        transition: TransitionDefinition {
            duration_ms: 0,
            curve: TransitionCurve::Linear,
        },
    };
    let event =
        CustomEvent::from_latest_payload(MATRIX_PLAYBACK_EVENT, Some(parent), &desired).map_err(|e| e.to_string())?;
    engine.edits.push(Edit::EmitCustomEvent { event });
    control.stage(desired.clone(), engine.edits.completion());
    Ok(MediaEditResult::MatrixPlayback { snapshot: desired })
}

pub(crate) fn root(engine: &AppEngine, media: MediaId) -> Result<golden_core::node::NodeId, String> {
    let (id, kind) = crate::media::authoring::media(engine, media)?;
    if kind != crate::media::protocol::MediaKind::Matrix {
        return Err("media is not a Matrix".into());
    }
    let AppNode::Media(node) = engine.nodes.get(id).unwrap() else {
        unreachable!()
    };
    if let Some(uuid) = node.matrix_root() {
        return engine
            .node_id_by_uuid(uuid)
            .ok_or("Matrix Final channel is missing".into());
    }
    for child in super::document::children(engine, id)? {
        if let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(child)
            && matches!(
                channel.definition().source,
                ChannelSource::Matrix {
                    output: MatrixOutput::Final { .. }
                }
            )
        {
            node.cache_matrix_root(channel.node_data().meta.uuid);
            return Ok(child);
        }
    }
    Err("Matrix Final channel is missing".into())
}

pub(super) fn inspect(engine: &AppEngine, media: MediaId) -> Result<MediaEditResult, String> {
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(root(engine, media)?) else {
        unreachable!()
    };
    Ok(MediaEditResult::MatrixPlayback {
        snapshot: node
            .matrix_playback()
            .ok_or("Matrix playback owner is missing")?
            .snapshot(),
    })
}
