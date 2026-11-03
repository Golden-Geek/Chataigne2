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
    pub transitions: HashMap<ColumnId, TransitionDefinition>,
    pub cell_apps: HashMap<(LayerId, ColumnId), crate::applications::protocol::ApplicationId>,
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
    for id in &document.elements {
        match engine.nodes.get(*id) {
            Some(AppNode::MatrixElement(node)) => node.bind_runtime(control.binding()),
            Some(AppNode::MatrixColumn(node)) => node.bind_runtime(control.binding()),
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
        if let Some(transition) = cell.definition.transition {
            overrides
                .entry(cell.definition.column)
                .or_default()
                .insert(cell.layer, transition);
        }
        if crate::applications::control_node(engine, cell.definition.application)
            .is_some_and(|app| app.configured() && app.node_data().effective_enabled)
        {
            cell_apps.insert((cell.layer, cell.definition.column), cell.definition.application);
        }
    }
    let index = Arc::new(Configuration {
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
        cell_apps,
        transitions: document
            .snapshot
            .columns
            .iter()
            .map(|column| {
                (
                    column.id,
                    column
                        .definition
                        .transition
                        .unwrap_or(document.snapshot.definition.transition),
                )
            })
            .collect(),
    });
    control.cache_configuration(index.clone());
    Ok(index)
}
