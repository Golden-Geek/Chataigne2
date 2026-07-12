use crate::node::{Node, NodeId};
use slotmap::{Key, KeyData, SlotMap, new_key_type};

new_key_type! {
    struct NodeKey;
}

/// Storage backend for nodes keyed by stable [`NodeId`] values.
#[derive(Default)]
pub struct NodeStore<T: Node> {
    /// Slotmap storage keyed by internal node keys.
    inner: SlotMap<NodeKey, T>,
}

impl<T: Node> NodeStore<T> {
    /// Creates an empty node store.
    pub fn new() -> Self {
        Self {
            inner: SlotMap::with_key(),
        }
    }

    /// Inserts a node and returns its assigned id.
    pub fn insert(&mut self, mut node: T) -> NodeId {
        node.node_data_mut().id = NodeId(0);
        let key = self.inner.insert(node);
        let id = Self::id_from_key(key);
        if let Some(inserted) = self.inner.get_mut(key) {
            inserted.node_data_mut().id = id;
        }
        id
    }

    /// Gets an immutable node reference by id.
    pub fn get(&self, id: NodeId) -> Option<&T> {
        self.inner.get(Self::key_from_id(id))
    }

    /// Gets a mutable node reference by id.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        self.inner.get_mut(Self::key_from_id(id))
    }

    /// Removes and returns a node by id.
    pub fn remove(&mut self, id: NodeId) -> Option<T> {
        self.inner.remove(Self::key_from_id(id))
    }

    /// Detaches and returns a node by id while reserving its key for later reattachment.
    pub fn detach(&mut self, id: NodeId) -> Option<T> {
        self.inner.detach(Self::key_from_id(id))
    }

    /// Reattaches a previously detached node at the same id.
    ///
    /// # Panics
    /// Panics if `id` is not currently detached.
    pub fn reattach(&mut self, id: NodeId, mut node: T) {
        node.node_data_mut().id = id;
        self.inner.reattach(Self::key_from_id(id), node);
    }

    /// Returns `true` when an id exists in the store.
    pub fn contains(&self, id: NodeId) -> bool {
        self.inner.contains_key(Self::key_from_id(id))
    }

    /// Returns the number of nodes in the store.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` when the store contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterates over node values.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.inner.values()
    }

    /// Iterates over node ids.
    pub fn keys(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.inner.keys().map(Self::id_from_key)
    }

    /// Iterates over `(NodeId, &Node)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.inner.iter().map(|(key, node)| (Self::id_from_key(key), node))
    }

    /// Converts an internal slot key to the external stable [`NodeId`] representation.
    fn id_from_key(key: NodeKey) -> NodeId {
        NodeId(key.data().as_ffi())
    }

    /// Converts a stable external [`NodeId`] to its internal slot key representation.
    fn key_from_id(id: NodeId) -> NodeKey {
        NodeKey::from(KeyData::from_ffi(id.0))
    }
}
