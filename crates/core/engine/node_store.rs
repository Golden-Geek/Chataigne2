use slotmap::{Key, KeyData, SlotMap, new_key_type};
use crate::node::{Node, NodeId};

new_key_type! {
    struct NodeKey;
}

#[derive(Default)]
pub struct NodeStore<T: Node> {
    inner: SlotMap<NodeKey, T>,
}

impl<T: Node> NodeStore<T> {
    pub fn new() -> Self {
        Self { inner: SlotMap::with_key() }
    }

    pub fn insert(&mut self, mut node: T) -> NodeId {
        node.node_data_mut().id = NodeId(0);
        let key = self.inner.insert(node);
        let id = Self::id_from_key(key);
        if let Some(inserted) = self.inner.get_mut(key) {
            inserted.node_data_mut().id = id;
        }
        id
    }

    pub fn get(&self, id: NodeId) -> Option<&T> {
        self.inner.get(Self::key_from_id(id))
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        self.inner.get_mut(Self::key_from_id(id))
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.inner.values()
    }

    pub fn keys(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.inner.keys().map(Self::id_from_key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.inner.iter().map(|(key, node)| (Self::id_from_key(key), node))
    }

    fn id_from_key(key: NodeKey) -> NodeId {
        NodeId(key.data().as_ffi())
    }

    fn key_from_id(id: NodeId) -> NodeKey {
        NodeKey::from(KeyData::from_ffi(id.0))
    }
}
