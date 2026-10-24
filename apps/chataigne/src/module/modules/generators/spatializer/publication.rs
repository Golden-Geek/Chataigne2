use super::*;
use golden_core::edit::{Edit, EditQueue, NodeReadGuard};
use golden_core::parameter::ParameterEventBehaviour;
use golden_io::{PayloadPool, RetainedPayload};
use std::sync::Mutex;
use worker::Prepared;

pub(super) const PAIRS_PER_TURN: usize = 16;
pub(super) const TREE_NODES_PER_TURN: usize = 32;
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
    row: Option<RetainedPayload<Mutex<Option<NodeTree>>>>,
    row_filled: usize,
}

fn rows() -> Result<PayloadPool, String> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|e| e.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new("spatializer-row-disposer", 16, 128 * 1024 * 1024).map_err(|e| e.to_string())?);
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
        }
    }
    pub fn reset(&mut self) {
        self.outer = 0;
        self.phase = Phase::Outer;
        self.row = None;
        self.row_filled = 0;
    }
    pub fn done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }
    #[cfg(test)]
    pub(super) fn detached_nodes(&self) -> usize {
        self.row_filled
    }
    fn parameter(&self, inner: usize) -> Parameter {
        let outer = &self.prepared.outer()[self.outer];
        let point = &self.prepared.inner()[inner];
        let value = self.prepared.value(self.outer, inner);
        match self.prepared.config.value_layout {
            SpatializerValueLayout::SourceCentric => target_value_param(outer, point, value),
            SpatializerValueLayout::TargetCentric => source_value_param(outer, point, value),
        }
    }
    fn outer_decls(&self) -> &HashSet<String> {
        match self.prepared.config.value_layout {
            SpatializerValueLayout::SourceCentric => &self.prepared.source_decls,
            SpatializerValueLayout::TargetCentric => &self.prepared.target_decls,
        }
    }
    fn inner_decls(&self) -> &HashSet<String> {
        match self.prepared.config.value_layout {
            SpatializerValueLayout::SourceCentric => &self.prepared.target_decls,
            SpatializerValueLayout::TargetCentric => &self.prepared.source_decls,
        }
    }

    pub fn step(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        values: NodeId,
        edits: &mut EditQueue,
        reads: &mut Vec<NodeReadGuard>,
    ) -> Result<(), String> {
        if self.outer >= self.prepared.outer().len() && !matches!(self.phase, Phase::CleanupOuter(_) | Phase::Done) {
            self.phase = Phase::CleanupOuter(0);
        }
        match self.phase {
            Phase::Outer => {
                let outer = &self.prepared.outer()[self.outer];
                if let Some(id) = snapshot.find_child_by_decl_id(values, &outer.value_decl_id) {
                    // An externally restored folder supersedes a detached preparation.
                    self.row = None;
                    self.row_filled = 0;
                    patch_label(snapshot, edits, id, &outer.label);
                    reads.push(NodeReadGuard::capture(snapshot, id).ok_or("Spatializer row disappeared")?);
                    self.phase = Phase::Pairs(0);
                } else {
                    let uuid = match self.prepared.config.value_layout {
                        SpatializerValueLayout::SourceCentric => source_folder_uuid(outer.item_uuid),
                        SpatializerValueLayout::TargetCentric => target_folder_uuid(outer.item_uuid),
                    };
                    if relocate(snapshot, uuid, &outer.value_decl_id, values, edits, reads)? {
                        self.row = None;
                        self.row_filled = 0;
                        return Ok(());
                    }
                    if self.row.is_none() {
                        let weight = 16384 + (self.prepared.inner().len() + 1) * 8192;
                        let reservation = rows()?
                            .try_reserve(weight)
                            .map_err(|_| "Spatializer detached row capacity is full")?;
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
                        let tree = row.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap();
                        edits.push(Edit::AddNodeTree {
                            tree,
                            parent: values,
                            prev_sibling: None,
                        });
                        edits.retain_payload(row.clone());
                        self.row = None;
                        self.row_filled = 0;
                        self.outer += 1;
                    }
                }
            }
            Phase::Pairs(start) => {
                let outer = &self.prepared.outer()[self.outer];
                let Some(folder) = snapshot.find_child_by_decl_id(values, &outer.value_decl_id) else {
                    self.phase = Phase::Outer;
                    return Ok(());
                };
                reads.push(NodeReadGuard::capture(snapshot, folder).unwrap());
                let end = (start + PAIRS_PER_TURN).min(self.prepared.inner().len());
                for inner in start..end {
                    let point = &self.prepared.inner()[inner];
                    if let Some(id) = snapshot.find_child_by_decl_id(folder, &point.value_decl_id) {
                        let node = snapshot.node(id).ok_or("Spatializer output disappeared")?;
                        reads.push(NodeReadGuard::capture(snapshot, id).unwrap());
                        let value = self.prepared.value(self.outer, inner);
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
                self.phase = if end == self.prepared.inner().len() {
                    Phase::CleanupInner(0)
                } else {
                    Phase::Pairs(end)
                };
            }
            Phase::CleanupInner(start) => {
                let outer = &self.prepared.outer()[self.outer];
                let Some(folder) = snapshot.find_child_by_decl_id(values, &outer.value_decl_id) else {
                    self.phase = Phase::Outer;
                    return Ok(());
                };
                reads.push(NodeReadGuard::capture(snapshot, folder).unwrap());
                let children = snapshot.child_ids_slice(folder);
                let mut removed = 0;
                for id in children.iter().skip(start).take(PAIRS_PER_TURN) {
                    let Some(node) = snapshot.node(*id) else { continue };
                    if generated(&node.decl_id)
                        && (!self.inner_decls().contains(&node.decl_id)
                            || snapshot.find_child_by_decl_id(folder, &node.decl_id) != Some(*id))
                    {
                        reads.push(NodeReadGuard::capture(snapshot, *id).unwrap());
                        edits.push(Edit::RemoveNode { node: *id });
                        removed += 1;
                    }
                }
                if start + PAIRS_PER_TURN >= children.len() {
                    self.outer += 1;
                    self.phase = Phase::Outer;
                } else {
                    self.phase = Phase::CleanupInner(start + PAIRS_PER_TURN - removed);
                }
            }
            Phase::CleanupOuter(start) => {
                let children = snapshot.child_ids_slice(values);
                let mut removed = 0;
                // One whole-row removal per turn bounds lifecycle work by the
                // admitted row size; never submit a Cartesian removal batch.
                let mut visited = 0;
                for id in children.iter().skip(start).take(PAIRS_PER_TURN) {
                    visited += 1;
                    let Some(node) = snapshot.node(*id) else { continue };
                    if generated(&node.decl_id)
                        && (!self.outer_decls().contains(&node.decl_id)
                            || snapshot.find_child_by_decl_id(values, &node.decl_id) != Some(*id))
                    {
                        reads.push(NodeReadGuard::capture(snapshot, *id).unwrap());
                        edits.push(Edit::RemoveNode { node: *id });
                        removed = 1;
                        break;
                    }
                }
                self.phase = if start + visited >= children.len() {
                    Phase::Done
                } else {
                    Phase::CleanupOuter(start + visited - removed)
                };
            }
            Phase::Done => {}
        }
        Ok(())
    }
}

fn generated(decl: &str) -> bool {
    decl.starts_with(VALUE_SOURCE_DECL_PREFIX) || decl.starts_with(VALUE_TARGET_DECL_PREFIX)
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
