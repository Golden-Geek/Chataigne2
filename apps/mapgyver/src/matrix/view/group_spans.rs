//! Group intervals share the cached visible-row index; viewport reads transfer only intersecting ancestry.
use super::{ViewIndex, ViewRow};
use crate::matrix::protocol::{GroupId, MatrixGroupSpan, MatrixItemId, MatrixViewRow};
use std::collections::{HashMap, HashSet};

pub(super) fn build(rows: &[ViewRow], visible: &[usize], groups: usize) -> HashMap<GroupId, MatrixGroupSpan> {
    let mut spans = HashMap::new();
    let mut stack: Vec<MatrixGroupSpan> = Vec::new();
    for &full in visible {
        let row = &rows[full];
        let position = row.visible_index.expect("visible row ordinal");
        while stack.last().is_some_and(|span| span.depth >= row.depth) {
            let mut span = stack.pop().unwrap();
            span.end = position;
            span.end_groups = row.groups_before;
            spans.insert(span.id, span);
        }
        if let MatrixItemId::Group { id } = row.id {
            stack.push(MatrixGroupSpan {
                id,
                depth: row.depth,
                start: position,
                end: 0,
                start_groups: row.groups_before,
                end_groups: 0,
            });
        }
    }
    for mut span in stack {
        span.end = visible.len() as u32;
        span.end_groups = groups as u32;
        spans.insert(span.id, span);
    }
    spans
}

pub(super) fn visible(index: &ViewIndex, rows: &[MatrixViewRow]) -> Vec<MatrixGroupSpan> {
    let mut seen = HashSet::new();
    let mut spans = Vec::new();
    for row in rows {
        let mut parent = match row.item.id {
            MatrixItemId::Group { id } => Some(id),
            MatrixItemId::Layer { .. } => row.item.parent,
        };
        while let Some(id) = parent {
            if !seen.insert(id) {
                break;
            }
            if let Some(span) = index.row_groups.get(&id) {
                spans.push(*span);
            }
            let row = &index.rows[index.row_positions[&MatrixItemId::Group { id }]];
            parent = row.parent;
        }
    }
    spans.sort_by_key(|span| (span.depth, span.start));
    spans
}
