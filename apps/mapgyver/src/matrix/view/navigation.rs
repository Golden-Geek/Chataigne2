//! Step from committed playback through the cached document order.
use super::ViewIndex;
use crate::{
    app::{AppEngine, AppNode},
    matrix::protocol::{ColumnGroupId, ColumnId, ColumnItemId, LaunchMode, LayerId, MatrixNavigationDirection},
    media::protocol::{MediaEditResult, MediaId},
};
use MatrixNavigationDirection::{Next, Previous};
use golden_core::node::{Node, NodeId};

pub(in crate::matrix) enum AxisTarget {
    Column,
    TopLevelGroup,
}

pub(in crate::matrix) fn launch(
    engine: &mut AppEngine,
    media: MediaId,
    direction: MatrixNavigationDirection,
    scope: AxisTarget,
) -> Result<MediaEditResult, String> {
    let current = crate::matrix::playback::inspect(engine, media)?;
    let MediaEditResult::MatrixPlayback { snapshot } = &current else {
        unreachable!("Matrix playback inspection always returns its snapshot")
    };
    let index = super::index(engine, media)?;
    let anchor = snapshot
        .desired_column
        .and_then(|column| index.column_positions.get(&column).copied());
    let target = match scope {
        AxisTarget::Column => adjacent_column(engine, &index, anchor, direction),
        AxisTarget::TopLevelGroup => adjacent_group(engine, &index, anchor, direction),
    };
    match target {
        Some(column) => crate::matrix::playback::launch(engine, media, column, LaunchMode::Configured),
        None => Ok(current),
    }
}

pub(in crate::matrix) fn launch_layer_cell(
    engine: &mut AppEngine,
    media: MediaId,
    layer: LayerId,
    direction: MatrixNavigationDirection,
) -> Result<MediaEditResult, String> {
    let root = crate::matrix::playback::root(engine, media)?;
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(root) else {
        unreachable!()
    };
    let control = channel.matrix_playback().ok_or("Matrix playback is missing")?;
    let snapshot = control.snapshot();
    let index = super::index(engine, media)?;
    let configuration = crate::matrix::configuration::index(engine, media, control)?;
    // An explicit stopped Layer has no anchor even if the rest of the Matrix is playing.
    let anchor = snapshot
        .layers
        .iter()
        .find(|request| request.layer == layer)
        .map_or(snapshot.desired_column, |request| request.column);
    match layer_cell_target(engine, &index, &configuration, layer, anchor, direction) {
        Some(column) => crate::matrix::playback::launch_cell(engine, media, layer, column, LaunchMode::Configured),
        None => Ok(MediaEditResult::MatrixPlayback { snapshot }),
    }
}

pub(in crate::matrix) fn layer_cell_target(
    engine: &AppEngine,
    index: &ViewIndex,
    configuration: &crate::matrix::configuration::Configuration,
    layer: LayerId,
    anchor: Option<ColumnId>,
    direction: MatrixNavigationDirection,
) -> Option<ColumnId> {
    let cells = index.layer_cells.get(&layer)?;
    let anchor = anchor.and_then(|column| index.column_positions.get(&column).copied());
    let column_of = |id: NodeId| match engine.nodes.get(id) {
        Some(AppNode::MatrixCell(cell)) => cell.column(),
        _ => unreachable!("The Matrix view index contains current Cell nodes"),
    };
    let candidates = match (direction, anchor) {
        (Previous, Some(anchor)) => {
            &cells[..cells.partition_point(|&id| index.column_positions[&column_of(id)] < anchor)]
        }
        (Next, Some(anchor)) => &cells[cells.partition_point(|&id| index.column_positions[&column_of(id)] <= anchor)..],
        (_, None) => cells,
    };
    find_in_direction(candidates, direction, |&id| {
        let column = column_of(id);
        (configuration.playable.contains(&(layer, column))
            && column_enabled(engine, index, column)
            && engine
                .nodes
                .get(id)
                .is_some_and(|cell| crate::media::routing_enabled(engine, cell)))
        .then_some(column)
    })
}

pub(in crate::matrix) fn launch_group_child(
    engine: &mut AppEngine,
    media: MediaId,
    group: ColumnGroupId,
    direction: Option<MatrixNavigationDirection>,
) -> Result<MediaEditResult, String> {
    let current = crate::matrix::playback::inspect(engine, media)?;
    let MediaEditResult::MatrixPlayback { snapshot } = &current else {
        unreachable!("Matrix playback inspection always returns its snapshot")
    };
    let index = super::index(engine, media)?;
    let (_, group_node) = index.groups.get(&group).ok_or("Column Group is missing")?;
    let target = match direction {
        None => first_enabled_column(engine, &index, group),
        Some(direction) => {
            let children = crate::matrix::document::children(engine, *group_node)?
                .into_iter()
                .filter_map(|node| match engine.nodes.get(node)? {
                    AppNode::MatrixColumn(column) => Some(ColumnItemId::Column {
                        id: ColumnId(column.node_data().meta.uuid),
                    }),
                    AppNode::MatrixColumnGroup(group) => Some(ColumnItemId::Group {
                        id: ColumnGroupId(group.node_data().meta.uuid),
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();
            // A Column anywhere under a nested Group anchors to that one direct child.
            let anchor = snapshot.desired_column.and_then(|column| {
                let &position = index.column_positions.get(&column)?;
                children.iter().position(|item| match *item {
                    ColumnItemId::Column { id } => id == column,
                    ColumnItemId::Group { id } => {
                        let span = &index.groups[&id].0;
                        (span.axis_start as usize..(span.axis_start + span.axis_count) as usize).contains(&position)
                    }
                })
            });
            let candidates = match (direction, anchor) {
                (Previous, Some(position)) => &children[..position],
                (Next, Some(position)) => &children[position + 1..],
                (_, None) => &children,
            };
            find_in_direction(candidates, direction, |item| match *item {
                ColumnItemId::Column { id } => column_enabled(engine, &index, id).then_some(id),
                ColumnItemId::Group { id } => first_enabled_column(engine, &index, id),
            })
        }
    };
    match target {
        Some(column) => crate::matrix::playback::launch(engine, media, column, LaunchMode::Configured),
        None => Ok(current),
    }
}

fn adjacent_column(
    engine: &AppEngine,
    index: &ViewIndex,
    anchor: Option<usize>,
    direction: MatrixNavigationDirection,
) -> Option<ColumnId> {
    let axis = match (direction, anchor) {
        (Previous, Some(index_at)) => &index.axis[..index_at],
        (Next, Some(index_at)) => &index.axis[index_at + 1..],
        (_, None) => &index.axis,
    };
    find_in_direction(axis, direction, |item| match *item {
        ColumnItemId::Column { id } if column_enabled(engine, index, id) => Some(id),
        _ => None,
    })
}

fn adjacent_group(
    engine: &AppEngine,
    index: &ViewIndex,
    anchor: Option<usize>,
    direction: MatrixNavigationDirection,
) -> Option<ColumnId> {
    let roots = &index.top_level_groups;
    let groups = match (direction, anchor) {
        (Previous, Some(index_at)) => {
            &roots[..roots.partition_point(|id| {
                let group = &index.groups[id].0;
                (group.axis_start + group.axis_count) as usize <= index_at
            })]
        }
        (Next, Some(index_at)) => {
            &roots[roots.partition_point(|id| index.groups[id].0.axis_start as usize <= index_at)..]
        }
        (_, None) => roots,
    };
    find_in_direction(groups, direction, |&id| first_enabled_column(engine, index, id))
}

fn first_enabled_column(engine: &AppEngine, index: &ViewIndex, group: ColumnGroupId) -> Option<ColumnId> {
    let group = &index.groups[&group].0;
    let start = group.column_start as usize;
    let end = start + group.column_count as usize;
    index.columns[start..end]
        .iter()
        .find_map(|&(column, _)| column_enabled(engine, index, column).then_some(column))
}

pub(in crate::matrix) fn column_enabled(engine: &AppEngine, index: &ViewIndex, column: ColumnId) -> bool {
    engine
        .nodes
        .get(index.column_nodes[&column])
        .is_some_and(|node| crate::media::routing_enabled(engine, node))
}

fn find_in_direction<T>(
    values: &[T],
    direction: MatrixNavigationDirection,
    eligible: impl FnMut(&T) -> Option<ColumnId>,
) -> Option<ColumnId> {
    match direction {
        Previous => values.iter().rev().find_map(eligible),
        Next => values.iter().find_map(eligible),
    }
}
