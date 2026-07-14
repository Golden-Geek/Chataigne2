use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;

use crate::{
    GraphEdgeId, GraphEditError, GraphId, GraphNodeId, GraphPortId, GraphPresentation, GraphRevision, NodePresentation,
};

/// Stable reference to one port on one graph node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, ts_rs::TS)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PortRef {
    pub node: GraphNodeId,
    pub port: GraphPortId,
}

impl PortRef {
    #[must_use]
    pub const fn new(node: GraphNodeId, port: GraphPortId) -> Self {
        Self { node, port }
    }
}

/// One typed node record in a generic graph document.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphNode<N> {
    pub id: GraphNodeId,
    pub data: N,
}

impl<N> GraphNode<N> {
    #[must_use]
    pub fn new(data: N) -> Self {
        Self {
            id: GraphNodeId::new(),
            data,
        }
    }
}

/// One typed directed edge record.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphEdge<E> {
    pub id: GraphEdgeId,
    pub from: PortRef,
    pub to: PortRef,
    pub data: E,
}

impl<E> GraphEdge<E> {
    #[must_use]
    pub fn new(from: PortRef, to: PortRef, data: E) -> Self {
        Self {
            id: GraphEdgeId::new(),
            from,
            to,
            data,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TopologyIndex {
    incoming_by_node: HashMap<GraphNodeId, BTreeSet<GraphEdgeId>>,
    outgoing_by_node: HashMap<GraphNodeId, BTreeSet<GraphEdgeId>>,
    incoming_by_port: HashMap<PortRef, GraphEdgeId>,
    connections: HashMap<(PortRef, PortRef), GraphEdgeId>,
}

impl TopologyIndex {
    fn add_node(&mut self, node: GraphNodeId) {
        self.incoming_by_node.entry(node).or_default();
        self.outgoing_by_node.entry(node).or_default();
    }

    fn remove_node(&mut self, node: GraphNodeId) {
        self.incoming_by_node.remove(&node);
        self.outgoing_by_node.remove(&node);
    }

    fn add_edge(&mut self, edge: &GraphEdge<impl Sized>) {
        self.outgoing_by_node.entry(edge.from.node).or_default().insert(edge.id);
        self.incoming_by_node.entry(edge.to.node).or_default().insert(edge.id);
        self.incoming_by_port.insert(edge.to, edge.id);
        self.connections.insert((edge.from, edge.to), edge.id);
    }

    fn remove_edge(&mut self, edge: &GraphEdge<impl Sized>) {
        if let Some(edges) = self.outgoing_by_node.get_mut(&edge.from.node) {
            edges.remove(&edge.id);
        }
        if let Some(edges) = self.incoming_by_node.get_mut(&edge.to.node) {
            edges.remove(&edge.id);
        }
        self.incoming_by_port.remove(&edge.to);
        self.connections.remove(&(edge.from, edge.to));
    }

    fn incident_edges(&self, node: GraphNodeId) -> BTreeSet<GraphEdgeId> {
        self.incoming_by_node
            .get(&node)
            .into_iter()
            .chain(self.outgoing_by_node.get(&node))
            .flatten()
            .copied()
            .collect()
    }
}

/// Indexed generic graph document. Domain meaning lives outside this type.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphDocument<G, N, E> {
    pub(crate) id: GraphId,
    pub(crate) revision: GraphRevision,
    pub(crate) data: G,
    pub(crate) nodes: IndexMap<GraphNodeId, GraphNode<N>>,
    pub(crate) edges: IndexMap<GraphEdgeId, GraphEdge<E>>,
    pub(crate) presentation: GraphPresentation,
    topology: TopologyIndex,
}

impl<G, N, E> GraphDocument<G, N, E> {
    #[must_use]
    pub fn new(data: G) -> Self {
        Self {
            id: GraphId::new(),
            revision: GraphRevision::default(),
            data,
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            presentation: GraphPresentation::default(),
            topology: TopologyIndex::default(),
        }
    }

    pub(crate) fn from_parts(
        id: GraphId,
        revision: GraphRevision,
        data: G,
        nodes: IndexMap<GraphNodeId, GraphNode<N>>,
        edges: IndexMap<GraphEdgeId, GraphEdge<E>>,
        presentation: GraphPresentation,
    ) -> Result<Self, GraphEditError> {
        let mut document = Self {
            id,
            revision,
            data,
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            presentation,
            topology: TopologyIndex::default(),
        };

        for (key, node) in nodes {
            if key != node.id || document.nodes.contains_key(&node.id) {
                return Err(GraphEditError::DuplicateNode(node.id));
            }
            document.insert_node_at(None, node)?;
        }
        for (key, edge) in edges {
            if key != edge.id || document.edges.contains_key(&edge.id) {
                return Err(GraphEditError::DuplicateEdge(edge.id));
            }
            document.insert_edge_at(None, edge)?;
        }
        if let Some(node) = document
            .presentation
            .nodes
            .keys()
            .find(|node| !document.nodes.contains_key(*node))
        {
            return Err(GraphEditError::PresentationForMissingNode(*node));
        }
        if let Some(node) = document
            .presentation
            .groups
            .values()
            .flat_map(|group| &group.nodes)
            .find(|node| !document.nodes.contains_key(*node))
        {
            return Err(GraphEditError::PresentationForMissingNode(*node));
        }
        Ok(document)
    }

    #[must_use]
    pub const fn id(&self) -> GraphId {
        self.id
    }

    #[must_use]
    pub const fn revision(&self) -> GraphRevision {
        self.revision
    }

    #[must_use]
    pub const fn data(&self) -> &G {
        &self.data
    }

    #[must_use]
    pub fn node(&self, node: GraphNodeId) -> Option<&GraphNode<N>> {
        self.nodes.get(&node)
    }

    #[must_use]
    pub fn edge(&self, edge: GraphEdgeId) -> Option<&GraphEdge<E>> {
        self.edges.get(&edge)
    }

    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &GraphNode<N>> {
        self.nodes.values()
    }

    pub fn edges(&self) -> impl ExactSizeIterator<Item = &GraphEdge<E>> {
        self.edges.values()
    }

    #[must_use]
    pub fn presentation(&self) -> &GraphPresentation {
        &self.presentation
    }

    #[must_use]
    pub fn incoming_edge(&self, port: PortRef) -> Option<GraphEdgeId> {
        self.topology.incoming_by_port.get(&port).copied()
    }

    pub fn incoming_edges(&self, node: GraphNodeId) -> impl Iterator<Item = GraphEdgeId> + '_ {
        self.topology.incoming_by_node.get(&node).into_iter().flatten().copied()
    }

    pub fn outgoing_edges(&self, node: GraphNodeId) -> impl Iterator<Item = GraphEdgeId> + '_ {
        self.topology.outgoing_by_node.get(&node).into_iter().flatten().copied()
    }

    pub(crate) fn has_connection(&self, from: PortRef, to: PortRef) -> bool {
        self.topology.connections.contains_key(&(from, to))
    }

    pub(crate) fn insert_node_at(&mut self, index: Option<usize>, node: GraphNode<N>) -> Result<(), GraphEditError> {
        if self.nodes.contains_key(&node.id) {
            return Err(GraphEditError::DuplicateNode(node.id));
        }
        self.topology.add_node(node.id);
        if let Some(index) = index {
            self.nodes.shift_insert(index.min(self.nodes.len()), node.id, node);
        } else {
            self.nodes.insert(node.id, node);
        }
        Ok(())
    }

    pub(crate) fn insert_edge_at(&mut self, index: Option<usize>, edge: GraphEdge<E>) -> Result<(), GraphEditError> {
        if self.edges.contains_key(&edge.id) {
            return Err(GraphEditError::DuplicateEdge(edge.id));
        }
        self.require_node(edge.from.node)?;
        self.require_node(edge.to.node)?;
        if self.incoming_edge(edge.to).is_some() {
            return Err(GraphEditError::InputAlreadyConnected(edge.to));
        }
        if self.has_connection(edge.from, edge.to) {
            return Err(GraphEditError::DuplicateConnection {
                from: edge.from,
                to: edge.to,
            });
        }
        self.topology.add_edge(&edge);
        if let Some(index) = index {
            self.edges.shift_insert(index.min(self.edges.len()), edge.id, edge);
        } else {
            self.edges.insert(edge.id, edge);
        }
        Ok(())
    }

    pub(crate) fn remove_edge(&mut self, edge: GraphEdgeId) -> Result<RemovedEdge<E>, GraphEditError> {
        let index = self
            .edges
            .get_index_of(&edge)
            .ok_or(GraphEditError::MissingEdge(edge))?;
        let removed = self.edges.shift_remove(&edge).expect("edge index was checked");
        self.topology.remove_edge(&removed);
        Ok(RemovedEdge { index, edge: removed })
    }

    pub(crate) fn remove_node(&mut self, node: GraphNodeId) -> Result<RemovedNode<N, E>, GraphEditError> {
        let node_index = self
            .nodes
            .get_index_of(&node)
            .ok_or(GraphEditError::MissingNode(node))?;
        let mut incident = self
            .topology
            .incident_edges(node)
            .into_iter()
            .map(|edge| (self.edges.get_index_of(&edge).expect("indexed edge must exist"), edge))
            .collect::<Vec<_>>();
        incident.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));

        let mut edges = Vec::with_capacity(incident.len());
        for (_, edge) in incident {
            edges.push(self.remove_edge(edge)?);
        }
        edges.sort_unstable_by_key(|edge| edge.index);

        let node = self.nodes.shift_remove(&node).expect("node index was checked");
        self.topology.remove_node(node.id);
        let presentation = self.presentation.nodes.get_index_of(&node.id).map(|index| {
            let (_, value) = self
                .presentation
                .nodes
                .shift_remove_index(index)
                .expect("presentation index was checked");
            (index, value)
        });
        let group_memberships = self
            .presentation
            .groups
            .iter_mut()
            .filter_map(|(group_id, group)| group.nodes.remove(&node.id).then_some(*group_id))
            .collect();

        Ok(RemovedNode {
            node_index,
            node,
            edges,
            presentation,
            group_memberships,
        })
    }

    pub(crate) fn restore_node(&mut self, removed: RemovedNode<N, E>) {
        let node_id = removed.node.id;
        self.insert_node_at(Some(removed.node_index), removed.node)
            .expect("rollback node must restore");
        if let Some((index, presentation)) = removed.presentation {
            self.presentation
                .nodes
                .shift_insert(index.min(self.presentation.nodes.len()), node_id, presentation);
        }
        for group_id in removed.group_memberships {
            self.presentation
                .groups
                .get_mut(&group_id)
                .expect("rollback group must exist")
                .nodes
                .insert(node_id);
        }
        for edge in removed.edges {
            self.insert_edge_at(Some(edge.index), edge.edge)
                .expect("rollback edge must restore");
        }
    }

    pub(crate) fn restore_edge(&mut self, removed: RemovedEdge<E>) {
        self.insert_edge_at(Some(removed.index), removed.edge)
            .expect("rollback edge must restore");
    }

    pub(crate) fn set_node_presentation(
        &mut self,
        node: GraphNodeId,
        presentation: Option<NodePresentation>,
    ) -> Result<Option<NodePresentation>, GraphEditError> {
        self.require_node(node)?;
        let old = self.presentation.nodes.shift_remove(&node);
        if let Some(presentation) = presentation {
            self.presentation.nodes.insert(node, presentation);
        }
        Ok(old)
    }

    pub(crate) fn require_node(&self, node: GraphNodeId) -> Result<(), GraphEditError> {
        self.nodes
            .contains_key(&node)
            .then_some(())
            .ok_or(GraphEditError::MissingNode(node))
    }
}

pub(crate) struct RemovedEdge<E> {
    pub index: usize,
    pub edge: GraphEdge<E>,
}

pub(crate) struct RemovedNode<N, E> {
    pub node_index: usize,
    pub node: GraphNode<N>,
    pub edges: Vec<RemovedEdge<E>>,
    pub presentation: Option<(usize, NodePresentation)>,
    pub group_memberships: Vec<crate::GraphGroupId>,
}
