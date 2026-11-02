use super::{Node, NodeId};

/// Borrowed live document access for side-effect-free user-item preparation. Unlike
/// intrinsic decode factories, a live factory may inspect selected document defaults.
/// Callers control the bounded traversal; no full process-tree snapshot is built.
pub struct UserItemCreationContext<'a> {
    root: NodeId,
    lookup: &'a dyn Fn(NodeId) -> Option<&'a dyn Node>,
}

impl<'a> UserItemCreationContext<'a> {
    /// Borrows one live document reader for the duration of detached preparation.
    pub fn new(root: NodeId, lookup: &'a dyn Fn(NodeId) -> Option<&'a dyn Node>) -> Self {
        Self { root, lookup }
    }

    /// Root of the current document.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Reads an explicitly addressed live node without creating a tree snapshot.
    pub fn node(&self, id: NodeId) -> Option<&'a dyn Node> {
        (self.lookup)(id)
    }
}
