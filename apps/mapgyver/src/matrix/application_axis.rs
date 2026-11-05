//! Shared immutable application projection of the actual Column axis and sparse Cells.
use super::{document::Document, playback::PlaybackControl, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    applications::{demand::*, protocol::ApplicationId},
    media::protocol::MediaId,
};
use golden_core::node::{Node, NodeId};
use std::{collections::HashMap, sync::Arc};

pub(crate) struct ApplicationAxis {
    pub columns: Vec<DemandColumn>,
    pub spans: Vec<DemandSpan>,
    pub cell_apps: HashMap<(LayerId, ColumnId), ApplicationId>,
    column_apps: HashMap<ColumnId, ApplicationId>,
    column_groups: HashMap<ColumnId, ColumnGroupId>,
    group_parents: HashMap<ColumnGroupId, ColumnGroupId>,
    group_apps: HashMap<ColumnGroupId, ApplicationId>,
}

impl ApplicationAxis {
    pub fn column_groups(&self, column: ColumnId) -> impl Iterator<Item = ColumnGroupId> + '_ {
        std::iter::successors(self.column_groups.get(&column).copied(), |group| {
            self.group_parents.get(group).copied()
        })
    }
    pub fn column_applications(&self, column: ColumnId) -> impl Iterator<Item = ApplicationId> + '_ {
        self.column_apps.get(&column).copied().into_iter().chain(
            self.column_groups(column)
                .filter_map(|group| self.group_apps.get(&group).copied()),
        )
    }
}

pub(crate) fn capture(
    engine: &AppEngine,
    root: NodeId,
    control: &PlaybackControl,
    maximum: usize,
) -> Result<Arc<ApplicationAxis>, String> {
    let axis = if let Some(axis) = control.application_axis() {
        axis
    } else {
        let owner = engine
            .nodes
            .get(root)
            .and_then(|node| node.node_data().parent)
            .and_then(|id| engine.nodes.get(id))
            .ok_or("Matrix media is missing")?;
        let document = Document::load(engine, MediaId(owner.node_data().meta.uuid))?;
        for_document(engine, &document, control)?
    };
    if axis.columns.len() > maximum {
        return Err("logical Matrix aggregate column capacity exceeded".into());
    }
    Ok(axis)
}

pub(super) fn for_document(
    engine: &AppEngine,
    document: &Document,
    control: &PlaybackControl,
) -> Result<Arc<ApplicationAxis>, String> {
    if let Some(axis) = control.application_axis() {
        return Ok(axis);
    }
    let configured = |application| {
        crate::applications::control_node(engine, application)
            .is_some_and(|node| node.configured() && node.node_data().effective_enabled)
    };
    let mut columns = document
        .snapshot
        .columns
        .iter()
        .map(|column| DemandColumn {
            id: column.id,
            bindings: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut count = super::cell_applications::append(engine, document, control, &mut columns)?;
    let mut spans = Vec::new();
    let mut group_apps = HashMap::new();
    for group in &document.snapshot.column_groups {
        let Some(AppNode::MatrixColumnGroup(owner)) = engine.nodes.get(document.column_groups[&group.id]) else {
            unreachable!()
        };
        owner.bind_runtime(control.binding());
        let node = crate::applications::control_node(engine, group.application)
            .ok_or("Column Group app control is missing")?;
        super::cell_applications::bind(node, control);
        if !configured(group.application) {
            continue;
        }
        count += 1;
        if count > DemandLimits::default().applications {
            return Err("Matrix app-control binding capacity exceeded".into());
        }
        let (pre, post) = node.distances();
        spans.push(DemandSpan {
            start: group.column_start as usize,
            end: (group.column_start + group.column_count) as usize,
            binding: DemandBinding {
                application: group.application,
                pre,
                post,
            },
        });
        group_apps.insert(group.id, group.application);
    }
    let axis = Arc::new(ApplicationAxis {
        columns,
        spans,
        group_apps,
        column_apps: document
            .snapshot
            .columns
            .iter()
            .filter(|column| configured(column.application))
            .map(|column| (column.id, column.application))
            .collect(),
        cell_apps: document
            .snapshot
            .cells
            .iter()
            .filter(|cell| configured(cell.definition.application))
            .map(|cell| ((cell.layer, cell.definition.column), cell.definition.application))
            .collect(),
        column_groups: document
            .snapshot
            .columns
            .iter()
            .filter_map(|column| column.parent.map(|group| (column.id, group)))
            .collect(),
        group_parents: document
            .snapshot
            .column_groups
            .iter()
            .filter_map(|group| group.parent.map(|parent| (group.id, parent)))
            .collect(),
    });
    control.cache_application_axis(axis.clone());
    Ok(axis)
}
