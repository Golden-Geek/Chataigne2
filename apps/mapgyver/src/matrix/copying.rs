//! Copy complete authored forests through Golden, then reconnect paired exports/address Parameters.
use super::{
    authoring::{descendants, export, item_id},
    document::Document,
    protocol::*,
};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{ChannelReference, MediaEditResult},
};
use golden_core::{
    app::ProjectNode,
    edit::{Edit, NodeTree, UserItemTreeInsertion},
    node::{Node, NodeId, NodeReference, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiDuplicateNodeSpec,
};
use std::collections::{HashMap, HashSet};

fn raw_children(engine: &AppEngine, root: NodeId) -> Vec<NodeId> {
    let mut next = engine.nodes.get(root).unwrap().node_data().first_child;
    let mut ids = Vec::new();
    while let Some(id) = next {
        ids.push(id);
        next = engine.nodes.get(id).unwrap().node_data().next_sibling;
    }
    ids
}
fn mapping(
    engine: &AppEngine,
    roots: impl IntoIterator<Item = (NodeId, NodeId)>,
) -> Result<(HashMap<NodeUuid, NodeUuid>, Vec<NodeId>), String> {
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    let mut remap = HashMap::new();
    let mut copied = Vec::new();
    while let Some((old, new)) = pending.pop() {
        remap.insert(
            engine.nodes.get(old).unwrap().node_data().meta.uuid,
            engine.nodes.get(new).unwrap().node_data().meta.uuid,
        );
        copied.push(new);
        let a = raw_children(engine, old);
        let b = raw_children(engine, new);
        if a.len() != b.len() {
            return Err("Copy changed authored subtree shape".into());
        }
        pending.extend(a.into_iter().zip(b));
    }
    Ok((remap, copied))
}
fn patch_references(
    engine: &mut AppEngine,
    copied: &[NodeId],
    remap: &HashMap<NodeUuid, NodeUuid>,
    extra: Vec<(NodeId, ParamValue)>,
) {
    let mut values = extra;
    for id in copied {
        if let Some(snapshot) = engine.nodes.get(*id).unwrap().engine_param_snapshot()
            && let ParamValue::Reference(reference) = snapshot.value
            && let Some(uuid) = remap.get(&reference.uuid)
        {
            values.push((*id, ParamValue::Reference(NodeReference::new(*uuid))));
        }
    }
    if !values.is_empty() {
        engine.edits.push(Edit::SetParamsAtomic {
            values,
            behaviour: ParameterEventBehaviour::Append,
        });
    }
}
fn duplicate(engine: &mut AppEngine, specs: Vec<UiDuplicateNodeSpec>) -> Result<Vec<NodeId>, String> {
    engine
        .ui_apply_duplicate_nodes_with_dependent_user_items(
            specs,
            vec![],
            vec![],
            Node::project_encode_data,
            AppNode::project_decode_node,
        )
        .map_err(|e| e.to_string())
}
fn spec(source: NodeId, new_parent: NodeId) -> UiDuplicateNodeSpec {
    UiDuplicateNodeSpec {
        source,
        new_parent,
        new_prev_sibling: None,
        initial_params: vec![],
    }
}
pub(super) fn column(engine: &mut AppEngine, document: &Document, column: ColumnId) -> Result<MediaEditResult, String> {
    let root = document.column(column)?;
    let cells = document
        .snapshot
        .cells
        .iter()
        .filter(|cell| cell.definition.column == column)
        .map(|cell| document.cells[&(cell.layer, column)])
        .collect::<Vec<_>>();
    document.admit(cells.len() + 1)?;
    let parent = engine.nodes.get(root).unwrap().node_data().parent.unwrap();
    let mut specs = vec![spec(root, parent)];
    for id in &cells {
        specs.push(spec(*id, engine.nodes.get(*id).unwrap().node_data().parent.unwrap()));
    }
    let roots = duplicate(engine, specs)?;
    let created = ColumnId(engine.nodes.get(roots[0]).unwrap().node_data().meta.uuid);
    let (remap, copied) = mapping(
        engine,
        std::iter::once((root, roots[0])).chain(cells.into_iter().zip(roots.iter().copied().skip(1))),
    )?;
    patch_references(engine, &copied, &remap, vec![]);
    Ok(MediaEditResult::MatrixCreated {
        items: vec![],
        columns: vec![created],
    })
}
pub(super) fn item(engine: &mut AppEngine, document: &Document, item: MatrixItemId) -> Result<MediaEditResult, String> {
    let root = document.item(item)?;
    let all = descendants(engine, root)?;
    document.admit(all.len())?;
    let parent = engine.nodes.get(root).unwrap().node_data().parent.unwrap();
    let created = duplicate(engine, vec![spec(root, parent)])?[0];
    let (mut remap, copied) = mapping(engine, [(root, created)])?;
    let mut paired = Vec::<UserItemTreeInsertion>::new();
    for (old, id) in &document.items {
        let old_uuid = engine.nodes.get(*id).unwrap().node_data().meta.uuid;
        if all.contains(id) {
            let uuid = remap[&old_uuid];
            let new_id = match old {
                MatrixItemId::Layer { .. } => MatrixItemId::Layer { id: LayerId(uuid) },
                MatrixItemId::Group { .. } => MatrixItemId::Group { id: GroupId(uuid) },
            };
            let label = engine
                .nodes
                .get(engine.node_id_by_uuid(uuid).unwrap())
                .unwrap()
                .node_data()
                .meta
                .label
                .clone();
            let (tree, reference): (NodeTree, ChannelReference) =
                export(document.snapshot.media, new_id, format!("{label} output"));
            remap.insert(document.channel(*old).channel.0, reference.channel.0);
            paired.push(UserItemTreeInsertion::new(document.media, tree));
        }
    }
    let Some(AppNode::MatrixElement(node)) = engine.nodes.get(created) else {
        unreachable!()
    };
    let new_item = item_id(node);
    patch_references(engine, &copied, &remap, vec![]);
    engine.edits.push(Edit::AddUserItemTrees { items: paired });
    Ok(MediaEditResult::MatrixCreated {
        items: vec![new_item],
        columns: vec![],
    })
}

pub(super) fn column_group(
    engine: &mut AppEngine,
    document: &Document,
    group: ColumnGroupId,
) -> Result<MediaEditResult, String> {
    let root = document.column_parent(Some(group))?;
    let range = document
        .snapshot
        .column_groups
        .iter()
        .find(|entry| entry.id == group)
        .unwrap();
    let columns: HashSet<_> = document.snapshot.columns
        [range.column_start as usize..(range.column_start + range.column_count) as usize]
        .iter()
        .map(|column| column.id)
        .collect();
    let mut groups = HashSet::from([group]);
    for current in &document.snapshot.column_groups {
        if current.parent.is_some_and(|parent| groups.contains(&parent)) {
            groups.insert(current.id);
        }
    }
    let cells: Vec<_> = document
        .snapshot
        .cells
        .iter()
        .filter(|cell| columns.contains(&cell.definition.column))
        .map(|cell| document.cells[&(cell.layer, cell.definition.column)])
        .collect();
    document.admit(columns.len() + groups.len() + cells.len())?;
    let parent = engine.nodes.get(root).unwrap().node_data().parent.unwrap();
    let mut root_spec = spec(root, parent);
    root_spec.new_prev_sibling = Some(root);
    let mut specs = vec![root_spec];
    specs.extend(
        cells
            .iter()
            .map(|id| spec(*id, engine.nodes.get(*id).unwrap().node_data().parent.unwrap())),
    );
    let copies = duplicate(engine, specs)?;
    let (remap, copied) = mapping(
        engine,
        std::iter::once((root, copies[0])).chain(cells.into_iter().zip(copies.iter().copied().skip(1))),
    )?;
    patch_references(engine, &copied, &remap, vec![]);
    Ok(MediaEditResult::MatrixColumnGroupCreated {
        group: ColumnGroupId(engine.nodes.get(copies[0]).unwrap().node_data().meta.uuid),
    })
}
