use super::*;

impl ProcessTreeSnapshot {
    /// Canonical engine refresh. UUID rebinding/collision and removals use full capture.
    pub(crate) fn refreshed(
        &self,
        updates: HashMap<NodeId, Box<ProcessTreeNodeSnapshot>>,
        revisions: LocalRevisionSnapshot,
    ) -> Option<(Self, usize)> {
        if !self.node_overrides.is_empty() {
            return None;
        }
        let mut uuid_changes = HashMap::new();
        let mut parents = HashSet::new();
        let mut topology_roots = HashSet::new();
        let mut replaced = 0;
        for (id, next) in &updates {
            if let Some(before) = self.nodes.get(id) {
                if before.uuid != next.uuid {
                    return None;
                }
                replaced += 1;
                if before.parent != next.parent || before.next_sibling != next.next_sibling {
                    topology_roots.extend(before.parent);
                    topology_roots.extend(next.parent);
                }
                if before.parent != next.parent
                    || before.next_sibling != next.next_sibling
                    || before.decl_id != next.decl_id
                    || before.short_name != next.short_name
                    || before.label != next.label
                {
                    parents.extend(before.parent);
                    parents.extend(next.parent);
                }
                if before.first_child != next.first_child || before.child_count != next.child_count {
                    parents.insert(*id);
                    topology_roots.insert(*id);
                }
            } else {
                if self.node_ids_by_uuid.get(&next.uuid).is_some() {
                    return None;
                }
                if uuid_changes.insert(next.uuid, Some(*id)).is_some() {
                    return None;
                }
                parents.extend(next.parent);
                topology_roots.insert(*id);
                topology_roots.extend(next.parent);
                if next.first_child.is_some() {
                    parents.insert(*id);
                }
            }
        }
        // Count deep record copies caused by bounded-layer compaction as well as
        // the engine's changed-record captures; do not report them as sparse work.
        let compacted = if !updates.is_empty() && self.nodes.needs_compaction(updates.len()) {
            self.nodes.len().saturating_sub(replaced)
        } else {
            0
        };
        let nodes = self
            .nodes
            .changed(updates.into_iter().map(|(id, node)| (id, Some(node))).collect());
        let child_indexes = self.refreshed_child_indexes(&nodes, &parents);
        let revisions = self.revisions.refreshed(revisions, &nodes, &topology_roots);
        let mut snapshot = Self {
            weights: memory::AllocationWeights::default(),
            storage_leases: std::sync::OnceLock::new(),
            revisions,
            root: self.root,
            nodes,
            node_overrides: self.node_overrides.clone(),
            node_ids_by_uuid: self.node_ids_by_uuid.changed(uuid_changes),
            child_indexes,
        };
        snapshot.weights = memory::weights(&snapshot);
        Some((snapshot, compacted))
    }
}
