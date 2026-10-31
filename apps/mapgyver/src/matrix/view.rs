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
    columns: Vec<(ColumnId, NodeId)>,
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
        let rows = document
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
            rows,
            columns,
            cells: document.cells,
        }
    }
}
pub(super) fn inspect(engine: &AppEngine, media: MediaId, viewport: MatrixViewport) -> Result<MediaEditResult, String> {
    if viewport.row_count == 0
        || viewport.row_count > 128
        || viewport.column_count == 0
        || viewport.column_count > 64
        || u32::from(viewport.row_count) * u32::from(viewport.column_count) > 4096
    {
        return Err("Matrix viewport admits 1..128 rows, 1..64 columns and at most 4096 addresses".into());
    }
    let root = super::playback::root(engine, media)?;
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(root) else {
        unreachable!()
    };
    let control = node.matrix_playback().ok_or("Matrix playback owner is missing")?;
    let index = if let Some(index) = control.view() {
        index
    } else {
        let document = Document::load(engine, media)?;
        for id in &document.elements {
            let Some(AppNode::MatrixElement(node)) = engine.nodes.get(*id) else {
                unreachable!()
            };
            node.bind_runtime(control.binding());
        }
        let index = Arc::new(ViewIndex::build(document));
        control.cache_view(index.clone());
        index
    };
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
        let Some(AppNode::MatrixElement(node)) = engine.nodes.get(*node_id) else {
            return Err("Matrix column changed".into());
        };
        let ElementDefinition::Column { definition } = node.definition() else {
            return Err("Matrix column type changed".into());
        };
        columns.push(ColumnSnapshot {
            id: *id,
            label: node.node_data().meta.label.clone(),
            definition: definition.clone(),
        });
    }
    for row in index.rows.iter().skip(row_start).take(usize::from(viewport.row_count)) {
        let Some(AppNode::MatrixElement(node)) = engine.nodes.get(row.node) else {
            return Err("Matrix row changed".into());
        };
        let (ElementDefinition::Layer { settings } | ElementDefinition::Group { settings }) = node.definition() else {
            return Err("Matrix row type changed".into());
        };
        rows.push(MatrixViewRow {
            item: MatrixItemSnapshot {
                id: row.id,
                parent: row.parent,
                label: node.node_data().meta.label.clone(),
                settings: settings.clone(),
                channel: row.channel,
            },
            depth: row.depth,
        });
        let MatrixItemId::Layer { id: layer } = row.id else {
            continue;
        };
        for column in &columns {
            let Some(cell_id) = index.cells.get(&(layer, column.id)) else {
                continue;
            };
            let Some(AppNode::MatrixElement(cell)) = engine.nodes.get(*cell_id) else {
                return Err("Matrix cell changed".into());
            };
            let ElementDefinition::Cell { definition } = cell.definition() else {
                return Err("Matrix cell type changed".into());
            };
            let source = definition
                .source
                .and_then(|reference| crate::media::channel(engine, reference).ok());
            cells.push(MatrixViewCell {
                cell: CellSnapshot {
                    id: CellId(cell.node_data().meta.uuid),
                    layer,
                    definition: definition.clone(),
                },
                source_label: source.as_ref().map(|source| source.label.clone()),
                source_missing: definition.source.is_some() && source.is_none(),
            });
        }
    }
    Ok(MediaEditResult::MatrixViewport {
        snapshot: MatrixViewportSnapshot {
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
            playback: control.snapshot(),
        },
    })
}
