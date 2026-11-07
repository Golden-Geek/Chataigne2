//! Atomic sparse Cell relocation. Settings, owned media and process identities travel with the subtree.
use super::{document::Document, protocol::*};
use crate::app::{AppEngine, AppNode};
use golden_core::{
    app::ProjectNode,
    edit::{Edit, NodeTree},
    engine::ProjectImportSpec,
    node::{DeclId, Node, NodeId, NodeReference},
    parameter::{ParamValue, ParameterEventBehaviour},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum DuplicateMode {
    Copy,
    Reference,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum OccupiedPolicy {
    Reject,
    Swap,
}

pub(super) fn transfer(
    engine: &mut AppEngine,
    document: &Document,
    cell: CellId,
    layer: LayerId,
    column: ColumnId,
    action: CellTransferAction,
) -> Result<crate::media::protocol::MediaEditResult, String> {
    match action {
        CellTransferAction::Copy => duplicate(engine, document, cell, layer, column, DuplicateMode::Copy),
        CellTransferAction::Reference => duplicate(engine, document, cell, layer, column, DuplicateMode::Reference),
        CellTransferAction::Move | CellTransferAction::Drop => {
            relocate(
                engine,
                document,
                cell,
                layer,
                column,
                if action == CellTransferAction::Drop {
                    OccupiedPolicy::Swap
                } else {
                    OccupiedPolicy::Reject
                },
            )?;
            Ok(crate::media::protocol::MediaEditResult::Changed)
        }
    }
}

fn duplicate(
    engine: &mut AppEngine,
    document: &Document,
    cell: CellId,
    layer: LayerId,
    column: ColumnId,
    mode: DuplicateMode,
) -> Result<crate::media::protocol::MediaEditResult, String> {
    let parent = document.item(MatrixItemId::Layer { id: layer })?;
    document.column(column)?;
    if document.cells.contains_key(&(layer, column)) {
        return Err("Destination Cell is occupied; Copy and Reference require an empty address".into());
    }
    let id = engine.node_id_by_uuid(cell.0).ok_or("Source Cell is missing")?;
    let Some(AppNode::MatrixCell(source)) = engine.nodes.get(id) else {
        return Err("Source must be a Cell".into());
    };
    let source_media = super::cell_application_links::matrix(engine, id)?;
    let source_document = Document::load(engine, source_media)?;
    if mode == DuplicateMode::Reference {
        super::cell_application_links::validate_new_reference(engine, cell)?;
    }
    let original = source_document
        .snapshot
        .cells
        .iter()
        .find(|entry| entry.id == cell)
        .ok_or("Source Cell is outside its Matrix")?;
    let mut ancestor = Some(parent);
    while let Some(current) = ancestor {
        if current == id {
            return Err("A Cell cannot be copied or referenced inside its own producer".into());
        }
        ancestor = engine.nodes.get(current).and_then(|node| node.node_data().parent);
    }
    document.admit(1)?;
    let target_channel = document.channel(MatrixItemId::Layer { id: layer });
    let mut replacements = super::document::dependencies(&document.snapshot);
    if let Some(output) = original.definition.source {
        replacements.entry(target_channel).or_default().push(output);
    }
    crate::media::dependencies::replace_sources(engine, replacements)?;
    let selection = source.selection(engine)?;
    let content = source
        .content(engine)?
        .map(|id| engine.nodes.get(id).unwrap().node_data().meta.uuid);
    let mut project = engine
        .capture_project_subtree_with(id, Node::project_encode_data)
        .map_err(|error| error.to_string())?;
    let mut initial_params = vec![(
        DeclId("parameters/column".into()),
        ParamValue::Reference(NodeReference::new(column.0)),
    )];
    if mode == DuplicateMode::Reference {
        project.root.children.retain(|child| Some(child.uuid) != content);
        initial_params.push((
            DeclId("parameters/output".into()),
            ParamValue::Reference(NodeReference::empty()),
        ));
        initial_params.push((
            DeclId("parameters/shared_app_control".into()),
            ParamValue::Reference(NodeReference::new(cell.0)),
        ));
    }
    let imported = engine
        .import_project_subtrees_with(
            vec![ProjectImportSpec {
                project,
                initial_params,
            }],
            parent,
            None,
            AppNode::project_decode_node,
            |_, _| Ok(()),
        )
        .map_err(|error| error.to_string())?;
    let root = imported.roots[0];
    let Some(AppNode::MatrixCell(created)) = engine.nodes.get(root) else {
        unreachable!()
    };
    let created_id = CellId(created.node_data().meta.uuid);
    if mode == DuplicateMode::Reference {
        let settings = created
            .application_node()
            .ok_or("Cell app-control settings are missing")?;
        crate::applications::configure_node(
            engine,
            settings,
            crate::applications::protocol::ApplicationDefinition::default(),
        )?;
        if let Some(source) = selection {
            engine.edits.push(Edit::AddUserItemTree {
                parent: root,
                prev_sibling: None,
                tree: NodeTree::new(crate::media::ReferenceNode::from_source(
                    "Reference".into(),
                    Some(source),
                )),
            });
        }
    }
    Ok(crate::media::protocol::MediaEditResult::MatrixCellPrepared {
        cell: created_id,
        application: crate::applications::protocol::ApplicationId(created_id.0),
    })
}

fn relocate(
    engine: &mut AppEngine,
    document: &Document,
    cell: CellId,
    layer: LayerId,
    column: ColumnId,
    occupied: OccupiedPolicy,
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
    if target.is_some() && occupied == OccupiedPolicy::Reject {
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
