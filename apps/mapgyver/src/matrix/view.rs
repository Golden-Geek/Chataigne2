//! Cached address/order indices and bounded viewport DTOs; labels and payloads read live.
use super::{document::Document, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{MediaEditResult, MediaId},
};
use golden_core::node::{Node, NodeId};
use std::{collections::HashMap, sync::Arc};

pub(crate) struct ViewIndex {
    rows: Vec<ViewRow>,
    row_positions: HashMap<MatrixItemId, usize>,
    columns: Vec<(ColumnId, NodeId)>,
    column_nodes: HashMap<ColumnId, NodeId>,
    cells: HashMap<(LayerId, ColumnId), NodeId>,
}
struct ViewRow {
    id: MatrixItemId,
    parent: Option<GroupId>,
    node: NodeId,
    depth: u8,
    channel: crate::media::protocol::ChannelReference,
}
impl ViewIndex {
    fn build(document: Document) -> Self {
        let mut depths = HashMap::new();
        let rows: Vec<_> = document
            .snapshot
            .items
            .iter()
            .map(|item| {
                let depth = item
                    .parent
                    .map_or(0, |parent| depths[&MatrixItemId::Group { id: parent }] + 1);
                depths.insert(item.id, depth);
                ViewRow {
                    id: item.id,
                    parent: item.parent,
                    node: document.items[&item.id],
                    depth,
                    channel: item.channel,
                }
            })
            .collect();
        let columns = document
            .snapshot
            .columns
            .iter()
            .map(|column| (column.id, document.columns[&column.id]))
            .collect();
        Self {
            row_positions: rows
                .iter()
                .enumerate()
                .map(|(position, row)| (row.id, position))
                .collect(),
            rows,
            columns,
            column_nodes: document.columns,
            cells: document.cells,
        }
    }
}
fn index(engine: &AppEngine, media: MediaId) -> Result<Arc<ViewIndex>, String> {
    let root = super::playback::root(engine, media)?;
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(root) else {
        unreachable!()
    };
    let control = node.matrix_playback().ok_or("Matrix playback owner is missing")?;
    Ok(if let Some(index) = control.view() {
        index
    } else {
        let document = Document::load(engine, media)?;
        for id in &document.elements {
            match engine.nodes.get(*id) {
                Some(AppNode::MatrixElement(node)) => node.bind_runtime(control.binding()),
                Some(AppNode::MatrixCell(node)) => node.bind_runtime(control.binding()),
                _ => unreachable!(),
            }
        }
        let index = Arc::new(ViewIndex::build(document));
        control.cache_view(index.clone());
        index
    })
}

pub(super) fn targets(engine: &AppEngine, media: MediaId, query: MatrixTargetQuery) -> Result<MediaEditResult, String> {
    if query.count == 0 || query.count > 128 || query.search.len() > 256 {
        return Err("Matrix targets require 1..128 results and at most 256 search bytes".into());
    }
    let index = index(engine, media)?;
    if let MatrixTargetScope::Items { parent: Some(parent) } = query.scope
        && !index
            .rows
            .iter()
            .any(|row| row.id == MatrixItemId::Group { id: parent })
    {
        return Err("Matrix target parent is missing".into());
    }
    let search = query.search.to_lowercase();
    let group_paths = matches!(query.scope, MatrixTargetScope::Groups);
    let mut page = MatrixTargetPage {
        choices: vec![],
        total: 0,
        start: query.start,
    };
    let mut admit = |id, node| -> Result<(), String> {
        let mut labels = Vec::new();
        let mut next = Some(node);
        while let Some(id) = next {
            let current = engine.nodes.get(id).ok_or("Matrix target changed")?;
            if !matches!(current, AppNode::MatrixElement(_)) {
                break;
            }
            labels.push(current.node_data().meta.label.as_str());
            next = if group_paths { current.node_data().parent } else { None };
        }
        labels.reverse();
        let label = labels.join(" / ");
        if label.to_lowercase().contains(&search) {
            let position = page.total;
            page.total += 1;
            if position >= query.start && page.choices.len() < usize::from(query.count) {
                page.choices.push(MatrixTargetChoice { id, label });
            }
        }
        Ok(())
    };
    match query.scope {
        MatrixTargetScope::Columns => {
            for (column, node) in &index.columns {
                admit(MatrixTargetId::Column { column: *column }, *node)?;
            }
        }
        scope => {
            for row in &index.rows {
                let included = match scope {
                    MatrixTargetScope::Groups => matches!(row.id, MatrixItemId::Group { .. }),
                    MatrixTargetScope::Items { parent } => row.parent == parent,
                    MatrixTargetScope::Columns => unreachable!(),
                };
                if included {
                    admit(MatrixTargetId::Item { item: row.id }, row.node)?;
                }
            }
        }
    }
    Ok(MediaEditResult::MatrixTargets { page })
}

fn row_snapshot(engine: &AppEngine, row: &ViewRow) -> Result<MatrixViewRow, String> {
    let Some(AppNode::MatrixElement(node)) = engine.nodes.get(row.node) else {
        return Err("Matrix row changed".into());
    };
    let (ElementDefinition::Layer { settings } | ElementDefinition::Group { settings }) = node.definition() else {
        return Err("Matrix row type changed".into());
    };
    Ok(MatrixViewRow {
        item: MatrixItemSnapshot {
            id: row.id,
            parent: row.parent,
            label: node.node_data().meta.label.clone(),
            settings: settings.clone(),
            channel: row.channel,
        },
        depth: row.depth,
    })
}

fn column_snapshot(engine: &AppEngine, id: ColumnId, node_id: NodeId) -> Result<ColumnSnapshot, String> {
    let Some(AppNode::MatrixElement(node)) = engine.nodes.get(node_id) else {
        return Err("Matrix column changed".into());
    };
    let ElementDefinition::Column { definition } = node.definition() else {
        return Err("Matrix column type changed".into());
    };
    Ok(ColumnSnapshot {
        id,
        label: node.node_data().meta.label.clone(),
        definition: definition.clone(),
    })
}

fn cell_snapshot(engine: &AppEngine, layer: LayerId, node_id: NodeId) -> Result<MatrixViewCell, String> {
    let Some(AppNode::MatrixCell(cell)) = engine.nodes.get(node_id) else {
        return Err("Matrix cell changed".into());
    };
    let definition = cell.definition(engine)?;
    let mut source = None;
    let mut next = definition.source;
    let mut seen = std::collections::HashSet::new();
    while let Some(reference) = next {
        if !seen.insert(reference) {
            break;
        }
        let Ok(channel) = crate::media::channel(engine, reference) else {
            break;
        };
        if let crate::media::protocol::ChannelSource::Reference { target } = channel.definition.source {
            next = target;
        } else {
            source = Some(channel);
            break;
        }
    }
    Ok(MatrixViewCell {
        cell: CellSnapshot {
            id: CellId(cell.node_data().meta.uuid),
            media: cell.selection(engine)?.map(|source| source.media),
            layer,
            definition: definition.clone(),
        },
        source_label: source.as_ref().map(|source| source.label.clone()),
        source_missing: cell.content(engine)?.is_some() && source.is_none(),
    })
}

fn selected_snapshot(
    engine: &AppEngine,
    index: &ViewIndex,
    selection: MatrixSelection,
) -> Result<MatrixSelectionSnapshot, String> {
    let row = selection
        .item
        .and_then(|id| index.row_positions.get(&id))
        .map(|position| row_snapshot(engine, &index.rows[*position]))
        .transpose()?;
    let column = selection
        .column
        .and_then(|id| index.column_nodes.get(&id).map(|node| (id, *node)))
        .map(|(id, node)| column_snapshot(engine, id, node))
        .transpose()?;
    let cell = match (row.as_ref().map(|row| row.item.id), column.as_ref()) {
        (Some(MatrixItemId::Layer { id: layer }), Some(column)) => index
            .cells
            .get(&(layer, column.id))
            .map(|node| cell_snapshot(engine, layer, *node))
            .transpose()?,
        _ => None,
    };
    Ok(MatrixSelectionSnapshot {
        selection,
        row,
        column,
        cell,
    })
}

pub(super) fn inspect(
    engine: &AppEngine,
    media: MediaId,
    viewport: MatrixViewport,
    selection: Option<MatrixSelection>,
) -> Result<MediaEditResult, String> {
    if viewport.row_count == 0
        || viewport.row_count > 128
        || viewport.column_count == 0
        || viewport.column_count > 64
        || u32::from(viewport.row_count) * u32::from(viewport.column_count) > 4096
    {
        return Err("Matrix viewport admits 1..128 rows, 1..64 columns and at most 4096 addresses".into());
    }
    let index = index(engine, media)?;
    let root = super::playback::root(engine, media)?;
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(root) else {
        unreachable!()
    };
    let control = node.matrix_playback().ok_or("Matrix playback owner is missing")?;
    let row_start = (viewport.row_start as usize).min(index.rows.len());
    let column_start = (viewport.column_start as usize).min(index.columns.len());
    let mut rows = Vec::new();
    let mut columns = Vec::new();
    let mut cells = Vec::new();
    for (id, node_id) in index
        .columns
        .iter()
        .skip(column_start)
        .take(usize::from(viewport.column_count))
    {
        columns.push(column_snapshot(engine, *id, *node_id)?);
    }
    for row in index.rows.iter().skip(row_start).take(usize::from(viewport.row_count)) {
        rows.push(row_snapshot(engine, row)?);
        let MatrixItemId::Layer { id: layer } = row.id else {
            continue;
        };
        for column in &columns {
            let Some(cell_id) = index.cells.get(&(layer, column.id)) else {
                continue;
            };
            cells.push(cell_snapshot(engine, layer, *cell_id)?);
        }
    }
    Ok(MediaEditResult::MatrixViewport {
        snapshot: MatrixViewportSnapshot {
            project: crate::media::authoring::project_instance(engine)?,
            media,
            final_output: crate::media::channel(
                engine,
                crate::media::protocol::ChannelReference {
                    media,
                    channel: crate::media::protocol::ChannelId(node.node_data().meta.uuid),
                },
            )?,
            revision: control.authoring_revision(),
            row_total: index.rows.len() as u32,
            column_total: index.columns.len() as u32,
            row_start: row_start as u32,
            column_start: column_start as u32,
            rows,
            columns,
            cells,
            selected: selection
                .map(|selection| selected_snapshot(engine, &index, selection))
                .transpose()?
                .map(Box::new),
            playback: control.snapshot(),
        },
    })
}
