use crate::{
    node::{NodeId, NodeUuid},
    process_ctx::{ProcessTreeSnapshot, SubtreeRevision},
};

/// A read of one node's identity, effective enabled flag and optionally its
/// authored state. Descendant contents require their own guards.
#[derive(Clone, Copy, Debug)]
pub struct NodeReadGuard {
    pub(crate) node: NodeId,
    pub(crate) uuid: NodeUuid,
    pub(crate) enabled: bool,
    pub(crate) revision: SubtreeRevision,
    pub(crate) authored: bool,
}

impl NodeReadGuard {
    /// Captures a node-local read from this snapshot's engine lifetime.
    pub fn capture(snapshot: &ProcessTreeSnapshot, node: NodeId) -> Option<Self> {
        let state = snapshot.node(node)?;
        Some(Self {
            node,
            uuid: state.uuid,
            enabled: state.enabled,
            revision: snapshot.subtree_revision(node)?,
            authored: true,
        })
    }

    /// Captures only engine epoch, node UUID and effective enabled state.
    /// Use this when labels, warnings and other authored metadata are not inputs.
    pub fn capture_identity(snapshot: &ProcessTreeSnapshot, node: NodeId) -> Option<Self> {
        let mut guard = Self::capture(snapshot, node)?;
        guard.authored = false;
        Some(guard)
    }
}

/// Bounded read fences checked at their position in the edit queue. A rejected
/// fence discards the remaining drain suffix under ordinary prefix semantics;
/// it does not roll back preceding edits or lock a native producer.
pub struct EditPreconditions {
    pub(crate) nodes: Vec<NodeReadGuard>,
    pub(crate) native: Vec<golden_io::RevisionGuard>,
}

impl EditPreconditions {
    /// Admission is limited to 32 node reads and eight native publications.
    pub fn new(nodes: Vec<NodeReadGuard>, native: Vec<golden_io::RevisionGuard>) -> Option<Self> {
        (nodes.capacity() <= 32 && native.capacity() <= 8).then_some(Self { nodes, native })
    }
}
