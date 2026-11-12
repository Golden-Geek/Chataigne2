//! Deferred, receipt-gated Cut filters. They describe scope, never a playback event queue.
use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{MATRIX_PLAYBACK_EVENT, MediaEditResult, MediaId},
};
use golden_core::{
    edit::Edit,
    events::CustomEvent,
    node::{Node, NodeId, NodeUuid},
};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct Scope {
    pub columns: HashSet<ColumnId>,
    pub layers: HashSet<LayerId>,
    pub cells: HashSet<(LayerId, ColumnId)>,
    pub suppressed: HashSet<(Option<LayerId>, uuid::Uuid)>,
}
#[derive(Default)]
pub(super) struct Cuts {
    all: Option<u64>,
    columns: HashMap<ColumnId, u64>,
    layers: HashMap<LayerId, u64>,
    cells: HashMap<(LayerId, ColumnId), u64>,
    suppressed: HashSet<(Option<LayerId>, uuid::Uuid)>,
}
impl Cuts {
    pub fn stop_all(&mut self, order: u64) {
        *self = Self {
            all: Some(order),
            ..Default::default()
        };
    }
    pub fn extend(&mut self, scope: Scope, order: u64) {
        self.columns.extend(scope.columns.into_iter().map(|id| (id, order)));
        self.layers.extend(scope.layers.into_iter().map(|id| (id, order)));
        self.cells.extend(scope.cells.into_iter().map(|id| (id, order)));
        self.suppressed.extend(scope.suppressed);
    }
    pub fn suppresses(&self, layer: Option<LayerId>, launch: Option<uuid::Uuid>) -> bool {
        launch.is_some_and(|id| self.suppressed.contains(&(layer, id)))
    }
    pub fn removes(&self, layer: Option<LayerId>, clip: super::timeline::Clip) -> bool {
        self.all.is_some_and(|cut| clip.order <= cut)
            || self.columns.get(&clip.column).is_some_and(|cut| clip.order <= *cut)
            || layer.is_some_and(|layer| {
                self.layers.get(&layer).is_some_and(|cut| clip.order <= *cut)
                    || self
                        .cells
                        .get(&(layer, clip.column))
                        .is_some_and(|cut| clip.order <= *cut)
            })
    }
    pub fn layers(&self) -> impl Iterator<Item = LayerId> + '_ {
        self.layers
            .keys()
            .copied()
            .chain(self.cells.keys().map(|(layer, _)| *layer))
    }
    pub fn is_empty(&self) -> bool {
        self.all.is_none() && self.columns.is_empty() && self.layers.is_empty() && self.cells.is_empty()
    }
    pub fn len(&self) -> usize {
        usize::from(self.all.is_some())
            + self.columns.len()
            + self.layers.len()
            + self.cells.len()
            + self.suppressed.len()
    }
}

fn scope(engine: &AppEngine, root: NodeId, owner: NodeUuid) -> Result<Scope, String> {
    let owner = engine.node_id_by_uuid(owner).ok_or("Stop owner is missing")?;
    let mut ancestor = Some(owner);
    while ancestor != Some(root) {
        let node = ancestor
            .and_then(|id| engine.nodes.get(id))
            .ok_or("Stop belongs to another Matrix")?;
        if matches!(node, AppNode::Media(_)) {
            return Err("Stop belongs to another Matrix".into());
        }
        ancestor = node.node_data().parent;
    }
    let mut scope = Scope::default();
    let mut pending = vec![owner];
    while let Some(id) = pending.pop() {
        let node = engine.nodes.get(id).ok_or("Stop scope changed")?;
        let uuid = node.node_data().meta.uuid;
        match node {
            AppNode::MatrixColumn(_) => {
                scope.columns.insert(ColumnId(uuid));
            }
            AppNode::MatrixElement(element) if element.is_layer() => {
                scope.layers.insert(LayerId(uuid));
            }
            AppNode::MatrixColumnGroup(_) | AppNode::MatrixElement(_) => {
                // Only visit the selected axis subtree, never Cell media or unrelated Matrix rows.
                pending.extend(super::document::children(engine, id)?.into_iter().filter(|child| {
                    matches!(
                        engine.nodes.get(*child),
                        Some(AppNode::MatrixColumn(_) | AppNode::MatrixColumnGroup(_) | AppNode::MatrixElement(_))
                    )
                }));
            }
            AppNode::MatrixCell(cell) => {
                let parent = cell
                    .node_data()
                    .parent
                    .and_then(|id| engine.nodes.get(id))
                    .filter(|node| matches!(node, AppNode::MatrixElement(element) if element.is_layer()))
                    .ok_or("Cell has no Layer")?;
                scope
                    .cells
                    .insert((LayerId(parent.node_data().meta.uuid), cell.column()));
            }
            _ => return Err("node has no scoped Matrix Stop".into()),
        }
    }
    Ok(scope)
}

pub(super) fn stop(engine: &mut AppEngine, media: MediaId, owner: NodeUuid) -> Result<MediaEditResult, String> {
    let root = super::playback::root(engine, media)?;
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(root) else {
        unreachable!()
    };
    let control = channel.matrix_playback().ok_or("Matrix playback is missing")?.clone();
    let mut desired = control.snapshot();
    let mut scope = scope(engine, root, owner)?;
    let active_columns = control.active_columns();
    if active_columns.is_empty() {
        return Ok(MediaEditResult::MatrixPlayback { snapshot: desired });
    }
    let index = super::configuration::index(engine, media, &control)?;
    control.retain_playing_layers(&mut scope.layers, &index);
    scope.columns.retain(|column| active_columns.contains(column));
    if scope.columns.is_empty() && scope.layers.is_empty() && scope.cells.is_empty() {
        return Ok(MediaEditResult::MatrixPlayback { snapshot: desired });
    }
    let mut requests = desired
        .layers
        .iter()
        .cloned()
        .map(|request| (request.layer, request))
        .collect::<HashMap<_, _>>();
    for request in requests.values_mut() {
        if scope.layers.contains(&request.layer)
            || request
                .column
                .is_some_and(|column| scope.columns.contains(&column) || scope.cells.contains(&(request.layer, column)))
        {
            request.column = None;
            request.transition.duration_ms = 0;
            if let Some(id) = request.launch_id {
                scope.suppressed.insert((Some(request.layer), id));
            }
        }
    }
    let mut stopped_layers = scope.layers.clone();
    stopped_layers.extend(scope.cells.iter().filter_map(|(layer, column)| {
        (Some(*column) == desired.desired_column && !requests.contains_key(layer)).then_some(*layer)
    }));
    for layer in stopped_layers {
        requests.entry(layer).or_insert_with(|| {
            let launch = desired.launch_id.unwrap_or_else(uuid::Uuid::new_v4);
            scope.suppressed.insert((Some(layer), launch));
            LayerPlaybackRequest {
                layer,
                column: None,
                launch_id: Some(launch),
                individual: false,
                transition: TransitionDefinition {
                    duration_ms: 0,
                    curve: TransitionCurve::Linear,
                },
            }
        });
    }
    if desired
        .desired_column
        .is_some_and(|column| scope.columns.contains(&column))
    {
        desired.desired_column = None;
        desired.transition.duration_ms = 0;
        if let Some(id) = desired.launch_id {
            scope.suppressed.insert((None, id));
        }
    }
    if requests.len() > super::bindings::MAX_BINDINGS
        || !control.stop_capacity(scope.columns.len() + scope.layers.len() + scope.cells.len() + scope.suppressed.len())
    {
        return Err(
            "Matrix deferred stop exceeds the bounded runtime scope; capture or Stop All before retrying".into(),
        );
    }
    let mut requests = requests.into_values().collect::<Vec<_>>();
    requests.sort_by_key(|request| request.layer.0.0);
    desired.layers = requests.into();
    desired.request_id = Some(uuid::Uuid::new_v4());
    let videos = super::video::Prepared::capture(engine, &desired, &index, control.authoring_revision())?;
    let event = CustomEvent::from_latest_payload(MATRIX_PLAYBACK_EVENT, Some(root), &desired)
        .map_err(|error| error.to_string())?;
    engine.edits.push(Edit::EmitCustomEvent { event });
    control.stage_with_stop(desired.clone(), engine.edits.completion(), Some(scope), Some(videos));
    Ok(MediaEditResult::MatrixPlayback { snapshot: desired })
}
