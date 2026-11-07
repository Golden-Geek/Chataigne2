//! Atomic sparse Cell relocation. Settings, owned media and process identities travel with the subtree.
use super::{document::Document, protocol::*};
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    node::{NodeId, NodeReference},
    parameter::{ParamValue, ParameterEventBehaviour},
};

pub(super) fn relocate(
    engine: &mut AppEngine,
    document: &Document,
    cell: CellId,
    layer: LayerId,
    column: ColumnId,
    occupied: CellOccupiedDrop,
) -> Result<(), String> {
    let source = document
        .snapshot
        .cells
        .iter()
        .find(|entry| entry.id == cell)
        .ok_or("Cell does not belong to this Matrix")?;
    let target_parent = document.item(MatrixItemId::Layer { id: layer })?;
    document.column(column)?;
    if (source.layer, source.definition.column) == (layer, column) {
        return Ok(());
    }
    let target = document
        .snapshot
        .cells
        .iter()
        .find(|entry| entry.layer == layer && entry.definition.column == column);
    if target.is_some() && occupied == CellOccupiedDrop::Reject {
        return Err("Destination Cell is occupied; choose Swap or an empty address".into());
    }
    let mut candidate = document.snapshot.clone();
    for entry in &mut candidate.cells {
        if entry.id == cell {
            entry.layer = layer;
            entry.definition.column = column;
        } else if target.is_some_and(|target| entry.id == target.id) {
            entry.layer = source.layer;
            entry.definition.column = source.definition.column;
        }
    }
    // Validate the final graph before staging either half. Retained outgoing sources still participate.
    crate::media::dependencies::replace_sources(engine, super::document::dependencies(&candidate))?;
    let source_node = document.cells[&(source.layer, source.definition.column)];
    let mut values = vec![column_value(engine, source_node, column)];
    let source_parent = document.item(MatrixItemId::Layer { id: source.layer })?;
    let target_node = target.map(|target| document.cells[&(target.layer, target.definition.column)]);
    if let Some(node) = target_node {
        values.push(column_value(engine, node, source.definition.column));
    }
    engine.edits.push(Edit::SetParamsAtomic {
        values,
        behaviour: ParameterEventBehaviour::Append,
    });
    if source_parent != target_parent {
        engine.edits.push(Edit::MoveNode {
            node: source_node,
            new_parent: target_parent,
            new_prev_sibling: None,
        });
        if let Some(node) = target_node {
            engine.edits.push(Edit::MoveNode {
                node,
                new_parent: source_parent,
                new_prev_sibling: None,
            });
        }
    }
    Ok(())
}

fn column_value(engine: &AppEngine, node: NodeId, column: ColumnId) -> (NodeId, ParamValue) {
    let Some(AppNode::MatrixCell(cell)) = engine.nodes.get(node) else {
        unreachable!()
    };
    (
        cell.column_parameter(),
        ParamValue::Reference(NodeReference::new(column.0)),
    )
}
