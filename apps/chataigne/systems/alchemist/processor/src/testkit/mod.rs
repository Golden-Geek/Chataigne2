use indexmap::IndexMap;

use chataigne_alchemist::{
    AEdge, ALCHEMIST_SCHEMA_VERSION, ANodeId, ANodeInstance, AlchemistEdgeData, AlchemistGraphData,
    AlchemistGraphDocument, AlchemistGraphDomain, AlchemistGraphId, AlchemistNodeData, ExposedSurface, GraphMetadata,
    InputSocketRef, OutputSocketRef,
};
use golden_graph::{
    GraphDocumentData, GraphEdge, GraphEdgeId, GraphId, GraphNode, GraphPresentation, GraphRevision, NodePresentation,
    PortRef,
};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TestGraphError {
    #[error("node {0} is duplicated")]
    DuplicateNode(ANodeId),
    #[error("node {0} is missing")]
    MissingNode(ANodeId),
    #[error("input {0:?} is already connected")]
    InputAlreadyConnected(InputSocketRef),
}

/// Test-only semantic fixture. Runtime code consumes typed documents exclusively.
pub struct TestGraph {
    id: AlchemistGraphId,
    nodes: IndexMap<ANodeId, ANodeInstance>,
    edges: Vec<AEdge>,
}

impl TestGraph {
    pub fn new() -> Self {
        Self {
            id: AlchemistGraphId::new(),
            nodes: IndexMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: ANodeInstance) -> Result<ANodeId, TestGraphError> {
        if self.nodes.contains_key(&node.id) {
            return Err(TestGraphError::DuplicateNode(node.id));
        }
        let id = node.id;
        self.nodes.insert(id, node);
        Ok(id)
    }

    pub fn connect(&mut self, from: OutputSocketRef, to: InputSocketRef) -> Result<(), TestGraphError> {
        if !self.nodes.contains_key(&from.node) {
            return Err(TestGraphError::MissingNode(from.node));
        }
        if !self.nodes.contains_key(&to.node) {
            return Err(TestGraphError::MissingNode(to.node));
        }
        if self.edges.iter().any(|edge| edge.to == to) {
            return Err(TestGraphError::InputAlreadyConnected(to));
        }
        self.edges.push(AEdge { from, to });
        Ok(())
    }

    pub fn to_document(&self) -> Result<AlchemistGraphDocument, golden_graph::GraphPersistenceError> {
        let graph_id = GraphId::from_uuid(self.id.as_uuid());
        let mut nodes = IndexMap::with_capacity(self.nodes.len());
        let mut presentation = GraphPresentation::default();
        for node in self.nodes.values() {
            let id = AlchemistGraphDomain::node_id(node.id);
            nodes.insert(
                id,
                GraphNode {
                    id,
                    data: AlchemistNodeData::from_instance(node),
                },
            );
            presentation.nodes.insert(
                id,
                NodePresentation {
                    position: node.ui.position,
                    size: node.ui.size,
                    collapsed: node.ui.collapsed,
                },
            );
        }
        let edges = self
            .edges
            .iter()
            .map(|edge| {
                let id = GraphEdgeId::new();
                (
                    id,
                    GraphEdge {
                        id,
                        from: PortRef::new(
                            AlchemistGraphDomain::node_id(edge.from.node),
                            AlchemistGraphDomain::output_port_id(&edge.from.socket),
                        ),
                        to: PortRef::new(
                            AlchemistGraphDomain::node_id(edge.to.node),
                            AlchemistGraphDomain::input_port_id(&edge.to.socket),
                        ),
                        data: AlchemistEdgeData,
                    },
                )
            })
            .collect();
        GraphDocumentData {
            id: graph_id,
            revision: GraphRevision::default(),
            data: AlchemistGraphData {
                schema_version: ALCHEMIST_SCHEMA_VERSION,
                exposed: ExposedSurface::default(),
                metadata: GraphMetadata::default(),
                viewport_origin: [0.0, 0.0],
                viewport_zoom: 1.0,
            },
            nodes,
            edges,
            presentation,
        }
        .into_document()
    }
}
