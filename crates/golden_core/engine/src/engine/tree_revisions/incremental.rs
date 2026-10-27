use std::{cmp::Reverse, collections::HashSet};

use super::*;
use crate::process_ctx::ProcessTreeNodeSnapshot;

type Nodes = SnapshotMap<NodeId, Box<ProcessTreeNodeSnapshot>>;

impl RevisionSnapshot {
    pub(crate) fn refreshed(
        &self,
        local: LocalRevisionSnapshot,
        nodes: &Nodes,
        topology_roots: &HashSet<NodeId>,
    ) -> Self {
        self.try_refresh(&local, nodes, topology_roots)
            .unwrap_or_else(|| Self::from_local(local, nodes))
    }

    fn try_refresh(
        &self,
        local: &LocalRevisionSnapshot,
        nodes: &Nodes,
        topology_roots: &HashSet<NodeId>,
    ) -> Option<Self> {
        if self.epoch != local.epoch || local.sequence < self.sequence {
            return None;
        }
        let budget = (nodes.len() / 4).max(32);
        if !self.valid_topology(local, nodes, topology_roots, budget) {
            return None;
        }
        // Scan compact local counters, never unrelated node records or parent links.
        // Descending stamps let lower writes stop at already updated ancestors.
        let mut changed: Vec<_> = local
            .by_node
            .iter()
            .filter(|(_, revision)| revision.authored > self.sequence || revision.structure > self.sequence)
            .map(|(id, revision)| (*id, *revision))
            .collect();
        if changed.len() > budget {
            return None;
        }
        changed.sort_unstable_by_key(|(_, revision)| Reverse(revision.authored.max(revision.structure)));
        let mut updates: HashMap<NodeId, NodeRevisions> = HashMap::new();
        for (id, revision) in changed {
            let mut cursor = Some(id);
            while let Some(id) = cursor {
                let node = nodes.get(&id)?;
                let before = updates
                    .get(&id)
                    .or_else(|| self.by_node.get(&id))
                    .copied()
                    .unwrap_or_default();
                let next = NodeRevisions {
                    authored: before.authored.max(revision.authored),
                    structure: before.structure.max(revision.structure),
                };
                if before == next {
                    break;
                }
                updates.insert(id, next);
                if updates.len() > budget {
                    return None;
                }
                cursor = node.parent;
            }
        }
        Some(Self {
            epoch: local.epoch,
            sequence: local.sequence,
            by_node: self
                .by_node
                .changed(updates.into_iter().map(|(id, value)| (id, Some(value))).collect()),
        })
    }

    fn valid_topology(
        &self,
        local: &LocalRevisionSnapshot,
        nodes: &Nodes,
        roots: &HashSet<NodeId>,
        budget: usize,
    ) -> bool {
        let mut checked = HashSet::new();
        for root in roots {
            // Structural edits stamp both affected parents after prior descendants.
            // Raw pointer edits can lower a subtree maximum and require full folding.
            if local
                .by_node
                .get(root)
                .is_none_or(|revision| revision.structure <= self.sequence)
            {
                return false;
            }
            let mut path = HashSet::new();
            let mut cursor = Some(*root);
            while let Some(id) = cursor {
                if checked.contains(&id) {
                    break;
                }
                if !path.insert(id) || checked.len() + path.len() > budget {
                    return false;
                }
                let Some(node) = nodes.get(&id) else {
                    return false;
                };
                cursor = node.parent;
            }
            checked.extend(path);
        }
        true
    }
}
