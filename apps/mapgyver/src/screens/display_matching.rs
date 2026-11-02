//! App-owned best-effort recovery. Native metadata is evidence, never a new authored destination.
use super::protocol::{DisplayIdentityHint, DisplayInfo, ScreenId};
use std::collections::HashMap;

/// Strongest agreeing evidence; only EDID matching identifies monitor-provided metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMatchReason {
    Edid,
    Identity,
    Name,
    Position,
    Index,
    Extent,
    Fallback,
}

/// One allocation in the current catalog. Weaker matches and ambiguous ties are best effort,
/// not a claim that an indistinguishable physical panel was uniquely identified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayMatch {
    pub display_index: usize,
    pub reason: DisplayMatchReason,
    /// Another equally good cohort allocation would give this Screen a different display.
    pub ambiguous: bool,
}

/// Match a bounded fullscreen cohort one-to-one against the host's at-most-32 active targets.
/// EDID, path, name, position, index and extent agreements are maximized lexicographically over
/// the whole cohort. A weak fallback cannot steal a peer's stronger match. Differing EDIDs do
/// not veto recovery when the original is absent; immutable hints reclaim it when it returns.
pub fn match_displays(
    requests: &[(ScreenId, DisplayIdentityHint)],
    displays: &[DisplayInfo],
) -> HashMap<ScreenId, DisplayMatch> {
    if requests.is_empty() || displays.is_empty() {
        return HashMap::new();
    }
    let mut requested = requests.iter().collect::<Vec<_>>();
    requested.sort_by(|(left_id, left), (right_id, right)| {
        layout_key(left)
            .cmp(&layout_key(right))
            .then(left.identity.cmp(&right.identity))
            .then(left_id.0.0.cmp(&right_id.0.0))
    });
    let mut available = (0..displays.len()).collect::<Vec<_>>();
    available.sort_by_key(|&index| {
        let display = &displays[index];
        (
            display.bounds.left,
            display.bounds.top,
            display.index,
            display.id.as_str(),
        )
    });
    let request_names = requested
        .iter()
        .map(|(_, hint)| normalize_name(&hint.name))
        .collect::<Vec<_>>();
    let display_names = available
        .iter()
        .map(|&index| normalize_name(&displays[index].name))
        .collect::<Vec<_>>();
    let closeness = requested
        .iter()
        .map(|(_, hint)| {
            let distances = available
                .iter()
                .map(|&index| distance(hint, &displays[index]))
                .collect::<Vec<_>>();
            let mut distinct = distances.clone();
            distinct.sort_unstable();
            distinct.dedup();
            distances
                .iter()
                .map(|distance| distinct.len() - distinct.binary_search(distance).unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let matched = requested.len().min(available.len()) as i128;
    let base = matched + 1;
    let proximity_base = matched * (available.len() as i128 + 1) + 1;
    // Rows are physical targets, keeping assignment work bounded by the native catalog rather
    // than graph size. Dummy request columns permit every unused target to remain unallocated.
    let scores = available
        .iter()
        .enumerate()
        .map(|(row, &index)| {
            let mut values = requested
                .iter()
                .enumerate()
                .map(|(column, (_, hint))| {
                    let evidence = evidence(hint, &displays[index], &request_names[column], &display_names[row]);
                    let quality = evidence
                        .into_iter()
                        .fold(0_i128, |score, agrees| score * base + i128::from(agrees));
                    quality * proximity_base + closeness[column][row] as i128 + 1
                })
                .collect::<Vec<_>>();
            values.resize(requested.len() + available.len(), 0);
            values
        })
        .collect::<Vec<_>>();
    let (allocation, total) = assign(&scores, None);
    let mut result = HashMap::new();
    for (row, &column) in allocation.iter().enumerate() {
        if column >= requested.len() {
            continue;
        }
        let (screen, hint) = requested[column];
        let display_index = available[row];
        let flags = evidence(
            hint,
            &displays[display_index],
            &request_names[column],
            &display_names[row],
        );
        let reason = [
            DisplayMatchReason::Edid,
            DisplayMatchReason::Identity,
            DisplayMatchReason::Name,
            DisplayMatchReason::Position,
            DisplayMatchReason::Index,
            DisplayMatchReason::Extent,
        ]
        .into_iter()
        .zip(flags)
        .find_map(|(reason, agrees)| agrees.then_some(reason))
        .unwrap_or(DisplayMatchReason::Fallback);
        let (_, alternative) = assign(&scores, Some((row, column)));
        result.insert(
            *screen,
            DisplayMatch {
                display_index,
                reason,
                ambiguous: alternative == total,
            },
        );
    }
    result
}

fn normalize_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn evidence(hint: &DisplayIdentityHint, display: &DisplayInfo, name: &str, display_name: &str) -> [bool; 6] {
    [
        hint.edid
            .as_deref()
            .zip(display.edid.as_deref())
            .is_some_and(|(left, right)| !left.is_empty() && left.eq_ignore_ascii_case(right)),
        !hint.identity.is_empty() && hint.identity.eq_ignore_ascii_case(&display.id),
        !name.is_empty() && name == display_name,
        hint.bounds
            .is_some_and(|bounds| bounds.left == display.bounds.left && bounds.top == display.bounds.top),
        hint.index != 0 && hint.index == display.index,
        hint.bounds.is_some_and(|bounds| bounds.extent == display.bounds.extent),
    ]
}

fn layout_key(hint: &DisplayIdentityHint) -> (i32, i32, u32) {
    let (left, top) = hint
        .bounds
        .map_or((i32::MAX, i32::MAX), |bounds| (bounds.left, bounds.top));
    (left, top, if hint.index == 0 { u32::MAX } else { hint.index })
}

fn distance(hint: &DisplayIdentityHint, display: &DisplayInfo) -> (u64, u32, u64) {
    let position = hint.bounds.map_or(u64::MAX, |bounds| {
        u64::from(bounds.left.abs_diff(display.bounds.left)) + u64::from(bounds.top.abs_diff(display.bounds.top))
    });
    let index = if hint.index == 0 || display.index == 0 {
        u32::MAX
    } else {
        hint.index.abs_diff(display.index)
    };
    let extent = hint.bounds.map_or(u64::MAX, |bounds| {
        u64::from(bounds.extent.width().abs_diff(display.bounds.extent.width()))
            + u64::from(bounds.extent.height().abs_diff(display.bounds.extent.height()))
    });
    (position, index, extent)
}

/// Rectangular shortest augmenting-path assignment. Native targets are the small row dimension;
/// exact i128 scores preserve evidence priorities without floating-point or greedy tie drift.
fn assign(scores: &[Vec<i128>], excluded: Option<(usize, usize)>) -> (Vec<usize>, i128) {
    let rows = scores.len();
    let columns = scores[0].len();
    let mut row_potential = vec![0_i128; rows + 1];
    let mut column_potential = vec![0_i128; columns + 1];
    let mut owner = vec![0_usize; columns + 1];
    let mut previous = vec![0_usize; columns + 1];
    for row in 1..=rows {
        owner[0] = row;
        let mut column = 0;
        let mut minimum = vec![i128::MAX / 4; columns + 1];
        let mut visited = vec![false; columns + 1];
        loop {
            visited[column] = true;
            let current = owner[column];
            let mut delta = i128::MAX / 4;
            let mut next = 0;
            for candidate in 1..=columns {
                if visited[candidate] {
                    continue;
                }
                // Every row has a zero-cost dummy, so an excluded positive-cost pair is never needed.
                let cost = if excluded == Some((current - 1, candidate - 1)) {
                    1
                } else {
                    -scores[current - 1][candidate - 1]
                };
                let reduced = cost - row_potential[current] - column_potential[candidate];
                if reduced < minimum[candidate] {
                    minimum[candidate] = reduced;
                    previous[candidate] = column;
                }
                if minimum[candidate] < delta {
                    delta = minimum[candidate];
                    next = candidate;
                }
            }
            for candidate in 0..=columns {
                if visited[candidate] {
                    row_potential[owner[candidate]] += delta;
                    column_potential[candidate] -= delta;
                } else {
                    minimum[candidate] -= delta;
                }
            }
            column = next;
            if owner[column] == 0 {
                break;
            }
        }
        loop {
            let prior = previous[column];
            owner[column] = owner[prior];
            column = prior;
            if column == 0 {
                break;
            }
        }
    }
    let mut allocation = vec![0; rows];
    for column in 1..=columns {
        if owner[column] != 0 {
            allocation[owner[column] - 1] = column - 1;
        }
    }
    let total = allocation
        .iter()
        .enumerate()
        .map(|(row, &column)| scores[row][column])
        .sum();
    (allocation, total)
}

#[cfg(test)]
mod tests;
