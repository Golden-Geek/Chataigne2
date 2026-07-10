use golden_model::Revision;
use serde::{Deserialize, Serialize};

use crate::{GraphDocument, GraphDomain, GraphEdgeId, GraphId, GraphNodeId, GraphPresentation, PortRef};

pub const GRAPH_ENVELOPE_VERSION: u32 = 1;

pub trait GraphProtocolAdapter<D: GraphDomain> {
    fn domain_id(&self) -> &str;
    fn domain_schema_version(&self) -> u32;
    fn encode_graph_data(&self, data: &D::GraphData) -> serde_json::Value;
    fn encode_node_data(&self, data: &D::NodeData) -> serde_json::Value;
    fn encode_edge_data(&self, data: &D::EdgeData) -> serde_json::Value;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNodeDto {
    pub id: GraphNodeId,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphEdgeDto {
    pub id: GraphEdgeId,
    pub from: PortRef,
    pub to: PortRef,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphDocumentDto {
    pub envelope_version: u32,
    pub domain_id: String,
    pub domain_schema_version: u32,
    pub graph_id: GraphId,
    pub revision: Revision,
    pub graph_data: serde_json::Value,
    pub nodes: Vec<GraphNodeDto>,
    pub edges: Vec<GraphEdgeDto>,
    pub presentation: GraphPresentation,
}

impl GraphDocumentDto {
    pub fn from_document<D: GraphDomain>(graph: &GraphDocument<D>, adapter: &impl GraphProtocolAdapter<D>) -> Self {
        Self {
            envelope_version: GRAPH_ENVELOPE_VERSION,
            domain_id: adapter.domain_id().to_owned(),
            domain_schema_version: adapter.domain_schema_version(),
            graph_id: graph.id(),
            revision: graph.revision(),
            graph_data: adapter.encode_graph_data(graph.graph_data()),
            nodes: graph
                .nodes()
                .map(|node| GraphNodeDto {
                    id: node.id,
                    data: adapter.encode_node_data(&node.data),
                })
                .collect(),
            edges: graph
                .edges()
                .map(|edge| GraphEdgeDto {
                    id: edge.id,
                    from: edge.from,
                    to: edge.to,
                    data: adapter.encode_edge_data(&edge.data),
                })
                .collect(),
            presentation: graph.presentation().clone(),
        }
    }
}
