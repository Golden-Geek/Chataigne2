//! Contextual creation policy is shared by UI, scripts and transport callers.
use super::{document::Document, protocol::*};
use crate::app::{AppEngine, AppNode};
use golden_core::{model::NodeUuid, node::Node};

pub(super) fn resolve(engine: &AppEngine, document: &Document, edit: MatrixEdit) -> Result<MatrixEdit, String> {
    let MatrixEdit::CreateAxisItem { item, context } = edit else {
        return Ok(edit);
    };
    let (column, row) = parents(engine, document, context)?;
    Ok(match item {
        MatrixAxisItemKind::Column => MatrixEdit::AddColumns {
            parent: column,
            count: 1,
        },
        MatrixAxisItemKind::ColumnGroup => MatrixEdit::AddColumnGroup {
            parent: column,
            label: None,
        },
        MatrixAxisItemKind::Layer => MatrixEdit::AddLayers { parent: row, count: 1 },
        MatrixAxisItemKind::LayerGroup => MatrixEdit::AddGroup {
            parent: row,
            label: None,
        },
    })
}

fn parents(
    engine: &AppEngine,
    document: &Document,
    context: Option<NodeUuid>,
) -> Result<(Option<ColumnGroupId>, Option<GroupId>), String> {
    let Some(context) = context else {
        return Ok((None, None));
    };
    let mut current = engine
        .node_id_by_uuid(context)
        .ok_or("Matrix creation context is missing")?;
    // Includes typed Parameters and owned child-media selection without reading UI caches.
    for _ in 0..64 {
        let node = engine.nodes.get(current).ok_or("Matrix creation context is missing")?;
        let uuid = node.node_data().meta.uuid;
        if document.column_groups.contains_key(&ColumnGroupId(uuid)) {
            return Ok((Some(ColumnGroupId(uuid)), None));
        }
        if document.columns.contains_key(&ColumnId(uuid)) {
            let parent = document
                .snapshot
                .columns
                .iter()
                .find(|column| column.id.0 == uuid)
                .unwrap()
                .parent;
            return Ok((parent, None));
        }
        if document.elements.contains(&current) {
            if let AppNode::MatrixCell(cell) = node {
                let column = cell.definition(engine)?.column;
                let column_parent = document
                    .snapshot
                    .columns
                    .iter()
                    .find(|entry| entry.id == column)
                    .unwrap()
                    .parent;
                let layer = engine
                    .nodes
                    .get(node.node_data().parent.unwrap())
                    .unwrap()
                    .node_data()
                    .meta
                    .uuid;
                let row_parent = document
                    .snapshot
                    .items
                    .iter()
                    .find(|entry| item_uuid(entry.id) == layer)
                    .unwrap()
                    .parent;
                return Ok((column_parent, row_parent));
            }
            let row = document
                .snapshot
                .items
                .iter()
                .find(|entry| item_uuid(entry.id) == uuid)
                .unwrap();
            return Ok((
                None,
                match row.id {
                    MatrixItemId::Group { id } => Some(id),
                    MatrixItemId::Layer { .. } => row.parent,
                },
            ));
        }
        if current == document.root || current == document.media {
            return Ok((None, None));
        }
        let Some(parent) = node.node_data().parent else {
            return Ok((None, None));
        };
        current = parent;
    }
    Err("Matrix creation context exceeds the supported ancestry depth".into())
}

fn item_uuid(item: MatrixItemId) -> NodeUuid {
    match item {
        MatrixItemId::Layer { id } => id.0,
        MatrixItemId::Group { id } => id.0,
    }
}
