//! Pure set reconciliation. Runtime owners provide committed cursors and retained final-use holds.
//! Computing a set cannot launch/stop a process, change selection, or display a preloaded column.
use super::protocol::*;
use crate::matrix::protocol::{ColumnId, ColumnSnapshot};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DemandReason {
    Neighbor,
    Preparation,
    Active,
    Outgoing,
    FinalUse,
}
#[derive(Clone, Copy, Debug)]
pub struct DemandCursor {
    pub scope: uuid::Uuid,
    pub column: ColumnId,
    pub reason: DemandReason,
}
#[derive(Clone, Copy, Debug)]
pub struct DemandHold {
    pub application: ApplicationId,
    pub reason: DemandReason,
}
pub struct MatrixDemand<'a> {
    pub columns: &'a [ColumnSnapshot],
    pub cursors: &'a [DemandCursor],
    pub holds: &'a [DemandHold],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefinitionAvailability {
    Configured,
    Unconfigured,
    Disabled,
    Missing,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationDemand {
    pub application: ApplicationId,
    pub availability: DefinitionAvailability,
    pub reasons: BTreeSet<DemandReason>,
}
#[derive(Clone, Copy, Debug)]
pub struct DemandLimits {
    pub columns: usize,
    pub cursors: usize,
    pub holds: usize,
    pub applications: usize,
}
impl Default for DemandLimits {
    fn default() -> Self {
        Self {
            columns: 100_000,
            cursors: 128,
            holds: 4096,
            applications: 4096,
        }
    }
}

fn add(
    result: &mut HashMap<ApplicationId, BTreeSet<DemandReason>>,
    id: ApplicationId,
    reason: DemandReason,
    limit: usize,
) -> Result<(), String> {
    if !result.contains_key(&id) && result.len() >= limit {
        return Err("application demand exceeds admitted unique definitions".into());
    }
    result.entry(id).or_default().insert(reason);
    Ok(())
}

/// Whole next demand set, reconciled atomically by the lifecycle owner. Direct holds remain present
/// after authored deletion/disable; unavailable definitions can retain an existing owner, not start one.
pub fn resolve(
    catalog: &[ApplicationSummary],
    matrices: &[MatrixDemand<'_>],
    limits: DemandLimits,
) -> Result<Vec<ApplicationDemand>, String> {
    if matrices.len() > 128 || catalog.len() > limits.applications {
        return Err("application demand input exceeds admitted catalogs/matrices".into());
    }
    let mut columns_left = limits.columns;
    let mut cursors_left = limits.cursors;
    let mut holds_left = limits.holds;
    let mut result = HashMap::new();
    for matrix in matrices {
        columns_left = columns_left
            .checked_sub(matrix.columns.len())
            .ok_or("application demand column bound exceeded")?;
        cursors_left = cursors_left
            .checked_sub(matrix.cursors.len())
            .ok_or("application demand cursor bound exceeded")?;
        holds_left = holds_left
            .checked_sub(matrix.holds.len())
            .ok_or("application demand hold bound exceeded")?;
        let mut positions = HashMap::new();
        for (position, column) in matrix.columns.iter().enumerate() {
            if positions.insert(column.id, position).is_some() {
                return Err("application demand has duplicate column identities".into());
            }
            if column.definition.applications.len() > 64 {
                return Err("application demand column exceeds 64 associations".into());
            }
        }
        let mut cursors = Vec::new();
        for cursor in matrix.cursors {
            if !matches!(
                cursor.reason,
                DemandReason::Preparation | DemandReason::Active | DemandReason::Outgoing
            ) {
                return Err("cursor demand reason must be preparation, active or outgoing".into());
            }
            if let Some(position) = positions.get(&cursor.column) {
                cursors.push((*position, cursor.reason));
            }
            // A removed column contributes only its separately retained runtime holds.
        }
        cursors.sort_unstable();
        cursors.dedup();
        for (position, column) in matrix.columns.iter().enumerate() {
            for association in &column.definition.applications {
                let low = position.saturating_sub(usize::from(association.pre));
                let high = position
                    .saturating_add(usize::from(association.post))
                    .min(matrix.columns.len().saturating_sub(1));
                let begin = cursors.partition_point(|(index, _)| *index < low);
                let end = cursors.partition_point(|(index, _)| *index <= high);
                // Neighbor scopes collapse to one reason; avoid multiplying every association by
                // the scope count. Only up to three exact-column reasons remain after deduplication.
                let admitted = &cursors[begin..end];
                if admitted.first().is_some_and(|(index, _)| *index != position)
                    || admitted.last().is_some_and(|(index, _)| *index != position)
                {
                    add(
                        &mut result,
                        association.application,
                        DemandReason::Neighbor,
                        limits.applications,
                    )?;
                }
                let exact_begin = admitted.partition_point(|(index, _)| *index < position);
                let exact_end = admitted.partition_point(|(index, _)| *index <= position);
                for &(_, reason) in &admitted[exact_begin..exact_end] {
                    add(&mut result, association.application, reason, limits.applications)?;
                }
            }
        }
        for hold in matrix.holds {
            if hold.reason == DemandReason::Neighbor {
                return Err("direct holds require a preparation/active/outgoing/final-use reason".into());
            }
            add(&mut result, hold.application, hold.reason, limits.applications)?;
        }
    }
    let mut definitions = HashMap::new();
    for entry in catalog {
        if definitions.insert(entry.id, entry).is_some() {
            return Err("application catalog has duplicate identities".into());
        }
    }
    let mut demand = result
        .into_iter()
        .map(|(application, reasons)| {
            let availability = match definitions.get(&application) {
                None => DefinitionAvailability::Missing,
                Some(definition) if !definition.enabled => DefinitionAvailability::Disabled,
                Some(definition) if !definition.configured => DefinitionAvailability::Unconfigured,
                Some(_) => DefinitionAvailability::Configured,
            };
            ApplicationDemand {
                application,
                availability,
                reasons,
            }
        })
        .collect::<Vec<_>>();
    demand.sort_by_key(|entry| entry.application.0.0);
    Ok(demand)
}
