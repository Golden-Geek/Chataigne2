use std::{collections::HashMap, sync::Arc};

use uuid::Uuid;

use crate::node::{Node, NodeId};

use super::node_store::NodeStore;
use crate::process_ctx::snapshot_map::{SnapshotLayer, SnapshotMap, SnapshotValue};

mod incremental;

/// Identity of authored changes within one subtree, independent of callback delivery.
/// Revisions are local to an engine lifetime and are never persisted or sent over the protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubtreeRevision {
    epoch: Uuid,
    sequence: u64,
}

/// Root-specific identity of topology, node kinds and named-selector metadata.
/// Parameter values, constraints, controls and presentation alone do not advance it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubtreeStructureRevision {
    node: NodeId,
    revision: SubtreeRevision,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NodeRevisions {
    authored: u64,
    structure: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct RevisionSnapshot {
    epoch: Uuid,
    sequence: u64,
    by_node: SnapshotMap<NodeId, NodeRevisions>,
}

/// Unfolded engine counters, consumed by one immutable view capture.
pub(crate) struct LocalRevisionSnapshot {
    epoch: Uuid,
    sequence: u64,
    by_node: HashMap<NodeId, NodeRevisions>,
}

impl SnapshotValue for NodeRevisions {
    fn heap_bytes(&self) -> usize {
        0
    }
}

impl Default for RevisionSnapshot {
    fn default() -> Self {
        Self {
            epoch: Uuid::new_v4(),
            sequence: 0,
            by_node: SnapshotMap::new(HashMap::new()),
        }
    }
}

type RevisionLease = Result<
    Vec<golden_io::RetainedPayload<SnapshotLayer<NodeId, NodeRevisions>>>,
    golden_io::PayloadCapacityError<Arc<SnapshotLayer<NodeId, NodeRevisions>>>,
>;

impl RevisionSnapshot {
    pub(crate) fn try_lease(&self, pool: &golden_io::PayloadPool) -> RevisionLease {
        self.by_node.try_lease(pool)
    }
    pub(crate) fn retained_heap_bytes(&self) -> usize {
        self.by_node.retained_bytes()
    }
    pub(crate) fn lease_storage_bytes(&self) -> usize {
        self.by_node.lease_storage_bytes()
    }
    pub(crate) fn from_local(
        local: LocalRevisionSnapshot,
        nodes: &crate::process_ctx::snapshot_map::SnapshotMap<NodeId, Box<crate::process_ctx::ProcessTreeNodeSnapshot>>,
    ) -> Self {
        let mut children: HashMap<_, usize> = nodes.keys().map(|id| (*id, 0)).collect();
        for node in nodes.values() {
            if let Some(count) = node.parent.and_then(|parent| children.get_mut(&parent)) {
                *count += 1;
            }
        }
        let mut ready: Vec<_> = children
            .iter()
            .filter_map(|(id, count)| (*count == 0).then_some(*id))
            .collect();
        let mut revisions = local.by_node;
        let mut processed = 0;
        while let Some(id) = ready.pop() {
            processed += 1;
            let Some(parent) = nodes
                .get(&id)
                .and_then(|node| node.parent)
                .filter(|parent| nodes.contains_key(parent))
            else {
                continue;
            };
            let revision = revisions.get(&id).copied().unwrap_or_default();
            let parent_revision = revisions.entry(parent).or_default();
            parent_revision.authored = parent_revision.authored.max(revision.authored);
            parent_revision.structure = parent_revision.structure.max(revision.structure);
            let count = children.get_mut(&parent).expect("indexed parent");
            *count -= 1;
            if *count == 0 {
                ready.push(parent);
            }
        }
        if processed != nodes.len() {
            // A malformed cycle cannot receive a reusable identity. Normal engine
            // topology is validated; externally constructed views fail conservatively.
            return Self::default();
        }
        Self {
            epoch: local.epoch,
            sequence: local.sequence,
            by_node: SnapshotMap::new(revisions),
        }
    }

    pub(crate) fn revision(&self, node: NodeId) -> SubtreeRevision {
        SubtreeRevision {
            epoch: self.epoch,
            sequence: self.by_node.get(&node).map_or(0, |revision| revision.authored),
        }
    }

    pub(crate) fn structure_revision(&self, node: NodeId) -> SubtreeStructureRevision {
        SubtreeStructureRevision {
            node,
            revision: SubtreeRevision {
                epoch: self.epoch,
                sequence: self.by_node.get(&node).map_or(0, |revision| revision.structure),
            },
        }
    }
}

pub(super) struct TreeRevisions {
    epoch: Uuid,
    by_node: HashMap<NodeId, NodeRevisions>,
    sequence: u64,
}

impl Default for TreeRevisions {
    fn default() -> Self {
        Self {
            epoch: Uuid::new_v4(),
            by_node: HashMap::new(),
            sequence: 0,
        }
    }
}

impl TreeRevisions {
    pub(super) fn version(&self) -> (Uuid, u64) {
        (self.epoch, self.sequence)
    }

    pub(super) fn same_epoch(&self, captured: SubtreeRevision) -> bool {
        self.epoch == captured.epoch
    }

    /// Snapshot subtree stamps are upper bounds on the node's local revision.
    /// Every subsequent engine write receives a globally greater sequence,
    /// so this validates a node-local read without folding the live graph.
    pub(super) fn unchanged_since(&self, node: NodeId, captured: SubtreeRevision) -> bool {
        self.same_epoch(captured)
            && self.by_node.get(&node).map_or(0, |revision| revision.authored) <= captured.sequence
    }

    /// Fold only live guarded subtrees, sharing overlap within one precondition fence.
    /// The caller must discard the fold before any following edit can mutate the tree.
    pub(super) fn subtree_matches<T: Node>(
        &self,
        nodes: &NodeStore<T>,
        root: NodeId,
        captured: SubtreeRevision,
        folded: &mut HashMap<NodeId, u64>,
    ) -> bool {
        if !self.same_epoch(captured) {
            return false;
        }
        let mut stack = vec![(root, false)];
        while let Some((id, finish)) = stack.pop() {
            if folded.contains_key(&id) {
                continue;
            }
            let Some(node) = nodes.get(id) else {
                return false;
            };
            let mut revision = self.by_node.get(&id).map_or(0, |revision| revision.authored);
            if !finish {
                stack.push((id, true));
            }
            // Live engine topology is validated. Unlike snapshot construction,
            // this reads identities and revision counters without cloning payloads.
            let mut child = node.node_data().first_child;
            while let Some(child_id) = child {
                let Some(child_node) = nodes.get(child_id) else {
                    return false;
                };
                if finish {
                    let Some(child_revision) = folded.get(&child_id) else {
                        return false;
                    };
                    revision = revision.max(*child_revision);
                } else {
                    stack.push((child_id, false));
                }
                child = child_node.node_data().next_sibling;
            }
            if finish {
                folded.insert(id, revision);
            }
        }
        folded.get(&root) == Some(&captured.sequence)
    }

    pub(super) fn structure_unchanged_since(&self, node: NodeId, captured: SubtreeStructureRevision) -> bool {
        captured.node == node
            && self.same_epoch(captured.revision)
            && self.by_node.get(&node).map_or(0, |revision| revision.structure) <= captured.revision.sequence
    }

    pub(super) fn next_change(&mut self) -> u64 {
        self.sequence = self.sequence.checked_add(1).expect("tree revision sequence exhausted");
        self.sequence
    }

    pub(super) fn mark(&mut self, node: NodeId, sequence: u64) {
        self.by_node.entry(node).or_default().authored = sequence;
    }

    pub(super) fn mark_structure(&mut self, node: NodeId, sequence: u64) {
        self.by_node.insert(
            node,
            NodeRevisions {
                authored: sequence,
                structure: sequence,
            },
        );
    }

    pub(super) fn remove(&mut self, node: NodeId) {
        self.by_node.remove(&node);
    }

    pub(super) fn snapshot(&self) -> LocalRevisionSnapshot {
        LocalRevisionSnapshot {
            epoch: self.epoch,
            sequence: self.sequence,
            by_node: self.by_node.clone(),
        }
    }
}

impl<T: crate::node::Node> super::Engine<T> {
    pub(super) fn mark_tree_changed(&mut self, node: NodeId) {
        let sequence = self.tree_revisions.next_change();
        // An opaque/direct write may alter lookup structure without semantic events.
        self.tree_revisions.mark_structure(node, sequence);
        self.clear_tick_tree_snapshot();
    }
}
