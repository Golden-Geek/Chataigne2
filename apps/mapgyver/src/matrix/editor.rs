//! Per-panel content resolution; the existing viewport index owns bounded document projections.
use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{MediaEditResult, MediaId, MediaKind},
};
use golden_core::node::{Node, NodeUuid};

#[derive(Clone, Copy)]
enum Focus {
    Item(MatrixItemId, NodeUuid),
    Column(ColumnId, NodeUuid),
    Group(NodeUuid),
    Cell(LayerId, ColumnId, NodeUuid),
}
struct Owner {
    media: MediaId,
    focus: Option<Focus>,
}
fn owner(engine: &AppEngine, selected: Option<NodeUuid>) -> Option<Owner> {
    let mut next = selected.and_then(|uuid| engine.node_id_by_uuid(uuid));
    let mut focus = None;
    for _ in 0..64 {
        let node = engine.nodes.get(next?)?;
        let uuid = node.node_data().meta.uuid;
        if let AppNode::Media(node) = node
            && node.kind() == MediaKind::Matrix
        {
            return Some(Owner {
                media: MediaId(uuid),
                focus,
            });
        }
        if focus.is_none() {
            focus = match node {
                AppNode::MatrixElement(node) => Some(Focus::Item(
                    match node.definition() {
                        ElementDefinition::Layer { .. } => MatrixItemId::Layer { id: LayerId(uuid) },
                        ElementDefinition::Group { .. } => MatrixItemId::Group { id: GroupId(uuid) },
                    },
                    uuid,
                )),
                AppNode::MatrixColumn(_) => Some(Focus::Column(ColumnId(uuid), uuid)),
                AppNode::MatrixColumnGroup(_) => Some(Focus::Group(uuid)),
                AppNode::MatrixCell(cell) => {
                    let parent = engine.nodes.get(node.node_data().parent?)?;
                    Some(Focus::Cell(LayerId(parent.node_data().meta.uuid), cell.column(), uuid))
                }
                _ => None,
            };
        }
        next = node.node_data().parent;
    }
    None
}

pub(crate) fn inspect(engine: &AppEngine, query: MatrixEditorQuery) -> Result<MatrixEditorPage, String> {
    if (query.locked.is_some() || query.retained.is_some()) && query.document.is_none() {
        return Err("Matrix bindings require a document identity".into());
    }
    let document = engine
        .nodes
        .get(engine.root)
        .ok_or("project root is missing")?
        .node_data()
        .meta
        .uuid;
    let followed = owner(engine, query.selected);
    let matches = query.document.is_none_or(|expected| expected == document);
    let mut page = MatrixEditorPage {
        at: engine.time,
        parameters: Vec::new(),
        document,
        document_matches: matches || (query.locked.is_none() && followed.is_some()),
        media: None,
        node: None,
        selection: None,
        snapshot: None,
    };
    if !page.document_matches {
        return Ok(page);
    }
    let Some(media) = query
        .locked
        .or_else(|| followed.as_ref().map(|owner| owner.media))
        .or(query.retained.filter(|_| matches))
    else {
        return Ok(page);
    };
    page.media = Some(media);
    let same = matches && query.retained == Some(media);
    let mut selection = if same { query.selection } else { None };
    let mut selected_node = if same { query.node } else { None };
    if let Some(focus) = followed
        .filter(|owner| owner.media == media)
        .and_then(|owner| owner.focus)
    {
        let mut current = selection.unwrap_or(MatrixSelection {
            item: None,
            column: None,
        });
        selected_node = Some(match focus {
            Focus::Item(item, node) => {
                current.item = Some(item);
                node
            }
            Focus::Column(column, node) => {
                current.column = Some(column);
                node
            }
            Focus::Group(node) => {
                current = MatrixSelection {
                    item: None,
                    column: None,
                };
                node
            }
            Focus::Cell(layer, column, node) => {
                current = MatrixSelection {
                    item: Some(MatrixItemId::Layer { id: layer }),
                    column: Some(column),
                };
                node
            }
        });
        selection = Some(current);
    }
    page.selection = selection;
    page.node = selected_node;
    if !engine
        .node_id_by_uuid(media.0)
        .and_then(|id| engine.nodes.get(id))
        .is_some_and(|node| matches!(node, AppNode::Media(node) if node.kind() == MediaKind::Matrix))
    {
        return Ok(page);
    }
    let mut viewport = query.viewport;
    if !same {
        viewport.row_start = 0;
        viewport.axis_start = 0;
    }
    let MediaEditResult::MatrixViewport { snapshot } = super::view::inspect(engine, media, viewport, selection)? else {
        return Err("unexpected Matrix editor projection".into());
    };
    for row in snapshot
        .rows
        .iter()
        .chain(snapshot.selected.iter().filter_map(|selected| selected.row.as_ref()))
    {
        let MatrixItemId::Layer { id } = row.item.id else {
            continue;
        };
        if page.parameters.iter().any(|binding| binding.item == id.0) {
            continue;
        }
        let node = engine.node_id_by_uuid(id.0).ok_or("Layer changed")?;
        let Some(AppNode::MatrixElement(node)) = engine.nodes.get(node) else {
            return Err("Layer changed".into());
        };
        page.parameters.push(node.geometry_bindings(engine, id.0));
    }
    page.snapshot = Some(snapshot);
    Ok(page)
}
