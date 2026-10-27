use super::*;

type Aliases = HashMap<Box<str>, NodeId>;
type ChildMaps = (
    HashMap<NodeId, Box<[NodeId]>>,
    HashMap<NodeId, Aliases>,
    HashMap<NodeId, Aliases>,
);

impl ProcessTreeSnapshot {
    pub(super) fn build_child_indexes(
        nodes: &SnapshotMap<NodeId, Box<ProcessTreeNodeSnapshot>>,
    ) -> ProcessTreeChildIndexes {
        let parents = nodes.iter().filter_map(|(id, node)| node.first_child.map(|_| *id));
        let (orders, declarations, keys) = collect(nodes, parents);
        ProcessTreeChildIndexes {
            ids_by_parent: SnapshotMap::new(orders),
            ids_by_decl_id: SnapshotMap::new(declarations),
            ids_by_child_key: SnapshotMap::new(keys),
        }
    }

    pub(super) fn refreshed_child_indexes(
        &self,
        nodes: &SnapshotMap<NodeId, Box<ProcessTreeNodeSnapshot>>,
        parents: &HashSet<NodeId>,
    ) -> ProcessTreeChildIndexes {
        let (mut orders, mut declarations, mut keys) = collect(nodes, parents.iter().copied());
        ProcessTreeChildIndexes {
            ids_by_parent: self
                .child_indexes
                .ids_by_parent
                .changed(parents.iter().map(|id| (*id, orders.remove(id))).collect()),
            ids_by_decl_id: self
                .child_indexes
                .ids_by_decl_id
                .changed(parents.iter().map(|id| (*id, declarations.remove(id))).collect()),
            ids_by_child_key: self
                .child_indexes
                .ids_by_child_key
                .changed(parents.iter().map(|id| (*id, keys.remove(id))).collect()),
        }
    }
}

fn collect(
    nodes: &SnapshotMap<NodeId, Box<ProcessTreeNodeSnapshot>>,
    parent_ids: impl Iterator<Item = NodeId>,
) -> ChildMaps {
    // Most folders are small; index only wide parents to avoid one map and
    // cloned selector strings per folder on every full-tree snapshot.
    const DECL_INDEX_THRESHOLD: usize = 16;
    let mut child_ids_by_parent = HashMap::new();
    let mut child_ids_by_decl_id = HashMap::new();
    let mut child_ids_by_key = HashMap::new();

    for parent in parent_ids {
        if nodes.get(&parent).is_none_or(|node| node.first_child.is_none()) {
            continue;
        }
        let mut child_ids = Vec::new();
        let mut child = nodes.get(&parent).and_then(|node| node.first_child);
        // Most declared folders have only a few children; reserve a hash set for
        // long sibling chains while still detecting malformed short cycles.
        let mut short_chain = [NodeId(0); 8];
        let mut short_len = 0;
        let mut visited = None::<HashSet<NodeId>>;
        while let Some(child_id) = child {
            if let Some(visited) = visited.as_mut() {
                if !visited.insert(child_id) {
                    break;
                }
            } else if short_chain[..short_len].contains(&child_id) {
                break;
            } else if short_len < short_chain.len() {
                short_chain[short_len] = child_id;
                short_len += 1;
            } else {
                let mut long_chain = HashSet::with_capacity(short_chain.len() * 2);
                long_chain.extend(short_chain);
                long_chain.insert(child_id);
                visited = Some(long_chain);
            }
            child_ids.push(child_id);
            child = nodes.get(&child_id).and_then(|node| node.next_sibling);
        }

        if child_ids.len() > DECL_INDEX_THRESHOLD {
            let mut decl_ids = HashMap::with_capacity(child_ids.len());
            let mut keys = HashMap::with_capacity(child_ids.len());
            for child_id in &child_ids {
                let Some(child) = nodes.get(child_id) else {
                    break;
                };
                // Match the ordered scan across all aliases: an earlier label also
                // wins over a later declaration id. Do not allocate duplicate aliases.
                for key in [&child.decl_id, &child.short_name, &child.label] {
                    if !keys.contains_key(key.as_str()) {
                        keys.insert(key.clone().into_boxed_str(), *child_id);
                    }
                }
                decl_ids
                    .entry(child.decl_id.clone().into_boxed_str())
                    .or_insert(*child_id);
                if let Some(short_decl_id) = child.decl_id.rsplit('/').next()
                    && short_decl_id != child.decl_id
                {
                    decl_ids.entry(short_decl_id.into()).or_insert(*child_id);
                }
            }
            child_ids_by_decl_id.insert(parent, decl_ids);
            child_ids_by_key.insert(parent, keys);
        }
        child_ids_by_parent.insert(parent, child_ids.into_boxed_slice());
    }

    (child_ids_by_parent, child_ids_by_decl_id, child_ids_by_key)
}
