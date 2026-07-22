use std::collections::BTreeSet;

use crate::{GraphEdgeId, GraphGroupId, GraphNodeId};

/// Monotonic graph revision split by consumer-facing change plane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ts_rs::TS)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphRevision {
    pub sequence: u64,
    pub topology: u64,
    pub payload: u64,
    pub presentation: u64,
}

impl GraphRevision {
    pub(crate) fn advanced(self, changes: &GraphChangeSet) -> Self {
        if changes.is_empty() {
            return self;
        }

        Self {
            sequence: self.sequence.saturating_add(1),
            topology: self.topology.saturating_add(u64::from(changes.affects_topology())),
            payload: self.payload.saturating_add(u64::from(changes.affects_payload())),
            presentation: self
                .presentation
                .saturating_add(u64::from(changes.affects_presentation())),
        }
    }
}

/// Precise net change set produced by one atomic graph transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq, ts_rs::TS)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphChangeSet {
    pub inserted_nodes: BTreeSet<GraphNodeId>,
    pub removed_nodes: BTreeSet<GraphNodeId>,
    pub updated_nodes: BTreeSet<GraphNodeId>,
    pub inserted_edges: BTreeSet<GraphEdgeId>,
    pub removed_edges: BTreeSet<GraphEdgeId>,
    pub updated_edges: BTreeSet<GraphEdgeId>,
    pub presentation_nodes: BTreeSet<GraphNodeId>,
    pub presentation_groups: BTreeSet<GraphGroupId>,
    pub graph_data_changed: bool,
}

impl GraphChangeSet {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inserted_nodes.is_empty()
            && self.removed_nodes.is_empty()
            && self.updated_nodes.is_empty()
            && self.inserted_edges.is_empty()
            && self.removed_edges.is_empty()
            && self.updated_edges.is_empty()
            && self.presentation_nodes.is_empty()
            && self.presentation_groups.is_empty()
            && !self.graph_data_changed
    }

    #[must_use]
    pub fn affects_topology(&self) -> bool {
        !self.inserted_nodes.is_empty()
            || !self.removed_nodes.is_empty()
            || !self.inserted_edges.is_empty()
            || !self.removed_edges.is_empty()
    }

    #[must_use]
    pub fn affects_payload(&self) -> bool {
        self.graph_data_changed
            || !self.inserted_nodes.is_empty()
            || !self.removed_nodes.is_empty()
            || !self.updated_nodes.is_empty()
            || !self.inserted_edges.is_empty()
            || !self.removed_edges.is_empty()
            || !self.updated_edges.is_empty()
    }

    #[must_use]
    pub fn affects_presentation(&self) -> bool {
        !self.presentation_nodes.is_empty() || !self.presentation_groups.is_empty()
    }

    pub(crate) fn node_inserted(&mut self, node: GraphNodeId) {
        if self.removed_nodes.remove(&node) {
            self.updated_nodes.insert(node);
        } else {
            self.inserted_nodes.insert(node);
        }
    }

    pub(crate) fn node_removed(&mut self, node: GraphNodeId) -> bool {
        let removed_existing = !self.inserted_nodes.remove(&node);
        if removed_existing {
            self.removed_nodes.insert(node);
        } else {
            self.presentation_nodes.remove(&node);
        }
        self.updated_nodes.remove(&node);
        removed_existing
    }

    pub(crate) fn edge_inserted(&mut self, edge: GraphEdgeId) {
        if self.removed_edges.remove(&edge) {
            self.updated_edges.insert(edge);
        } else {
            self.inserted_edges.insert(edge);
        }
    }

    pub(crate) fn edge_removed(&mut self, edge: GraphEdgeId) {
        if !self.inserted_edges.remove(&edge) {
            self.removed_edges.insert(edge);
        }
        self.updated_edges.remove(&edge);
    }
}

/// Revision envelope for one coherent graph transaction delta.
#[derive(Clone, Debug, PartialEq, Eq, ts_rs::TS)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphDelta {
    pub from: GraphRevision,
    pub to: GraphRevision,
    pub changes: GraphChangeSet,
}
