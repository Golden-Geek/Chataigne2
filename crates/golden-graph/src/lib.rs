//! Typed, domain-neutral authored graph documents and atomic transactions.

mod document;
mod domain;
mod ids;
mod presentation;
mod protocol;
mod transaction;
mod traversal;

pub use document::{GraphDocument, GraphEdge, GraphNode, GraphTopologyError};
pub use domain::{DiagnosticSeverity, GraphDiagnostic, GraphDomain, PortDescriptor, PortDirection, PortRef};
pub use ids::{GraphEdgeId, GraphId, GraphNodeId, GraphPortId};
pub use presentation::{
    FiniteCoordinate, GeometryError, GraphComment, GraphGroup, GraphPresentation, NodePresentation, Point, Size,
    ViewportBookmark,
};
pub use protocol::{GRAPH_ENVELOPE_VERSION, GraphDocumentDto, GraphEdgeDto, GraphNodeDto, GraphProtocolAdapter};
pub use transaction::{
    GraphChange, GraphChangeSet, GraphCommit, GraphOperation, GraphTransaction, GraphTransactionError,
};
pub use traversal::{GraphCycle, stable_topological_order, strongly_connected_components};

#[cfg(test)]
mod tests;
