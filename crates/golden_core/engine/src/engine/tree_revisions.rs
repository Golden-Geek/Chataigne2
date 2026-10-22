use std::{collections::HashMap, sync::Arc};

use uuid::Uuid;

use crate::node::NodeId;

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

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NodeRevisions {
    authored: u64,
    structure: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct RevisionSnapshot {
    epoch: Uuid,
    by_node: Arc<HashMap<NodeId, NodeRevisions>>,
}

impl Default for RevisionSnapshot {
    fn default() -> Self {
        Self {
            epoch: Uuid::new_v4(),
            by_node: Arc::default(),
        }
    }
}

type RevisionLease = Result<
    golden_io::RetainedPayload<HashMap<NodeId, NodeRevisions>>,
    golden_io::PayloadCapacityError<Arc<HashMap<NodeId, NodeRevisions>>>,
>;

impl RevisionSnapshot {
    pub(crate) fn try_lease(&self, pool: &golden_io::PayloadPool, weight: usize) -> RevisionLease {
        pool.try_retain_arc(self.by_node.clone(), weight)
    }
    pub(crate) fn retained_heap_bytes(&self) -> usize {
        crate::process_ctx::memory::arc_map(&self.by_node)
    }
    pub(crate) fn fold_subtrees(&mut self, nodes: &HashMap<NodeId, crate::process_ctx::ProcessTreeNodeSnapshot>) {
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
        // This is a new view, owned solely by snapshot construction; no mutation of
        // the engine's local-change index or any previously shared view occurs.
        let revisions = Arc::get_mut(&mut self.by_node).expect("new revision view");
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
            *self = Self::default();
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

    pub(super) fn snapshot(&self) -> RevisionSnapshot {
        RevisionSnapshot {
            epoch: self.epoch,
            by_node: Arc::new(self.by_node.clone()),
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
