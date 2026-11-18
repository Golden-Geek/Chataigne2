//! Cached authored addresses/control policy, independent of render/source demand.
use super::{document::Document, protocol::*};
use crate::app::{AppEngine, AppNode};
use golden_core::node::{Node, NodeId};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
pub(crate) struct Configuration {
    pub layers: HashSet<LayerId>,
    pub cell_nodes: HashMap<(LayerId, ColumnId), NodeId>,
    pub disabled: HashMap<ColumnId, Vec<LayerId>>,
    pub overrides: HashMap<ColumnId, HashMap<LayerId, TransitionDefinition>>,
    pub transitions: HashMap<ColumnId, Option<TransitionDefinition>>,
    pub layer_base: TransitionDefinition,
    pub layer_overrides: HashMap<LayerId, TransitionDefinition>,
    pub playable: HashSet<(LayerId, ColumnId)>,
    pub(super) auto_next: HashMap<(LayerId, ColumnId), super::playback::AutoNext>,
}
impl Configuration {
    pub(super) fn transition_for(&self, layer: LayerId, column: Option<ColumnId>) -> TransitionDefinition {
        column
            .and_then(|column| self.overrides.get(&column).and_then(|rows| rows.get(&layer)).copied())
            .or_else(|| column.and_then(|column| self.transitions.get(&column).copied().flatten()))
            .unwrap_or_else(|| self.layer_overrides.get(&layer).copied().unwrap_or(self.layer_base))
    }
}
pub(crate) fn index(
    engine: &AppEngine,
    media: crate::media::protocol::MediaId,
    control: &super::playback::PlaybackControl,
) -> Result<Arc<Configuration>, String> {
    if let Some(index) = control.configuration() {
        return Ok(index);
    }
    let document = Document::load(engine, media)?;
    let mut disabled = HashMap::<_, Vec<_>>::new();
    let mut overrides = HashMap::<_, HashMap<_, _>>::new();
    let mut cell_apps = HashMap::new();
    let mut auto_next = HashMap::new();
    for id in &document.elements {
        match engine.nodes.get(*id) {
            Some(AppNode::MatrixElement(node)) => node.bind_runtime(control.binding()),
            Some(AppNode::MatrixColumn(node)) => node.bind_runtime(control.binding()),
            Some(AppNode::MatrixColumnGroup(node)) => node.bind_runtime(control.binding()),
            Some(AppNode::MatrixCell(node)) => node.bind_runtime(control.binding()),
            _ => unreachable!(),
        }
    }
    for cell in &document.snapshot.cells {
        let Some(AppNode::MatrixCell(node)) = engine.nodes.get(document.cells[&(cell.layer, cell.definition.column)])
        else {
            unreachable!()
        };
        if !node.node_data().meta.enabled {
            disabled.entry(cell.definition.column).or_default().push(cell.layer);
        }
        if let Some(settings) = node.auto_next() {
            auto_next.insert((cell.layer, cell.definition.column), settings);
        }
        if let Some(transition) = cell.definition.transition {
            overrides
                .entry(cell.definition.column)
                .or_default()
                .insert(cell.layer, transition);
        }
        if crate::applications::control_node(engine, cell.definition.application)
            .is_some_and(|app| app.configured() && app.enabled(engine))
        {
            cell_apps.insert((cell.layer, cell.definition.column), cell.definition.application);
        }
    }
    let mut durations = HashMap::<u32, usize>::new();
    for layer in document.snapshot.items.iter().filter_map(|item| item.layer) {
        *durations.entry(layer.duration_ms).or_default() += 1;
    }
    let layer_base = TransitionDefinition {
        duration_ms: durations
            .into_iter()
            .max_by_key(|(duration, count)| (*count, std::cmp::Reverse(*duration)))
            .map_or(250, |(duration, _)| duration),
        curve: TransitionCurve::Smooth,
    };
    let layer_overrides = document
        .snapshot
        .items
        .iter()
        .filter_map(|item| match (item.id, item.layer) {
            (MatrixItemId::Layer { id }, Some(layer)) if layer.transition() != layer_base => {
                Some((id, layer.transition()))
            }
            _ => None,
        })
        .collect();
    let playable = document
        .snapshot
        .cells
        .iter()
        .filter(|cell| cell.media.is_some() || cell_apps.contains_key(&(cell.layer, cell.definition.column)))
        .map(|cell| (cell.layer, cell.definition.column))
        .collect();
    let index = Arc::new(Configuration {
        playable,
        auto_next,
        layer_base,
        layer_overrides,
        layers: document
            .snapshot
            .items
            .iter()
            .filter_map(|item| match item.id {
                MatrixItemId::Layer { id } => Some(id),
                _ => None,
            })
            .collect(),
        cell_nodes: document.cells,
        disabled,
        overrides,
        transitions: document
            .snapshot
            .columns
            .iter()
            .map(|column| (column.id, column.definition.transition))
            .collect(),
    });
    control.cache_configuration(index.clone());
    Ok(index)
}
