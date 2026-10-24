use crate::{
    node::{NodeId, NodeUuid},
    process_ctx::{ProcessTreeSnapshot, SubtreeRevision, SubtreeStructureRevision},
};

/// A read of one node's identity, effective enabled flag and selected revision.
/// Node-local, whole-subtree and structural reads are explicit capture choices.
#[derive(Clone, Copy, Debug)]
pub struct NodeReadGuard {
    pub(crate) node: NodeId,
    pub(crate) uuid: NodeUuid,
    pub(crate) enabled: bool,
    pub(crate) revision: ReadRevision,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ReadRevision {
    Authored(SubtreeRevision),
    Subtree(SubtreeRevision),
    Structure(SubtreeStructureRevision),
    Identity(SubtreeRevision),
}

impl NodeReadGuard {
    /// Captures authored state throughout this subtree, including changes whose
    /// callbacks have not run. Application shares the engine's ordinary snapshot;
    /// unlike node-local guards, a cold check may require rebuilding that view.
    /// Ancestors outside the subtree still require their own identity guards.
    pub fn capture_subtree(snapshot: &ProcessTreeSnapshot, node: NodeId) -> Option<Self> {
        let mut guard = Self::capture(snapshot, node)?;
        guard.revision = ReadRevision::Subtree(snapshot.subtree_revision(node)?);
        Some(guard)
    }

    /// Captures a node-local read from this snapshot's engine lifetime.
    pub fn capture(snapshot: &ProcessTreeSnapshot, node: NodeId) -> Option<Self> {
        let state = snapshot.node(node)?;
        Some(Self {
            node,
            uuid: state.uuid,
            enabled: state.enabled,
            revision: ReadRevision::Authored(snapshot.subtree_revision(node)?),
        })
    }

    /// Captures only engine epoch, node UUID and effective enabled state.
    /// Use this when labels, warnings and other authored metadata are not inputs.
    pub fn capture_identity(snapshot: &ProcessTreeSnapshot, node: NodeId) -> Option<Self> {
        let mut guard = Self::capture(snapshot, node)?;
        guard.revision = ReadRevision::Identity(snapshot.subtree_revision(node)?);
        Some(guard)
    }

    /// Captures node-local topology, named-selector metadata and identity while
    /// allowing scalar values, constraints and presentation to change. Capture
    /// each ancestor too when a read depends on a path through the tree.
    pub fn capture_structure(snapshot: &ProcessTreeSnapshot, node: NodeId) -> Option<Self> {
        let mut guard = Self::capture(snapshot, node)?;
        guard.revision = ReadRevision::Structure(snapshot.subtree_structure_revision(node)?);
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
