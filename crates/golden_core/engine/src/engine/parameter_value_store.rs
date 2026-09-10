use std::collections::HashMap;
use std::sync::Arc;

use crate::node::NodeId;
use crate::parameter::ParamValue;

const PARAMETER_VALUE_SHARD_COUNT: usize = 256;
type ParameterValueShard = Arc<HashMap<NodeId, ParamValue>>;

/// Fixed-shard copy-on-write storage for current parameter values.
///
/// Runtime reads and ordinary writes use the owned shard in place. Capturing compiler input clones
/// only the fixed set of shard roots; subsequent parameter writes copy just the affected shard.
#[derive(Clone)]
pub(crate) struct ParameterValueStore {
    shards: Box<[ParameterValueShard]>,
    len: usize,
}

impl Default for ParameterValueStore {
    fn default() -> Self {
        Self {
            shards: (0..PARAMETER_VALUE_SHARD_COUNT)
                .map(|_| Arc::new(HashMap::new()))
                .collect(),
            len: 0,
        }
    }
}

impl ParameterValueStore {
    pub(crate) fn get(&self, node: &NodeId) -> Option<&ParamValue> {
        self.shards[shard_index(*node)].get(node)
    }

    pub(crate) fn insert(&mut self, node: NodeId, value: ParamValue) -> Option<ParamValue> {
        let previous = Arc::make_mut(&mut self.shards[shard_index(node)]).insert(node, value);
        if previous.is_none() {
            self.len += 1;
        }
        previous
    }

    pub(crate) fn remove(&mut self, node: &NodeId) -> Option<ParamValue> {
        let removed = Arc::make_mut(&mut self.shards[shard_index(*node)]).remove(node);
        if removed.is_some() {
            self.len -= 1;
        }
        removed
    }

    #[cfg(test)]
    pub(crate) fn contains_key(&self, node: &NodeId) -> bool {
        self.get(node).is_some()
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (NodeId, &ParamValue)> {
        self.shards
            .iter()
            .flat_map(|shard| shard.iter().map(|(node, value)| (*node, value)))
    }

    #[cfg(test)]
    pub(crate) fn shared_shards_with(&self, other: &Self) -> usize {
        self.shards
            .iter()
            .zip(other.shards.iter())
            .filter(|(left, right)| Arc::ptr_eq(left, right))
            .count()
    }

    #[cfg(test)]
    pub(crate) const fn shard_count() -> usize {
        PARAMETER_VALUE_SHARD_COUNT
    }
}

impl FromIterator<(NodeId, ParamValue)> for ParameterValueStore {
    fn from_iter<T: IntoIterator<Item = (NodeId, ParamValue)>>(values: T) -> Self {
        let mut store = Self::default();
        for (node, value) in values {
            store.insert(node, value);
        }
        store
    }
}

fn shard_index(node: NodeId) -> usize {
    let folded = node.0 ^ (node.0 >> 32);
    folded as usize & (PARAMETER_VALUE_SHARD_COUNT - 1)
}
