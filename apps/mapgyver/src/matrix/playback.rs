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
    sources: OnceLock<Arc<crate::media::retention::RuntimeSources>>,
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
    configuration: Option<(u64, Arc<super::configuration::Configuration>)>,
    current: MatrixPlaybackSnapshot,
    version: u64,
    epochs: std::collections::HashMap<uuid::Uuid, u64>,
    pending: Option<(MatrixPlaybackSnapshot, EditCompletion, u64)>,
    index: Option<Arc<RenderIndex>>,
    timeline: super::timeline::Timeline,
    view: Option<(u64, Arc<super::view::ViewIndex>)>,
    application_columns: Option<(u64, Arc<[crate::applications::demand::DemandColumn]>)>,
}

#[derive(Clone)]
pub(crate) struct RuntimeBinding(Weak<Control>);
impl RuntimeBinding {
    pub(crate) fn sources(&self) -> Option<Vec<ChannelReference>> {
        let control = self.0.upgrade()?;
        let state = control.state.lock().expect("Matrix playback lock poisoned");
        Some(
            state
                .timeline
                .bindings
                .edges()
                .into_iter()
                .map(|(_, source)| source)
                .collect(),
        )
    }
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
    pub(super) fn configuration(&self) -> Option<Arc<super::configuration::Configuration>> {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .configuration
            .as_ref()
            .filter(|(revision, _)| *revision == self.authoring_revision())
            .map(|(_, index)| index.clone())
    }
    pub(super) fn cache_configuration(&self, index: Arc<super::configuration::Configuration>) {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .configuration = Some((self.authoring_revision(), index));
    }
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
    pub(crate) fn application_columns(&self) -> Option<Arc<[crate::applications::demand::DemandColumn]>> {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .application_columns
            .as_ref()
            .filter(|(revision, _)| *revision == self.0.columns.reader().current())
            .map(|(_, columns)| columns.clone())
    }
    pub(crate) fn cache_application_columns(&self, columns: Arc<[crate::applications::demand::DemandColumn]>) {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .application_columns = Some((self.0.columns.reader().current(), columns));
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
        let version = state.version;
        let State { timeline, epochs, .. } = &mut *state;
        let Some(frame) = timeline.frame(&desired, time, version, epochs)? else {
            return Ok(None);
        };
        capture(timeline, &frame, &desired).map(Some)
    }
    pub(crate) fn bind_sources(
        &self,
        sources: &Arc<crate::media::retention::RuntimeSources>,
        media: MediaId,
    ) -> Result<(), String> {
        let _ = self.0.sources.set(sources.clone());
        sources.register(media, self.binding())
    }
    pub(crate) fn retained_edges(&self) -> Vec<(ChannelReference, ChannelReference)> {
        let edges = self
            .0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .timeline
            .bindings
            .edges();
        match self.0.sources.get() {
            Some(sources) => sources.edges(edges),
            None => edges,
        }
    }
    fn admits(&self, request: &MatrixPlaybackSnapshot) -> bool {
        self.0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .timeline
            .accepts_request(request, self.0.authored.reader().current())
    }
    fn retained_layers(&self) -> std::collections::HashSet<LayerId> {
        let state = self.0.state.lock().expect("Matrix playback lock poisoned");
        let mut layers = state.timeline.bindings.layers();
        layers.extend(state.timeline.applications.layers());
        layers
    }
    pub fn snapshot(&self) -> MatrixPlaybackSnapshot {
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        if let Some((_, receipt, _)) = &state.pending {
            match receipt.status() {
                EditCompletionStatus::Applied => {
                    let (desired, _, version) = state.pending.take().unwrap();
                    if state.current.launch_id != desired.launch_id {
                        state.version = version;
                    }
                    let ids = desired
                        .launch_id
                        .into_iter()
                        .chain(desired.layers.iter().filter_map(|request| request.launch_id))
                        .collect::<std::collections::HashSet<_>>();
                    state.epochs.retain(|id, _| ids.contains(id));
                    for id in ids {
                        state.epochs.entry(id).or_insert(version);
                    }
                    state.current = desired;
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
        self.0.state.lock().expect("Matrix playback lock poisoned").pending =
            Some((desired, receipt, self.authoring_revision()));
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
        if let Some(sources) = self.0.sources.get() {
            sources.prune();
        }
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
    let Some(AppNode::MatrixColumn(node)) = engine.nodes.get(id) else {
        return Err("reference is not a Matrix column".into());
    };
    let column_definition = node.definition();
    let parent = node.node_data().parent.ok_or("Matrix column has no owner")?;
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(parent) else {
        return Err("Matrix Final channel is missing".into());
    };
    let ChannelSource::Matrix {
        output: MatrixOutput::Final { .. },
    } = &crate::media::authoring::effective_definition(engine, root)?.source
    else {
        return Err("column is outside its Matrix Final channel".into());
    };
    let Some(AppNode::Media(owner)) = root.node_data().parent.and_then(|id| engine.nodes.get(id)) else {
        return Err("Matrix media is missing".into());
    };
    if owner.node_data().meta.uuid != media.0 {
        return Err("column belongs to another Matrix".into());
    }
    let control = root
        .matrix_playback()
        .ok_or("Matrix playback owner is missing")?
        .clone();
    let index = super::configuration::index(engine, media, &control)?;
    let transition = column_definition.transition.unwrap_or(index.layer_base);
    let previous = control.snapshot();
    let launch = uuid::Uuid::new_v4();
    let mut layers = std::collections::HashMap::new();
    let previous_layers = previous
        .layers
        .iter()
        .map(|request| (request.layer, request))
        .collect::<std::collections::HashMap<_, _>>();
    let disabled_layers = index
        .disabled
        .get(&column)
        .into_iter()
        .flatten()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if let Some(disabled) = index.disabled.get(&column) {
        for layer in disabled {
            layers.insert(
                *layer,
                previous_layers
                    .get(layer)
                    .map(|request| (*request).clone())
                    .unwrap_or(LayerPlaybackRequest {
                        layer: *layer,
                        column: previous.desired_column,
                        launch_id: previous.launch_id,
                        transition: previous.transition,
                        individual: false,
                    }),
            );
        }
    }
    if column_definition.transition.is_none() && mode == LaunchMode::Configured {
        let mut active = control.retained_layers();
        active.extend(index.transition_layers.iter().copied());
        for layer in active {
            if !disabled_layers.contains(&layer)
                && let Some(transition) = index.layer_overrides.get(&layer)
            {
                layers.insert(
                    layer,
                    LayerPlaybackRequest {
                        layer,
                        column: Some(column),
                        launch_id: Some(launch),
                        transition: *transition,
                        individual: false,
                    },
                );
            }
        }
    }
    for (layer, transition) in index.overrides.get(&column).into_iter().flatten() {
        if !disabled_layers.contains(layer) {
            layers.insert(
                *layer,
                LayerPlaybackRequest {
                    layer: *layer,
                    column: Some(column),
                    launch_id: Some(launch),
                    transition: selected_transition(*transition, mode),
                    individual: false,
                },
            );
        }
    }
    // An unused base mix must not extend old Column leases beyond the actual Layer fades.
    let base_used = layers.len() < index.layers.len();
    if layers.len() > super::bindings::MAX_BINDINGS {
        return Err("Matrix launch exceeds 4096 active Layer requests".into());
    }
    let mut layers = layers.into_values().collect::<Vec<_>>();
    layers.sort_by_key(|request| request.layer.0.0);
    let desired = MatrixPlaybackSnapshot {
        request_id: Some(launch),
        layers: layers.into(),
        desired_column: Some(column),
        launch_id: Some(launch),
        transition: if mode == LaunchMode::Cut || !base_used {
            TransitionDefinition {
                duration_ms: 0,
                ..transition
            }
        } else {
            transition
        },
    };
    if !control.admits(&desired) {
        return Err("Matrix interrupted mix retains 32 runtime clips; use Cut or wait for completion".into());
    }
    let event = CustomEvent::from_latest_payload(MATRIX_PLAYBACK_EVENT, Some(parent), &desired)
        .map_err(|error| error.to_string())?;
    engine.edits.push(Edit::EmitCustomEvent { event });
    let receipt = engine.edits.completion();
    control.stage(desired.clone(), receipt);
    Ok(MediaEditResult::MatrixLaunched { snapshot: desired })
}

fn selected_transition(mut transition: TransitionDefinition, mode: LaunchMode) -> TransitionDefinition {
    if mode == LaunchMode::Cut {
        transition.duration_ms = 0;
    }
    transition
}

pub(super) fn launch_cell(
    engine: &mut AppEngine,
    media: MediaId,
    layer: LayerId,
    column: ColumnId,
    mode: LaunchMode,
) -> Result<MediaEditResult, String> {
    let root_id = root(engine, media)?;
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root_id) else {
        unreachable!()
    };
    let control = root.matrix_playback().ok_or("Matrix playback is missing")?.clone();
    let index = super::configuration::index(engine, media, &control)?;
    if !index.layers.contains(&layer) {
        return Err("Layer belongs to another Matrix or is missing".into());
    }
    let fallback = index
        .transitions
        .get(&column)
        .ok_or("Column belongs to another Matrix or is missing")?
        .unwrap_or_else(|| index.layer_overrides.get(&layer).copied().unwrap_or(index.layer_base));
    if index
        .cell_nodes
        .get(&(layer, column))
        .and_then(|id| engine.nodes.get(*id))
        .is_some_and(|node| !node.node_data().effective_enabled)
    {
        return Ok(MediaEditResult::MatrixPlayback {
            snapshot: control.snapshot(),
        });
    }
    let transition = selected_transition(
        index
            .overrides
            .get(&column)
            .and_then(|overrides| overrides.get(&layer))
            .copied()
            .unwrap_or(fallback),
        mode,
    );
    if transition.duration_ms != 0
        && !control
            .0
            .state
            .lock()
            .expect("Matrix playback lock poisoned")
            .timeline
            .accepts_cell(layer, column, control.authoring_revision())
    {
        return Err("Layer interrupted mix retains 32 clips; use Cut or wait for completion".into());
    }
    let mut desired = control.snapshot();
    let launch = uuid::Uuid::new_v4();
    let mut layers = desired.layers.to_vec();
    layers.retain(|request| request.layer != layer);
    layers.push(LayerPlaybackRequest {
        layer,
        column: Some(column),
        launch_id: Some(launch),
        transition,
        individual: true,
    });
    layers.sort_by_key(|request| request.layer.0.0);
    desired.layers = layers.into();
    desired.request_id = Some(launch);
    let event = CustomEvent::from_latest_payload(MATRIX_PLAYBACK_EVENT, Some(root_id), &desired)
        .map_err(|error| error.to_string())?;
    engine.edits.push(Edit::EmitCustomEvent { event });
    control.stage(desired.clone(), engine.edits.completion());
    Ok(MediaEditResult::MatrixLaunched { snapshot: desired })
}

pub(crate) fn unload(engine: &mut AppEngine, media: MediaId) -> Result<MediaEditResult, String> {
    let parent = root(engine, media)?;
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(parent) else {
        unreachable!()
    };
    let control = node.matrix_playback().ok_or("Matrix playback is missing")?.clone();
    let launch = uuid::Uuid::new_v4();
    let desired = MatrixPlaybackSnapshot {
        request_id: Some(launch),
        layers: Arc::from([]),
        desired_column: None,
        launch_id: Some(launch),
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
            && channel.is_matrix_final()
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
