use super::{
    document::{Document, MAX_DEPTH, children},
    protocol::*,
};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::protocol::MediaEditResult,
};
use golden_core::{
    edit::{Edit, NodeTree},
    node::{Node, NodeId},
};
use std::collections::{HashMap, HashSet};

fn depth(engine: &AppEngine, document: &Document, mut node: NodeId) -> usize {
    let mut depth = 0;
    while node != document.root {
        depth += 1;
        node = engine.nodes.get(node).unwrap().node_data().parent.unwrap();
    }
    depth
}

pub(super) fn apply(engine: &mut AppEngine, document: &Document, edit: MatrixEdit) -> Result<MediaEditResult, String> {
    match edit {
        MatrixEdit::AddColumnGroup { parent, label } => {
            document.admit(1)?;
            let parent = document.column_parent(parent)?;
            if depth(engine, document, parent) >= MAX_DEPTH {
                return Err("Column Groups exceed depth 32".into());
            }
            let label = labels::unique(engine, parent, label, "Column Group", &[])?;
            let node = super::ColumnGroupNode::named(label);
            let group = ColumnGroupId(node.node_data().meta.uuid);
            engine.edits.push(Edit::AddUserItemTree {
                parent,
                prev_sibling: engine.nodes.get(parent).unwrap().node_data().last_child,
                tree: NodeTree::new(node),
            });
            return Ok(MediaEditResult::MatrixColumnGroupCreated { group });
        }
        MatrixEdit::RenameColumnGroup { group, label } => {
            labels::rename(engine, document.column_parent(Some(group))?, label)?
        }
        MatrixEdit::MoveColumnItem { item, parent, after } => {
            let node = document.column_item(item)?;
            let parent_node = document.column_parent(parent)?;
            let previous = after.map(|item| document.column_item(item)).transpose()?;
            if previous == Some(node)
                || previous.is_some_and(|id| engine.nodes.get(id).unwrap().node_data().parent != Some(parent_node))
            {
                return Err("Column item move requires a distinct preceding sibling".into());
            }
            let mut cursor = parent_node;
            while cursor != document.root {
                if cursor == node {
                    return Err("a Column Group cannot move into itself or a descendant".into());
                }
                cursor = engine.nodes.get(cursor).unwrap().node_data().parent.unwrap();
            }
            if let ColumnItemId::Group { id: group } = item {
                let mut relative = HashMap::new();
                let mut height = 0;
                for current in &document.snapshot.column_groups {
                    let child_depth = if current.id == group {
                        Some(0)
                    } else {
                        current.parent.and_then(|id| relative.get(&id).map(|value| value + 1))
                    };
                    if let Some(child_depth) = child_depth {
                        relative.insert(current.id, child_depth);
                        height = height.max(child_depth);
                    }
                }
                if depth(engine, document, parent_node) + height >= MAX_DEPTH {
                    return Err("Column Groups exceed depth 32".into());
                }
            }
            engine.edits.push(Edit::MoveNode {
                node,
                new_parent: parent_node,
                new_prev_sibling: previous,
            });
        }
        MatrixEdit::UngroupColumns { group } => {
            let node = document.column_parent(Some(group))?;
            let metadata = engine.nodes.get(node).unwrap().node_data();
            let parent = metadata.parent.unwrap();
            let mut previous = metadata.prev_sibling;
            let Some(AppNode::MatrixColumnGroup(owner)) = engine.nodes.get(node) else {
                unreachable!()
            };
            for child in children(engine, node)? {
                if Some(child) == owner.application_node() {
                    continue;
                }
                engine.edits.push(Edit::MoveNode {
                    node: child,
                    new_parent: parent,
                    new_prev_sibling: previous,
                });
                previous = Some(child);
            }
            engine.edits.push(Edit::RemoveNode { node });
        }
        MatrixEdit::RemoveColumnGroup { group } => {
            let node = document.column_parent(Some(group))?;
            let snapshot = document
                .snapshot
                .column_groups
                .iter()
                .find(|entry| entry.id == group)
                .unwrap();
            let start = snapshot.column_start as usize;
            let end = start + snapshot.column_count as usize;
            let removed: HashSet<_> = document.snapshot.columns[start..end]
                .iter()
                .map(|column| column.id)
                .collect();
            for ((_, column), cell) in &document.cells {
                if removed.contains(column) {
                    engine.edits.push(Edit::RemoveNode { node: *cell });
                }
            }
            engine.edits.push(Edit::RemoveNode { node });
        }
        MatrixEdit::DuplicateColumnGroup { group } => return super::copying::column_group(engine, document, group),
        _ => unreachable!(),
    }
    Ok(MediaEditResult::Changed)
}

pub(super) fn siblings(snapshot: &MatrixSnapshot, parent: Option<ColumnGroupId>) -> Vec<ColumnItemId> {
    let groups: Vec<_> = snapshot
        .column_groups
        .iter()
        .filter(|group| group.parent == parent)
        .collect();
    let columns: Vec<_> = snapshot
        .columns
        .iter()
        .filter(|column| column.parent == parent)
        .collect();
    let mut result = vec![None; groups.len() + columns.len()];
    for group in groups {
        result[group.ordinal as usize] = Some(ColumnItemId::Group { id: group.id });
    }
    let mut columns = columns.into_iter();
    result
        .into_iter()
        .map(|item| {
            item.unwrap_or_else(|| ColumnItemId::Column {
                id: columns.next().unwrap().id,
            })
        })
        .collect()
}
