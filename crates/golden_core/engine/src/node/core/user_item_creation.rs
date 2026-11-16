use super::{Node, NodeId};

/// Visits detached batch roots lazily; factories that do not inspect them pay no scan/allocation cost.
pub type PreparedChildrenReader<'a> = dyn Fn(NodeId, &mut dyn FnMut(&dyn Node)) + 'a;

/// Borrowed live document access for side-effect-free user-item preparation. Unlike
/// intrinsic decode factories, a live factory may inspect selected document defaults.
/// Callers control the bounded traversal; no full process-tree snapshot is built.
pub struct UserItemCreationContext<'a> {
    root: NodeId,
    lookup: &'a dyn Fn(NodeId) -> Option<&'a dyn Node>,
    prepared_children: Option<&'a PreparedChildrenReader<'a>>,
}

impl<'a> UserItemCreationContext<'a> {
    /// Borrows one live document reader for the duration of detached preparation.
    pub fn new(root: NodeId, lookup: &'a dyn Fn(NodeId) -> Option<&'a dyn Node>) -> Self {
        Self {
            root,
            lookup,
            prepared_children: None,
        }
    }

    /// Earlier detached roots in the same atomic creation batch. They are read-only
    /// candidates, not live nodes, and must participate in app-owned default allocation.
    pub fn with_prepared_children(mut self, children: &'a PreparedChildrenReader<'a>) -> Self {
        self.prepared_children = Some(children);
        self
    }

    /// Reads only previously prepared direct children of this destination.
    pub fn visit_prepared_children(&self, parent: NodeId, mut visitor: impl FnMut(&dyn Node)) {
        if let Some(reader) = self.prepared_children {
            reader(parent, &mut visitor);
        }
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
