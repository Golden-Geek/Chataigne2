use super::*;
use golden_core::process_ctx::SubtreeRevision;
use golden_io::RevisionGuard;
use worker::{Failure, Prepared};

pub(super) struct Row {
    pub index: usize,
    pub pairs: Vec<usize>,
    pub removals: Vec<NodeId>,
}

pub(super) struct Plan {
    pub revision: SubtreeRevision,
    pub rows: Vec<Row>,
    pub removals: Vec<NodeId>,
}

/// Inspect the leased immutable view on the worker. The engine receives only
/// changed cells and obsolete declarations, never a Cartesian scan cursor.
pub(super) fn capture(
    snapshot: &ProcessTreeSnapshot,
    values: NodeId,
    prepared: &Prepared,
    generation: &RevisionGuard,
) -> Result<Plan, Failure> {
    let (outer_decls, inner_decls) = match prepared.config.value_layout {
        SpatializerValueLayout::SourceCentric => (&prepared.source_decls, &prepared.target_decls),
        SpatializerValueLayout::TargetCentric => (&prepared.target_decls, &prepared.source_decls),
    };
    let mut plan = Plan {
        revision: snapshot
            .subtree_revision(values)
            .ok_or("Spatializer values disappeared")?,
        rows: Vec::new(),
        removals: obsolete(snapshot, values, outer_decls),
    };
    let range = unit_value_constraints().range;
    for (index, endpoint) in prepared.outer().iter().enumerate() {
        if !generation.is_current() {
            return Err(Failure::Retry("Spatializer publication planning superseded"));
        }
        let mut row = Row {
            index,
            pairs: Vec::new(),
            removals: Vec::new(),
        };
        if let Some(folder) = snapshot.find_child_by_decl_id(values, &endpoint.value_decl_id) {
            row.removals = obsolete(snapshot, folder, inner_decls);
            for (inner, point) in prepared.inner().iter().enumerate() {
                let changed = snapshot
                    .find_child_by_decl_id(folder, &point.value_decl_id)
                    .and_then(|id| snapshot.node(id))
                    .is_none_or(|node| {
                        node.label != point.label
                            || node.param_value != Some(ParamValue::Float(prepared.value(index, inner)))
                            || node.param_constraints.as_ref().map(|c| &c.range) != Some(&range)
                    });
                if changed {
                    row.pairs.push(inner);
                }
            }
            if row.pairs.is_empty()
                && row.removals.is_empty()
                && snapshot.node(folder).is_some_and(|node| node.label == endpoint.label)
            {
                continue;
            }
        }
        plan.rows.push(row);
    }
    Ok(plan)
}

fn obsolete(snapshot: &ProcessTreeSnapshot, parent: NodeId, declarations: &HashSet<String>) -> Vec<NodeId> {
    snapshot
        .child_ids_slice(parent)
        .iter()
        .copied()
        .filter(|id| {
            snapshot.node(*id).is_some_and(|node| {
                generated(&node.decl_id)
                    && (!declarations.contains(&node.decl_id)
                        || snapshot.find_child_by_decl_id(parent, &node.decl_id) != Some(*id))
            })
        })
        .collect()
}

fn generated(decl: &str) -> bool {
    decl.starts_with(VALUE_SOURCE_DECL_PREFIX) || decl.starts_with(VALUE_TARGET_DECL_PREFIX)
}
