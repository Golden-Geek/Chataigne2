use super::{CellNode, authoring::MAX_BATCH, document::Document, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::{ReferenceNode, protocol::*},
};
use golden_core::{
    app::ProjectNode,
    edit::{Edit, NodeTree, UserItemTreeInsertion},
    engine::ProjectImportSpec,
    node::{Node, NodeReference},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::collections::{HashMap, HashSet};

pub(super) fn configure(
    engine: &mut AppEngine,
    document: &Document,
    layer: LayerId,
    column: ColumnId,
    settings: CellSettings,
) -> Result<(), String> {
    settings.validate()?;
    let parent = document.item(MatrixItemId::Layer { id: layer })?;
    document.column(column)?;
    if let Some(id) = document.cells.get(&(layer, column)) {
        let Some(AppNode::MatrixCell(node)) = engine.nodes.get(*id) else {
            unreachable!()
        };
        engine.edits.push(node.configuration_edit(settings));
    } else {
        document.admit(1)?;
        let node = CellNode::from_address("Cell".into(), column, None).with_settings(settings);
        engine.edits.push(Edit::AddUserItemTree {
            parent,
            prev_sibling: engine.nodes.get(parent).unwrap().node_data().last_child,
            tree: NodeTree::new(node),
        });
    }
    Ok(())
}

fn binding(engine: &AppEngine, document: &Document) -> super::playback::RuntimeBinding {
    let AppNode::MediaChannel(node) = engine.nodes.get(document.root).unwrap() else {
        unreachable!()
    };
    node.matrix_playback().unwrap().binding()
}

pub(super) fn assign(
    engine: &mut AppEngine,
    document: &Document,
    assignments: Vec<CellAssignment>,
) -> Result<(), String> {
    assign_sources(
        engine,
        document,
        assignments
            .into_iter()
            .map(|assignment| {
                (
                    assignment.layer,
                    assignment.column,
                    assignment.source.map(|source| crate::media::MediaSource {
                        media: source.media,
                        channel: Some(source.channel),
                    }),
                )
            })
            .collect(),
    )
}
fn assign_sources(
    engine: &mut AppEngine,
    document: &Document,
    assignments: Vec<(LayerId, ColumnId, Option<crate::media::MediaSource>)>,
) -> Result<(), String> {
    if assignments.is_empty() || assignments.len() > MAX_BATCH {
        return Err("assign 1..4096 Matrix cells per atomic batch".into());
    }
    let mut seen = HashSet::new();
    let mut additions = 0;
    let mut changes = HashMap::new();
    for (layer, column, source) in assignments {
        document.item(MatrixItemId::Layer { id: layer })?;
        document.column(column)?;
        if !seen.insert((layer, column)) {
            return Err("duplicate Matrix cell address in one batch".into());
        }
        if let Some(source) = source {
            crate::media::authoring::media(engine, source.media)?;
            if let Some(output) = source.output(engine)? {
                crate::media::channel(engine, output)?;
            }
        }
        if let Some(id) = document.cells.get(&(layer, column)) {
            let AppNode::MatrixCell(node) = engine.nodes.get(*id).unwrap() else {
                unreachable!()
            };
            let current = node.selection(engine)?;
            let unchanged = current == source || node.content(engine)?.is_some_and(|id| {
                matches!(engine.nodes.get(id), Some(AppNode::ReferenceMedia(reference)) if source.is_some_and(|source|
                    reference.target() == Some(source.media) && current.is_some_and(|current| current.channel == source.channel)))
            });
            if unchanged {
                continue;
            }
            node.bind_runtime(binding(engine, document));
        } else {
            additions += 1;
        }
        changes.insert((layer, column), source);
    }
    document.admit(additions)?;
    let affected = changes.keys().map(|(layer, _)| *layer).collect::<HashSet<_>>();
    let mut edges = HashMap::<LayerId, Vec<ChannelReference>>::new();
    for cell in &document.snapshot.cells {
        if affected.contains(&cell.layer)
            && !changes.contains_key(&(cell.layer, cell.definition.column))
            && let Some(source) = cell.definition.source
        {
            edges.entry(cell.layer).or_default().push(source);
        }
    }
    for ((layer, _), source) in &changes {
        if let Some(source) = source
            && let Some(output) = source.output(engine)?
        {
            edges.entry(*layer).or_default().push(output);
        }
    }
    crate::media::dependencies::replace_sources(
        engine,
        affected
            .into_iter()
            .map(|layer| {
                (
                    document.channel(MatrixItemId::Layer { id: layer }),
                    edges.remove(&layer).unwrap_or_default(),
                )
            })
            .collect(),
    )?;
    let mut insertions = Vec::with_capacity(additions);
    for ((layer, column), source) in changes {
        let content = source.map(|source| ReferenceNode::from_target("Reference".into(), Some(source.media)));
        if let Some(id) = document.cells.get(&(layer, column)) {
            let AppNode::MatrixCell(current) = engine.nodes.get(*id).unwrap() else {
                unreachable!()
            };
            if let Some(old) = current.content(engine)? {
                engine.edits.push(Edit::RemoveNode { node: old });
            }
            engine.edits.push(Edit::SetParam {
                node: current.output_parameter(),
                value: ParamValue::Reference(
                    source
                        .and_then(|source| source.channel)
                        .map_or_else(NodeReference::empty, |channel| NodeReference::new(channel.0)),
                ),
                behaviour: ParameterEventBehaviour::Append,
            });
            if let Some(node) = content {
                insertions.push(UserItemTreeInsertion::new(*id, NodeTree::new(node)));
            }
        } else {
            let node = CellNode::from_address("Cell".into(), column, source.and_then(|source| source.channel));
            node.bind_runtime(binding(engine, document));
            let mut tree = NodeTree::new(node);
            if let Some(content) = content {
                tree = tree.with_child(NodeTree::new(content).as_user_item());
            }
            insertions.push(UserItemTreeInsertion::new(
                document.item(MatrixItemId::Layer { id: layer })?,
                tree,
            ));
        }
    }
    if !insertions.is_empty() {
        engine.edits.push(Edit::AddUserItemTrees { items: insertions });
    }
    Ok(())
}

pub(super) fn drop_source(
    engine: &mut AppEngine,
    document: &Document,
    layer: LayerId,
    column: ColumnId,
    source: golden_core::node::NodeUuid,
    copy: bool,
) -> Result<MediaEditResult, String> {
    let source = crate::media::source_selection(engine, source)?;
    document.item(MatrixItemId::Layer { id: layer })?;
    document.column(column)?;
    if !copy {
        assign_sources(engine, document, vec![(layer, column, Some(source))])?;
        engine.apply_edits().map_err(|e| e.to_string())?;
    } else {
        let cell = if let Some(id) = document.cells.get(&(layer, column)) {
            *id
        } else {
            document.admit(1)?;
            let node = CellNode::from_address("Cell".into(), column, None);
            node.bind_runtime(binding(engine, document));
            let uuid = node.node_data().meta.uuid;
            engine.edits.push(Edit::AddUserItemTree {
                parent: document.item(MatrixItemId::Layer { id: layer })?,
                prev_sibling: None,
                tree: NodeTree::new(node),
            });
            engine.apply_edits().map_err(|e| e.to_string())?;
            engine.node_id_by_uuid(uuid).unwrap()
        };
        let AppNode::MatrixCell(node) = engine.nodes.get(cell).unwrap() else {
            unreachable!()
        };
        node.bind_runtime(binding(engine, document));
        let old = node.content(engine)?;
        let output_parameter = node.output_parameter();
        let source_id = engine
            .node_id_by_uuid(source.media.0)
            .ok_or("source media is missing")?;
        let mut project = engine
            .capture_project_subtree_with(source_id, Node::project_encode_data)
            .map_err(|e| e.to_string())?;
        // Placement belongs to the old Composition consumer; intrinsic/nested media state stays intact.
        project
            .root
            .children
            .retain(|node| node.node_type != crate::media::composition::PLACEMENT_TYPE);
        if let Some(old) = old {
            // History restores the old output only after its media subtree has returned.
            engine.edits.push(Edit::SetParam {
                node: output_parameter,
                value: ParamValue::Reference(NodeReference::empty()),
                behaviour: ParameterEventBehaviour::Append,
            });
            engine.edits.push(Edit::RemoveNode { node: old });
            engine.apply_edits().map_err(|e| e.to_string())?;
        }
        let imported = engine
            .import_project_subtrees_with(
                vec![ProjectImportSpec {
                    project,
                    initial_params: vec![],
                }],
                cell,
                None,
                AppNode::project_decode_node,
                |_, _| Ok(()),
            )
            .map_err(|e| e.to_string())?;
        let output = source.channel.map(|channel| {
            imported
                .identities
                .iter()
                .find(|item| item.source_uuid == channel.0)
                .map_or(channel.0, |item| item.uuid)
        });
        engine.edits.push(Edit::SetParam {
            node: output_parameter,
            value: ParamValue::Reference(output.map_or_else(NodeReference::empty, NodeReference::new)),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().map_err(|e| e.to_string())?;
    }
    let refreshed = Document::load(engine, document.snapshot.media)?;
    let id = refreshed.cells[&(layer, column)];
    let AppNode::MatrixCell(cell) = engine.nodes.get(id).unwrap() else {
        unreachable!()
    };
    let content = cell.content(engine)?.ok_or("Cell media is missing")?;
    Ok(MediaEditResult::MatrixMediaCreated {
        cell: CellId(cell.node_data().meta.uuid),
        media: MediaId(engine.nodes.get(content).unwrap().node_data().meta.uuid),
    })
}
