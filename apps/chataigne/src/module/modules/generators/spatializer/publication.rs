use super::*;
use golden_core::edit::{Edit, EditQueue, NodeReadGuard};
use golden_core::parameter::ParameterEventBehaviour;
use golden_core::process_ctx::SubtreeStructureRevision;
use golden_io::{PayloadPool, RetainedPayload};
use std::sync::Mutex;
use worker::Prepared;

pub(super) const PAIRS_PER_TURN: usize = 16;
pub(super) const TREE_NODES_PER_TURN: usize = 32;
pub(super) const ROWS_PER_BATCH: usize = 16;

type DetachedRow = RetainedPayload<Mutex<Option<NodeTree>>>;

struct RowBatch {
    start: usize,
    revision: SubtreeStructureRevision,
    guard: NodeReadGuard,
    rows: Vec<DetachedRow>,
}
enum Phase {
    Outer,
    Pairs(usize),
    CleanupInner(usize),
    CleanupOuter(usize),
    Done,
}
pub(super) struct Publication {
    pub prepared: RetainedPayload<Prepared>,
    outer: usize,
    phase: Phase,
    row: Option<DetachedRow>,
    row_filled: usize,
    batch: Option<RowBatch>,
}

pub(super) fn rows() -> Result<PayloadPool, String> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|e| e.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("spatializer-row-disposer", ROWS_PER_BATCH, 128 * 1024 * 1024)
                .map_err(|e| e.to_string())?,
        );
    }
    Ok(pool.as_ref().unwrap().clone())
}
impl Publication {
    pub fn new(prepared: RetainedPayload<Prepared>) -> Self {
        Self {
            prepared,
            outer: 0,
            phase: Phase::Outer,
            row: None,
            row_filled: 0,
            batch: None,
        }
    }
    pub fn done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }
    #[cfg(test)]
    pub(super) fn detached_nodes(&self) -> usize {
        self.row_filled
    }
    #[cfg(test)]
    pub(super) fn staged_rows(&self) -> usize {
        self.batch.as_ref().map_or(0, |batch| batch.rows.len())
    }

    fn flush_rows(&mut self, values: NodeId, edits: &mut EditQueue, reads: &mut Vec<NodeReadGuard>) {
        if let Some(batch) = self.batch.take() {
            reads.push(batch.guard);
            for row in batch.rows {
                publish_row(row, values, edits);
            }
        }
    }
    fn parameter(&self, inner: usize) -> Parameter {
        let index = self.work().index;
        let outer = &self.prepared.outer()[index];
        let point = &self.prepared.inner()[inner];
        let value = self.prepared.value(index, inner);
        match self.prepared.config.value_layout {
            SpatializerValueLayout::SourceCentric => target_value_param(outer, point, value),
            SpatializerValueLayout::TargetCentric => source_value_param(outer, point, value),
        }
    }
    fn plan(&self) -> &output_plan::Plan {
        self.prepared.output.as_ref().unwrap()
    }
    fn work(&self) -> &output_plan::Row {
        &self.plan().rows[self.outer]
    }

    pub fn step(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        values: NodeId,
        edits: &mut EditQueue,
        reads: &mut Vec<NodeReadGuard>,
    ) -> Result<(), String> {
        let mut remaining = PAIRS_PER_TURN;
        let mut guarded_row = None;
        while remaining > 0 && reads.len() < 26 && !self.done() {
            // Detached row construction keeps its independent 32-child turn
            // bound. Only sparse existing-row work shares a publication turn.
            let detached = matches!(self.phase, Phase::Outer)
                && self.outer < self.plan().rows.len()
                && snapshot
                    .find_child_by_decl_id(values, &self.prepared.outer()[self.work().index].value_decl_id)
                    .is_none();
            if detached && (!edits.pending.is_empty() || remaining != PAIRS_PER_TURN) {
                break;
            }
            let had_batch = self.batch.is_some();
            self.step_one(snapshot, values, edits, reads, &mut remaining, &mut guarded_row)?;
            if detached || had_batch || self.batch.is_some() {
                break;
            }
        }
        Ok(())
    }

    fn step_one(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        values: NodeId,
        edits: &mut EditQueue,
        reads: &mut Vec<NodeReadGuard>,
        remaining: &mut usize,
        guarded_row: &mut Option<NodeId>,
    ) -> Result<(), String> {
        if self
            .batch
            .as_ref()
            .is_some_and(|batch| snapshot.subtree_structure_revision(values) != Some(batch.revision))
        {
            // None of this forest has been submitted. Revisit its first row against
            // current topology rather than duplicate an externally restored folder.
            self.outer = self.batch.take().unwrap().start;
            self.phase = Phase::Outer;
            self.row = None;
            self.row_filled = 0;
        }
        if self.outer >= self.plan().rows.len() && !matches!(self.phase, Phase::CleanupOuter(_) | Phase::Done) {
            self.phase = Phase::CleanupOuter(0);
        }
        if self.batch.is_some() && !matches!(self.phase, Phase::Outer) {
            self.flush_rows(values, edits, reads);
            return Ok(());
        }
        match self.phase {
            Phase::Outer => {
                let outer = &self.prepared.outer()[self.work().index];
                let uuid = match self.prepared.config.value_layout {
                    SpatializerValueLayout::SourceCentric => source_folder_uuid(outer.item_uuid),
                    SpatializerValueLayout::TargetCentric => target_folder_uuid(outer.item_uuid),
                };
                if self.batch.is_some()
                    && (snapshot.find_child_by_decl_id(values, &outer.value_decl_id).is_some()
                        || snapshot.node_id_by_uuid(uuid).is_some())
                {
                    self.flush_rows(values, edits, reads);
                    return Ok(());
                }
                if let Some(id) = snapshot.find_child_by_decl_id(values, &outer.value_decl_id) {
                    // An externally restored folder supersedes a detached preparation.
                    self.row = None;
                    self.row_filled = 0;
                    patch_label(snapshot, edits, id, &outer.label);
                    read_row(snapshot, id, reads, guarded_row)?;
                    self.phase = Phase::Pairs(0);
                } else {
                    if relocate(snapshot, uuid, &outer.value_decl_id, values, edits, reads)? {
                        self.row = None;
                        self.row_filled = 0;
                        return Ok(());
                    }
                    if self.row.is_none() {
                        let weight = 16384 + (self.prepared.inner().len() + 1) * 8192;
                        let reservation = match rows()?.try_reserve(weight) {
                            Ok(reservation) => reservation,
                            Err(_) if self.batch.is_some() => {
                                // Competing modules may fill the shared pool before this
                                // batch is full. Publish its admitted prefix to make room.
                                self.flush_rows(values, edits, reads);
                                return Ok(());
                            }
                            Err(_) => return Err("Spatializer detached row capacity is full".into()),
                        };
                        self.row = Some(
                            reservation
                                .retain(
                                    Mutex::new(Some(NodeTree::new(value_folder(
                                        &outer.label,
                                        &outer.value_decl_id,
                                        uuid,
                                    )))),
                                    weight,
                                )
                                .unwrap(),
                        );
                        self.row_filled = 0;
                    }
                    let row = self.row.as_ref().unwrap();
                    let end = (self.row_filled + TREE_NODES_PER_TURN).min(self.prepared.inner().len());
                    {
                        let mut tree = row.lock().unwrap_or_else(|e| e.into_inner());
                        for inner in self.row_filled..end {
                            tree.as_mut().unwrap().push_child(NodeTree::new(self.parameter(inner)));
                        }
                    }
                    self.row_filled = end;
                    if end == self.prepared.inner().len() {
                        let row = self.row.take().unwrap();
                        self.row_filled = 0;
                        if end <= TREE_NODES_PER_TURN {
                            if self.batch.is_none() {
                                self.batch = Some(RowBatch {
                                    start: self.outer,
                                    revision: snapshot
                                        .subtree_structure_revision(values)
                                        .ok_or("Spatializer values disappeared")?,
                                    guard: NodeReadGuard::capture_structure(snapshot, values)
                                        .ok_or("Spatializer values disappeared")?,
                                    rows: Vec::with_capacity(ROWS_PER_BATCH),
                                });
                            }
                            self.batch.as_mut().unwrap().rows.push(row);
                        } else {
                            publish_row(row, values, edits);
                        }
                        self.outer += 1;
                        if self
                            .batch
                            .as_ref()
                            .is_some_and(|batch| batch.rows.len() == ROWS_PER_BATCH)
                            || self.outer == self.plan().rows.len()
                        {
                            self.flush_rows(values, edits, reads);
                        }
                    }
                }
            }
            Phase::Pairs(start) => {
                let outer = &self.prepared.outer()[self.work().index];
                let Some(folder) = snapshot.find_child_by_decl_id(values, &outer.value_decl_id) else {
                    self.phase = Phase::Outer;
                    return Ok(());
                };
                read_row(snapshot, folder, reads, guarded_row)?;
                let count = (*remaining).min((28 - reads.len()) / 2);
                let end = (start + count).min(self.work().pairs.len());
                for cursor in start..end {
                    let inner = self.work().pairs[cursor];
                    let point = &self.prepared.inner()[inner];
                    if let Some(id) = snapshot.find_child_by_decl_id(folder, &point.value_decl_id) {
                        let node = snapshot.node(id).ok_or("Spatializer output disappeared")?;
                        reads.push(NodeReadGuard::capture(snapshot, id).unwrap());
                        let value = self.prepared.value(self.work().index, inner);
                        if !matches!(node.param_value, Some(ParamValue::Float(_))) {
                            let mut replacement = self.parameter(inner);
                            replacement.node_data_mut().meta.uuid = node.uuid;
                            edits.push(Edit::ReplaceNode {
                                node: id,
                                new_node: Box::new(replacement),
                            });
                        } else {
                            patch_label(snapshot, edits, id, &point.label);
                            if node.param_constraints.as_ref().map(|c| &c.range)
                                != Some(&unit_value_constraints().range)
                            {
                                edits.push(Edit::SetParamConstraints {
                                    node: id,
                                    constraints: unit_value_constraints(),
                                });
                            }
                            if node.param_value != Some(ParamValue::Float(value)) {
                                edits.push(Edit::SetParam {
                                    node: id,
                                    value: ParamValue::Float(value),
                                    behaviour: ParameterEventBehaviour::Coalesce,
                                });
                            }
                        }
                    } else {
                        let uuid = match self.prepared.config.value_layout {
                            SpatializerValueLayout::SourceCentric => {
                                target_value_uuid(outer.item_uuid, point.item_uuid)
                            }
                            SpatializerValueLayout::TargetCentric => {
                                source_value_uuid(outer.item_uuid, point.item_uuid)
                            }
                        };
                        if relocate(snapshot, uuid, &point.value_decl_id, folder, edits, reads)? {
                            continue;
                        }
                        edits.push(Edit::AddNodeTree {
                            tree: NodeTree::new(self.parameter(inner)),
                            parent: folder,
                            prev_sibling: None,
                        });
                    }
                }
                *remaining -= end - start;
                self.phase = if end == self.work().pairs.len() {
                    Phase::CleanupInner(0)
                } else {
                    Phase::Pairs(end)
                };
            }
            Phase::CleanupInner(start) => {
                let outer = &self.prepared.outer()[self.work().index];
                let Some(folder) = snapshot.find_child_by_decl_id(values, &outer.value_decl_id) else {
                    self.phase = Phase::Outer;
                    return Ok(());
                };
                read_row(snapshot, folder, reads, guarded_row)?;
                let end = (start + (*remaining).min(28 - reads.len())).min(self.work().removals.len());
                for id in &self.work().removals[start..end] {
                    if snapshot.node(*id).is_some_and(|node| node.parent == Some(folder)) {
                        reads.push(NodeReadGuard::capture(snapshot, *id).unwrap());
                        edits.push(Edit::RemoveNode { node: *id });
                    }
                }
                *remaining -= end - start;
                if end == self.work().removals.len() {
                    self.outer += 1;
                    self.phase = Phase::Outer;
                } else {
                    self.phase = Phase::CleanupInner(end);
                }
            }
            Phase::CleanupOuter(start) => {
                // One whole-row removal per turn bounds lifecycle work by the
                // admitted row size; never submit a Cartesian removal batch.
                if let Some(id) = self.plan().removals.get(start) {
                    if snapshot.node(*id).is_some_and(|node| node.parent == Some(values)) {
                        reads.push(NodeReadGuard::capture(snapshot, *id).unwrap());
                        edits.push(Edit::RemoveNode { node: *id });
                    }
                    *remaining = 0;
                }
                self.phase = if start + 1 >= self.plan().removals.len() {
                    Phase::Done
                } else {
                    Phase::CleanupOuter(start + 1)
                };
            }
            Phase::Done => {}
        }
        Ok(())
    }
}

/// All phases in one planning turn borrow the same immutable snapshot. A row
/// needs one authored fence, even when that turn visits pairs and cleanup too.
fn read_row(
    snapshot: &ProcessTreeSnapshot,
    row: NodeId,
    reads: &mut Vec<NodeReadGuard>,
    guarded: &mut Option<NodeId>,
) -> Result<(), String> {
    if *guarded != Some(row) {
        reads.push(NodeReadGuard::capture(snapshot, row).ok_or("Spatializer row disappeared")?);
        *guarded = Some(row);
    }
    Ok(())
}

fn publish_row(row: DetachedRow, values: NodeId, edits: &mut EditQueue) {
    let tree = row.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap();
    edits.push(Edit::AddNodeTree {
        tree,
        parent: values,
        prev_sibling: None,
    });
    edits.retain_payload(row);
}

/// Generated identities have one owner. Restore a moved node instead of
/// attempting a duplicate UUID insertion or writing through a stale route.
fn relocate(
    snapshot: &ProcessTreeSnapshot,
    uuid: NodeUuid,
    decl: &str,
    parent: NodeId,
    edits: &mut EditQueue,
    reads: &mut Vec<NodeReadGuard>,
) -> Result<bool, String> {
    let Some(id) = snapshot.node_id_by_uuid(uuid) else {
        return Ok(false);
    };
    let node = snapshot.node(id).ok_or("Spatializer generated identity disappeared")?;
    if node.decl_id != decl || node.parent == Some(parent) {
        return Err("Spatializer generated UUID conflicts with another declaration".into());
    }
    reads.push(NodeReadGuard::capture_structure(snapshot, id).unwrap());
    edits.push(Edit::MoveNode {
        node: id,
        new_parent: parent,
        new_prev_sibling: None,
    });
    Ok(true)
}
fn patch_label(snapshot: &ProcessTreeSnapshot, edits: &mut EditQueue, node: NodeId, label: &str) {
    if snapshot.node(node).is_some_and(|node| node.label != label) {
        edits.push(Edit::PatchMeta {
            node,
            patch: NodeMetaPatch {
                label: Some(label.into()),
                ..Default::default()
            },
        });
    }
}
