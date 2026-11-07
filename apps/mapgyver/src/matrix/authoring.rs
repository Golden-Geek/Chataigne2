use super::{
    document::{Document, MAX_DEPTH, children},
    node::ElementNode,
    protocol::*,
};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::{
        ChannelNode,
        protocol::{
            ChannelDefinition, ChannelId, ChannelReference, ChannelSource, MediaEditResult, MediaId, MediaKind,
        },
    },
};
use golden_core::{
    edit::{Edit, NodeTree, UserItemTreeInsertion},
    node::{Node, NodeId},
};
use std::collections::HashSet;
pub(super) const MAX_BATCH: usize = 4096;

pub(super) fn export(media: MediaId, item: MatrixItemId, label: String) -> (NodeTree, ChannelReference) {
    let mut definition = ChannelDefinition::for_media(MediaKind::Matrix);
    definition.source = ChannelSource::Matrix {
        output: match item {
            MatrixItemId::Layer { id } => MatrixOutput::Layer { layer: id },
            MatrixItemId::Group { id } => MatrixOutput::Group { group: id },
        },
    };
    let node = ChannelNode::from_definition(label, definition);
    let reference = ChannelReference {
        media,
        channel: ChannelId(node.node_data().meta.uuid),
    };
    (NodeTree::new(node), reference)
}
pub(super) fn item_id(node: &ElementNode) -> MatrixItemId {
    match node.definition() {
        ElementDefinition::Layer { .. } => MatrixItemId::Layer {
            id: LayerId(node.node_data().meta.uuid),
        },
        ElementDefinition::Group { .. } => MatrixItemId::Group {
            id: GroupId(node.node_data().meta.uuid),
        },
    }
}
fn parent_depth(engine: &AppEngine, document: &Document, parent: NodeId) -> usize {
    let mut next = parent;
    let mut depth = 0;
    while next != document.root {
        depth += 1;
        next = engine.nodes.get(next).unwrap().node_data().parent.unwrap();
    }
    depth
}
pub(super) fn descendants(engine: &AppEngine, root: NodeId) -> Result<HashSet<NodeId>, String> {
    let mut pending = vec![root];
    let mut result = HashSet::new();
    while let Some(id) = pending.pop() {
        result.insert(id);
        if !matches!(engine.nodes.get(id), Some(AppNode::MatrixCell(_))) {
            pending.extend(children(engine, id)?);
        }
    }
    Ok(result)
}
fn admit_count(document: &Document, count: u16) -> Result<usize, String> {
    let count = usize::from(count);
    if count == 0 || count > MAX_BATCH {
        return Err("create 1..4096 Matrix elements per batch".into());
    }
    document.admit(count)?;
    Ok(count)
}

pub(crate) fn apply(engine: &mut AppEngine, media: MediaId, edit: MatrixEdit) -> Result<MediaEditResult, String> {
    if let MatrixEdit::EditCellGeometry { cell, edit } = edit {
        return super::geometry::cell(engine, media, cell, edit);
    }
    if let MatrixEdit::EditLayerGeometry { layer, edit } = edit {
        return super::geometry::layer(engine, media, layer, edit);
    }
    if let MatrixEdit::InspectTargets { query } = edit {
        return super::view::targets(engine, media, query);
    }
    if let MatrixEdit::InspectViewport { viewport, selection } = edit {
        return super::view::inspect(engine, media, viewport, selection);
    }
    if let MatrixEdit::LaunchCell { layer, column, mode } = edit {
        return super::controls::launch_cell(engine, media, layer, column, mode);
    }
    if let MatrixEdit::LaunchColumn { column, mode } = edit {
        return super::controls::launch_column(engine, media, column, mode);
    }
    if matches!(edit, MatrixEdit::InspectPlayback) {
        return super::playback::inspect(engine, media);
    }
    if matches!(edit, MatrixEdit::ClearPlayback) {
        return super::controls::clear(engine, media);
    }
    let document = Document::load(engine, media)?;
    let edit = super::creation::resolve(engine, &document, edit)?;
    let Some(edit) = super::placement::resolve(&document, edit)? else {
        return Ok(MediaEditResult::Changed);
    };
    match edit {
        MatrixEdit::TransferCell {
            cell,
            layer,
            column,
            action,
        } => return super::cell_drops::transfer(engine, &document, cell, layer, column, action),
        MatrixEdit::EditCellGeometry { .. } => unreachable!("direct Cell geometry is resolved before document loading"),
        MatrixEdit::EditLayerGeometry { .. } => unreachable!("direct geometry is resolved before document loading"),
        MatrixEdit::CreateAxisItem { .. } => unreachable!("contextual creation was resolved"),
        MatrixEdit::CreateCellMedia {
            layer,
            column,
            media_kind,
        } => return super::cells::create_media(engine, &document, layer, column, media_kind),
        MatrixEdit::ConfigureCell {
            layer,
            column,
            settings,
        } => super::cells::configure(engine, &document, layer, column, settings)?,
        MatrixEdit::ConfigureCellApplication {
            layer,
            column,
            definition,
            pre,
            post,
        } => {
            return super::cell_applications::configure(engine, &document, layer, column, definition, pre, post);
        }
        MatrixEdit::EnsureCell { layer, column } => {
            return super::cell_applications::prepare(engine, &document, layer, column);
        }
        MatrixEdit::DropSource {
            layer,
            column,
            source,
            copy,
        } => return super::cells::drop_source(engine, &document, layer, column, source, copy),
        MatrixEdit::InspectTargets { .. } => unreachable!(),
        MatrixEdit::DropColumnItem { .. } => unreachable!(),
        MatrixEdit::InspectViewport { .. } => unreachable!(),
        MatrixEdit::LaunchColumn { .. } => unreachable!(),
        MatrixEdit::InspectPlayback => unreachable!(),
        MatrixEdit::ClearPlayback => unreachable!(),
        MatrixEdit::Inspect => {
            return Ok(MediaEditResult::Matrix {
                snapshot: document.snapshot,
            });
        }
        MatrixEdit::Configure { settings } => {
            let Some(AppNode::Media(node)) = engine.nodes.get(document.media) else {
                unreachable!()
            };
            for edit in node.matrix_setting_edits(settings) {
                engine.edits.push(edit);
            }
        }
        MatrixEdit::AddLayers { parent, count } => {
            let count = admit_count(&document, count)?;
            let parent = document.parent(parent)?;
            if parent_depth(engine, &document, parent) >= MAX_DEPTH {
                return Err("Matrix groups exceed depth 32".into());
            }
            let mut labels = children(engine, parent)?
                .into_iter()
                .map(|id| engine.nodes.get(id).unwrap().node_data().meta.label.clone())
                .collect::<HashSet<_>>();
            let mut index = 1u64;
            let mut items = Vec::with_capacity(count * 2);
            let mut created = Vec::with_capacity(count);
            for _ in 0..count {
                let label = loop {
                    let candidate = format!("Layer {index}");
                    index += 1;
                    if labels.insert(candidate.clone()) {
                        break candidate;
                    }
                };
                let node = ElementNode::from_definition(
                    label.clone(),
                    ElementDefinition::Layer {
                        settings: CompositeSettings::default(),
                    },
                )
                .with_layer_settings(LayerSettings {
                    size: document.snapshot.definition.canvas,
                    ..Default::default()
                });
                let item = item_id(&node);
                created.push(item);
                let (channel, _) = export(media, item, format!("{label} output"));
                items.push(UserItemTreeInsertion::new(parent, NodeTree::new(node)));
                items.push(UserItemTreeInsertion::new(document.media, channel));
            }
            engine.edits.push(Edit::AddUserItemTrees { items });
            return Ok(MediaEditResult::MatrixCreated {
                items: created,
                columns: vec![],
            });
        }
        MatrixEdit::AddGroup { parent, label } => {
            document.admit(1)?;
            let parent = document.parent(parent)?;
            if parent_depth(engine, &document, parent) >= MAX_DEPTH {
                return Err("Matrix groups exceed depth 32".into());
            }
            let label = labels::unique(engine, parent, label, "Group", &[])?;
            let node = ElementNode::from_definition(
                label.clone(),
                ElementDefinition::Group {
                    settings: CompositeSettings::default(),
                },
            );
            let item = item_id(&node);
            let (channel, _) = export(media, item, format!("{label} output"));
            engine.edits.push(Edit::AddUserItemTrees {
                items: vec![
                    UserItemTreeInsertion::new(parent, NodeTree::new(node)),
                    UserItemTreeInsertion::new(document.media, channel),
                ],
            });
            return Ok(MediaEditResult::MatrixCreated {
                items: vec![item],
                columns: vec![],
            });
        }
        MatrixEdit::AddColumns { parent, count } => {
            let count = admit_count(&document, count)?;
            let parent = document.column_parent(parent)?;
            let mut names = children(engine, parent)?
                .into_iter()
                .map(|id| engine.nodes.get(id).unwrap().node_data().meta.label.clone())
                .collect::<HashSet<_>>();
            let mut index = 1u64;
            let mut items = Vec::with_capacity(count);
            let mut columns = Vec::with_capacity(count);
            for _ in 0..count {
                let label = loop {
                    let candidate = format!("Column {index}");
                    index += 1;
                    if names.insert(candidate.clone()) {
                        break candidate;
                    }
                };
                let node = super::ColumnNode::from_definition(label, ColumnDefinition::default());
                columns.push(ColumnId(node.node_data().meta.uuid));
                items.push(UserItemTreeInsertion::new(parent, NodeTree::new(node)));
            }
            engine.edits.push(Edit::AddUserItemTrees { items });
            return Ok(MediaEditResult::MatrixCreated { items: vec![], columns });
        }
        edit @ (MatrixEdit::AddColumnGroup { .. }
        | MatrixEdit::RenameColumnGroup { .. }
        | MatrixEdit::MoveColumnItem { .. }
        | MatrixEdit::UngroupColumns { .. }
        | MatrixEdit::RemoveColumnGroup { .. }
        | MatrixEdit::DuplicateColumnGroup { .. }) => {
            return super::column_groups::apply(engine, &document, edit);
        }
        MatrixEdit::ConfigureItem { item, settings } => {
            settings.validate()?;
            let id = document.item(item)?;
            let Some(AppNode::MatrixElement(node)) = engine.nodes.get(id) else {
                unreachable!()
            };
            engine.edits.push(node.configuration_edit(settings));
        }
        MatrixEdit::ConfigureLayer { layer, settings } => {
            settings.validate()?;
            let Some(AppNode::MatrixElement(node)) =
                engine.nodes.get(document.item(MatrixItemId::Layer { id: layer })?)
            else {
                unreachable!()
            };
            engine.edits.push(node.spatial_edit(settings));
        }
        MatrixEdit::RenameItem { item, label } => labels::rename(engine, document.item(item)?, label)?,
        MatrixEdit::MoveItem { item, parent, after } => {
            let node = document.item(item)?;
            let parent_id = document.parent(parent)?;
            let subtree = descendants(engine, node)?;
            if subtree.contains(&parent_id) {
                return Err("a group cannot move into itself or a descendant".into());
            }
            let previous = after.map(|id| document.item(id)).transpose()?;
            if previous == Some(node)
                || previous.is_some_and(|id| engine.nodes.get(id).unwrap().node_data().parent != Some(parent_id))
            {
                return Err("Matrix move has invalid preceding sibling".into());
            }
            let base = parent_depth(engine, &document, parent_id);
            for id in &subtree {
                if let Some(AppNode::MatrixElement(child)) = engine.nodes.get(*id)
                    && matches!(
                        child.definition(),
                        ElementDefinition::Layer { .. } | ElementDefinition::Group { .. }
                    )
                {
                    let mut relative = 0;
                    let mut next = *id;
                    while next != node {
                        relative += 1;
                        next = engine.nodes.get(next).unwrap().node_data().parent.unwrap();
                    }
                    if base + relative >= MAX_DEPTH {
                        return Err("Matrix groups exceed depth 32".into());
                    }
                }
            }
            let mut candidate = document.snapshot.clone();
            candidate
                .items
                .iter_mut()
                .find(|entry| entry.id == item)
                .unwrap()
                .parent = parent;
            crate::media::dependencies::replace_sources(engine, super::document::dependencies(&candidate))?;
            engine.edits.push(Edit::MoveNode {
                node,
                new_parent: parent_id,
                new_prev_sibling: previous,
            });
        }
        MatrixEdit::RemoveItem { item } => {
            let node = document.item(item)?;
            let subtree = descendants(engine, node)?;
            for (item, id) in &document.items {
                if subtree.contains(id) {
                    engine.edits.push(Edit::RemoveNode {
                        node: engine.node_id_by_uuid(document.channel(*item).channel.0).unwrap(),
                    });
                }
            }
            engine.edits.push(Edit::RemoveNode { node });
        }
        MatrixEdit::DuplicateItem { item } => return super::copying::item(engine, &document, item),
        MatrixEdit::RenameColumn { column, label } => labels::rename(engine, document.column(column)?, label)?,
        MatrixEdit::ConfigureColumn { column, definition } => {
            let Some(AppNode::MatrixColumn(node)) = engine.nodes.get(document.column(column)?) else {
                unreachable!()
            };
            for edit in node.configuration_edits(definition) {
                engine.edits.push(edit);
            }
        }
        MatrixEdit::ConfigureColumnApplication {
            column,
            definition,
            pre,
            post,
        } => return super::cell_applications::configure_column(engine, &document, column, definition, pre, post),
        MatrixEdit::ReorderColumn { column, after } => {
            let node = document.column(column)?;
            let parent = engine.nodes.get(node).unwrap().node_data().parent.unwrap();
            let previous = after.map(|id| document.column(id)).transpose()?;
            if previous == Some(node)
                || previous.is_some_and(|id| engine.nodes.get(id).unwrap().node_data().parent != Some(parent))
            {
                return Err("Column reorder requires distinct siblings; use a Column item move to reparent".into());
            }
            engine.edits.push(Edit::MoveNode {
                node,
                new_parent: parent,
                new_prev_sibling: previous,
            });
        }
        MatrixEdit::RemoveColumn { column } => {
            let node = document.column(column)?;
            for ((_, owner), cell) in &document.cells {
                if *owner == column {
                    engine.edits.push(Edit::RemoveNode { node: *cell });
                }
            }
            engine.edits.push(Edit::RemoveNode { node });
        }
        MatrixEdit::DuplicateColumn { column } => return super::copying::column(engine, &document, column),
        MatrixEdit::LaunchCell { .. } | MatrixEdit::DropItem { .. } | MatrixEdit::DropColumn { .. } => unreachable!(),
        MatrixEdit::SetCells { assignments } => super::cells::assign(engine, &document, assignments)?,
    }
    Ok(MediaEditResult::Changed)
}
