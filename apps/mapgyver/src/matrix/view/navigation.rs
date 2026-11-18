//! Step within the cached authoring order using the committed Matrix-wide launch as the anchor.
use super::ViewIndex;
use crate::{
    app::AppEngine,
    matrix::protocol::{ColumnId, ColumnItemId, LaunchMode, MatrixNavigationDirection},
    media::protocol::{MediaEditResult, MediaId},
};
use MatrixNavigationDirection::{Next, Previous};

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
    find_in_direction(groups, direction, |id| {
        let group = &index.groups[id].0;
        let start = group.column_start as usize;
        let end = start + group.column_count as usize;
        index.columns[start..end]
            .iter()
            .find_map(|&(column, _)| column_enabled(engine, index, column).then_some(column))
    })
}

fn column_enabled(engine: &AppEngine, index: &ViewIndex, column: ColumnId) -> bool {
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
