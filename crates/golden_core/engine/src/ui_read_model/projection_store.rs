use std::collections::HashMap;
use std::sync::Arc;

use crate::node::NodeId;
use crate::ui_sync::UiNodeDto;

const PROJECTION_SHARD_COUNT: usize = 256;
type NodeShard = Arc<HashMap<NodeId, Arc<UiNodeDto>>>;
type ParentShard = Arc<HashMap<NodeId, NodeId>>;

/// Fixed-shard copy-on-write node storage.
///
/// Capturing the store clones a constant number of `Arc`s. A later mutation clones only the
/// affected hash shard and node value, so an immutable capture never requires an O(N) copy while
/// the projection lock is held.
#[derive(Clone)]
pub(super) struct NodeStore {
    shards: Box<[NodeShard]>,
}

impl NodeStore {
    pub(super) fn from_nodes(nodes: &[UiNodeDto]) -> Self {
        let mut shards = empty_shards();
        for node in nodes {
            let shard = shard_index(node.node_id);
            shards[shard].insert(node.node_id, Arc::new(node.clone()));
        }
        Self {
            shards: shards.into_iter().map(Arc::new).collect(),
        }
    }

    pub(super) fn get(&self, node: &NodeId) -> Option<&UiNodeDto> {
        self.shards[shard_index(*node)].get(node).map(Arc::as_ref)
    }

    pub(super) fn get_mut(&mut self, node: &NodeId) -> Option<&mut UiNodeDto> {
        Arc::make_mut(&mut self.shards[shard_index(*node)])
            .get_mut(node)
            .map(Arc::make_mut)
    }

    pub(super) fn insert(&mut self, node: UiNodeDto) {
        let shard = Arc::make_mut(&mut self.shards[shard_index(node.node_id)]);
        shard.insert(node.node_id, Arc::new(node));
    }

    pub(super) fn remove(&mut self, node: &NodeId) -> Option<Arc<UiNodeDto>> {
        Arc::make_mut(&mut self.shards[shard_index(*node)]).remove(node)
    }

    pub(super) fn values(&self) -> impl Iterator<Item = &UiNodeDto> {
        self.shards.iter().flat_map(|shard| shard.values().map(Arc::as_ref))
    }

    #[cfg(test)]
    pub(super) fn shared_shards_with(&self, other: &Self) -> usize {
        self.shards
            .iter()
            .zip(other.shards.iter())
            .filter(|(left, right)| Arc::ptr_eq(left, right))
            .count()
    }
}

/// Fixed-shard copy-on-write parent index captured alongside [`NodeStore`].
#[derive(Clone)]
pub(super) struct ParentStore {
    shards: Box<[ParentShard]>,
    len: usize,
}

impl ParentStore {
    pub(super) fn from_nodes<'a>(nodes: impl IntoIterator<Item = &'a UiNodeDto>) -> Self {
        let mut shards = empty_shards();
        let mut len = 0;
        for node in nodes {
            for child in &node.children {
                if shards[shard_index(*child)].insert(*child, node.node_id).is_none() {
                    len += 1;
                }
            }
        }
        Self {
            shards: shards.into_iter().map(Arc::new).collect(),
            len,
        }
    }

    pub(super) fn get(&self, node: &NodeId) -> Option<&NodeId> {
        self.shards[shard_index(*node)].get(node)
    }

    pub(super) fn insert(&mut self, node: NodeId, parent: NodeId) {
        if Arc::make_mut(&mut self.shards[shard_index(node)])
            .insert(node, parent)
            .is_none()
        {
            self.len += 1;
        }
    }

    pub(super) fn remove(&mut self, node: &NodeId) {
        if Arc::make_mut(&mut self.shards[shard_index(*node)])
            .remove(node)
            .is_some()
        {
            self.len -= 1;
        }
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }
}

fn empty_shards<V>() -> Vec<HashMap<NodeId, V>> {
    (0..PROJECTION_SHARD_COUNT).map(|_| HashMap::new()).collect()
}

fn shard_index(node: NodeId) -> usize {
    let folded = node.0 ^ (node.0 >> 32);
    folded as usize & (PROJECTION_SHARD_COUNT - 1)
}

#[cfg(test)]
pub(super) const fn projection_shard_count() -> usize {
    PROJECTION_SHARD_COUNT
}
