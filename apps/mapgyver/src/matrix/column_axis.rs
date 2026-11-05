//! One bounded tree walk projects ordered axis topology; no parallel authored Column lists.
use super::{
    document::{MAX_DEPTH, children},
    protocol::*,
};
use crate::app::{AppEngine, AppNode};
use golden_core::node::{Node, NodeId};
use std::collections::HashMap;

pub(super) struct Axis {
    pub columns: Vec<ColumnSnapshot>,
    pub groups: Vec<ColumnGroupSnapshot>,
    pub column_nodes: HashMap<ColumnId, NodeId>,
    pub group_nodes: HashMap<ColumnGroupId, NodeId>,
}

enum Visit {
    Enter {
        node: NodeId,
        parent: Option<ColumnGroupId>,
        depth: usize,
        ordinal: u32,
    },
    Leave(usize),
}

fn axis_node(node: &AppNode) -> bool {
    matches!(node, AppNode::MatrixColumn(_) | AppNode::MatrixColumnGroup(_))
}

pub(super) fn load(engine: &AppEngine, root: NodeId) -> Result<Axis, String> {
    let mut axis = Axis {
        columns: vec![],
        groups: vec![],
        column_nodes: HashMap::new(),
        group_nodes: HashMap::new(),
    };
    let roots = children(engine, root)?
        .into_iter()
        .filter(|id| engine.nodes.get(*id).is_some_and(axis_node));
    let mut pending = roots
        .enumerate()
        .map(|(ordinal, node)| Visit::Enter {
            node,
            parent: None,
            depth: 0,
            ordinal: ordinal as u32,
        })
        .collect::<Vec<_>>();
    pending.reverse();
    let mut slots = 0u32;
    while let Some(visit) = pending.pop() {
        let Visit::Enter {
            node: id,
            parent,
            depth,
            ordinal,
        } = visit
        else {
            let Visit::Leave(index) = visit else { unreachable!() };
            axis.groups[index].column_count = axis.columns.len() as u32 - axis.groups[index].column_start;
            axis.groups[index].axis_count = slots - axis.groups[index].axis_start;
            continue;
        };
        let node = engine.nodes.get(id).ok_or("Column axis changed")?;
        let uuid = node.node_data().meta.uuid;
        match node {
            AppNode::MatrixColumn(column) => {
                let column_id = ColumnId(uuid);
                if children(engine, id)?
                    .into_iter()
                    .any(|child| Some(child) != column.application_node())
                {
                    return Err("Column children must be its declared settings".into());
                }
                axis.column_nodes.insert(column_id, id);
                axis.columns.push(ColumnSnapshot {
                    id: column_id,
                    axis_index: slots,
                    parent,
                    label: node.node_data().meta.label.clone(),
                    definition: column.definition(),
                    application: crate::applications::protocol::ApplicationId(uuid),
                });
                slots += 1;
            }
            AppNode::MatrixColumnGroup(group) => {
                if depth >= MAX_DEPTH {
                    return Err("Column Groups exceed depth 32".into());
                }
                let group_id = ColumnGroupId(uuid);
                axis.group_nodes.insert(group_id, id);
                let index = axis.groups.len();
                axis.groups.push(ColumnGroupSnapshot {
                    id: group_id,
                    parent,
                    ordinal,
                    depth: depth as u8,
                    axis_start: slots,
                    axis_count: 0,
                    empty: false,
                    label: node.node_data().meta.label.clone(),
                    application: crate::applications::protocol::ApplicationId(uuid),
                    column_start: axis.columns.len() as u32,
                    column_count: 0,
                });
                pending.push(Visit::Leave(index));
                let mut nested = Vec::new();
                for child in children(engine, id)? {
                    if Some(child) == group.application_node() {
                        continue;
                    }
                    if !engine.nodes.get(child).is_some_and(axis_node) {
                        return Err("Column Group children must be Columns or Column Groups".into());
                    }
                    nested.push(child);
                }
                if nested.is_empty() {
                    axis.groups[index].empty = true;
                    slots += 1;
                }
                pending.extend(
                    nested
                        .into_iter()
                        .enumerate()
                        .rev()
                        .map(|(ordinal, node)| Visit::Enter {
                            node,
                            parent: Some(group_id),
                            depth: depth + 1,
                            ordinal: ordinal as u32,
                        }),
                );
            }
            _ => unreachable!(),
        }
    }
    Ok(axis)
}

pub(super) fn parent(engine: &AppEngine, node: NodeId) -> Option<ColumnGroupId> {
    let parent = engine.nodes.get(node)?.node_data().parent?;
    let AppNode::MatrixColumnGroup(group) = engine.nodes.get(parent)? else {
        return None;
    };
    Some(ColumnGroupId(group.node_data().meta.uuid))
}

/// Resolve ownership through the bounded axis hierarchy without scanning the Matrix.
pub(super) fn owner<'a>(node: NodeId, resolve: impl Fn(NodeId) -> Option<&'a dyn Node>) -> Option<NodeId> {
    let mut parent = resolve(node)?.node_data().parent?;
    for _ in 0..=MAX_DEPTH {
        let current = resolve(parent)?;
        if current
            .as_any()
            .downcast_ref::<crate::media::ChannelNode>()
            .is_some_and(|channel| channel.is_matrix_final())
        {
            return Some(parent);
        }
        if !current.as_any().is::<super::ColumnGroupNode>() {
            return None;
        }
        parent = current.node_data().parent?;
    }
    None
}
