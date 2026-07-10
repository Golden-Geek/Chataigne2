use std::collections::{BTreeMap, BTreeSet};

use golden_model::Revision;
use thiserror::Error;

use crate::{
    GraphDomain, GraphEdgeId, GraphId, GraphNodeId, GraphPortId, GraphPresentation, NodePresentation, PortDirection,
    PortRef,
};

#[derive(Clone, Debug)]
pub struct GraphNode<D: GraphDomain> {
    pub id: GraphNodeId,
    pub data: D::NodeData,
}

#[derive(Clone, Debug)]
pub struct GraphEdge<D: GraphDomain> {
    pub id: GraphEdgeId,
    pub from: PortRef,
    pub to: PortRef,
    pub data: D::EdgeData,
}

pub(crate) struct RemovedNode<D: GraphDomain> {
    pub node: GraphNode<D>,
    pub presentation: Option<NodePresentation>,
    pub edges: Vec<GraphEdge<D>>,
}

pub struct GraphDocument<D: GraphDomain> {
    id: GraphId,
    revision: Revision,
    domain: D,
    graph_data: D::GraphData,
    pub(crate) nodes: BTreeMap<GraphNodeId, GraphNode<D>>,
    pub(crate) edges: BTreeMap<GraphEdgeId, GraphEdge<D>>,
    pub(crate) outgoing: BTreeMap<GraphNodeId, BTreeSet<GraphEdgeId>>,
    pub(crate) incoming: BTreeMap<GraphNodeId, BTreeSet<GraphEdgeId>>,
    pub(crate) presentation: GraphPresentation,
}

impl<D: GraphDomain> GraphDocument<D> {
    pub fn new(id: GraphId, domain: D, graph_data: D::GraphData) -> Self {
        Self {
            id,
            revision: Revision::ZERO,
            domain,
            graph_data,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            outgoing: BTreeMap::new(),
            incoming: BTreeMap::new(),
            presentation: GraphPresentation::default(),
        }
    }

    pub const fn id(&self) -> GraphId {
        self.id
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }

    pub fn domain(&self) -> &D {
        &self.domain
    }

    pub fn graph_data(&self) -> &D::GraphData {
        &self.graph_data
    }

    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &GraphNode<D>> {
        self.nodes.values()
    }

    pub fn edges(&self) -> impl ExactSizeIterator<Item = &GraphEdge<D>> {
        self.edges.values()
    }

    pub fn node(&self, id: GraphNodeId) -> Option<&GraphNode<D>> {
        self.nodes.get(&id)
    }

    pub fn edge(&self, id: GraphEdgeId) -> Option<&GraphEdge<D>> {
        self.edges.get(&id)
    }

    pub fn presentation(&self) -> &GraphPresentation {
        &self.presentation
    }

    pub fn outgoing_edges(&self, node: GraphNodeId) -> impl Iterator<Item = &GraphEdge<D>> {
        self.outgoing
            .get(&node)
            .into_iter()
            .flatten()
            .filter_map(|edge| self.edges.get(edge))
    }

    pub fn incoming_edges(&self, node: GraphNodeId) -> impl Iterator<Item = &GraphEdge<D>> {
        self.incoming
            .get(&node)
            .into_iter()
            .flatten()
            .filter_map(|edge| self.edges.get(edge))
    }

    pub fn assert_invariants(&self) -> Result<(), GraphTopologyError> {
        for node in self.nodes.keys() {
            if !self.outgoing.contains_key(node) || !self.incoming.contains_key(node) {
                return Err(GraphTopologyError::MissingNodeIndex(*node));
            }
        }
        for (edge_id, edge) in &self.edges {
            if !self.nodes.contains_key(&edge.from.node) || !self.nodes.contains_key(&edge.to.node) {
                return Err(GraphTopologyError::DanglingEdge(*edge_id));
            }
            if !self.outgoing[&edge.from.node].contains(edge_id) || !self.incoming[&edge.to.node].contains(edge_id) {
                return Err(GraphTopologyError::MissingEdgeIndex(*edge_id));
            }
        }
        Ok(())
    }

    pub(crate) fn set_revision(&mut self, revision: Revision) {
        self.revision = revision;
    }

    pub(crate) fn insert_node_internal(&mut self, node: GraphNode<D>, presentation: Option<NodePresentation>) {
        let id = node.id;
        self.nodes.insert(id, node);
        self.outgoing.insert(id, BTreeSet::new());
        self.incoming.insert(id, BTreeSet::new());
        if let Some(presentation) = presentation {
            self.presentation.nodes.insert(id, presentation);
        }
    }

    pub(crate) fn remove_node_internal(&mut self, id: GraphNodeId) -> Option<RemovedNode<D>> {
        let incident = self
            .outgoing
            .get(&id)
            .into_iter()
            .flatten()
            .chain(self.incoming.get(&id).into_iter().flatten())
            .copied()
            .collect::<BTreeSet<_>>();
        let edges = incident
            .into_iter()
            .filter_map(|edge| self.remove_edge_internal(edge))
            .collect();
        self.outgoing.remove(&id);
        self.incoming.remove(&id);
        let presentation = self.presentation.nodes.remove(&id);
        self.nodes.remove(&id).map(|node| RemovedNode {
            node,
            presentation,
            edges,
        })
    }

    pub(crate) fn insert_edge_internal(&mut self, edge: GraphEdge<D>) {
        let id = edge.id;
        self.outgoing.entry(edge.from.node).or_default().insert(id);
        self.incoming.entry(edge.to.node).or_default().insert(id);
        self.edges.insert(id, edge);
    }

    pub(crate) fn remove_edge_internal(&mut self, id: GraphEdgeId) -> Option<GraphEdge<D>> {
        let edge = self.edges.remove(&id)?;
        if let Some(outgoing) = self.outgoing.get_mut(&edge.from.node) {
            outgoing.remove(&id);
        }
        if let Some(incoming) = self.incoming.get_mut(&edge.to.node) {
            incoming.remove(&id);
        }
        Some(edge)
    }

    pub(crate) fn has_port(&self, reference: PortRef, direction: PortDirection) -> bool {
        let Some(node) = self.nodes.get(&reference.node) else {
            return false;
        };
        self.domain
            .node_ports(&node.data, self)
            .iter()
            .any(|port| port.id == reference.port && port.direction == direction)
    }

    pub(crate) fn node_data_mut(&mut self, id: GraphNodeId) -> Option<&mut D::NodeData> {
        self.nodes.get_mut(&id).map(|node| &mut node.data)
    }

    pub(crate) fn presentation_mut(&mut self) -> &mut GraphPresentation {
        &mut self.presentation
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GraphTopologyError {
    #[error("node {0:?} has no topology indexes")]
    MissingNodeIndex(GraphNodeId),
    #[error("edge {0:?} references a missing node")]
    DanglingEdge(GraphEdgeId),
    #[error("edge {0:?} is missing from a topology index")]
    MissingEdgeIndex(GraphEdgeId),
    #[error("port {0:?} is unavailable")]
    MissingPort(GraphPortId),
}
