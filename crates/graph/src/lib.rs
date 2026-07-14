//! Typed, app-agnostic graph documents and transactional editing.
//!
//! Domain crates supply semantic payloads and validation. This crate owns stable
//! graph identities, topology, transactions, revisions, presentation state,
//! traversal, and the generic persistence envelope.

mod document;
mod domain;
mod error;
mod ids;
mod persistence;
mod presentation;
mod revision;
mod transaction;
mod traversal;

pub use document::{GraphDocument, GraphEdge, GraphNode, PortRef};
pub use domain::{DiagnosticSeverity, GraphDiagnostic, GraphDomain, PortDefinition, PortDirection, PortSchema};
pub use error::{GraphEditError, GraphPersistenceError, GraphTransactionError};
pub use ids::{GraphCommentId, GraphEdgeId, GraphGroupId, GraphId, GraphNodeId, GraphPortId, ViewportBookmarkId};
pub use persistence::{GraphDocumentData, GraphEnvelope};
pub use presentation::{GraphComment, GraphGroup, GraphPresentation, NodePresentation, ViewportBookmark};
pub use revision::{GraphChangeSet, GraphDelta, GraphRevision};
pub use transaction::{GraphCommit, GraphOperation, GraphTransaction};
pub use traversal::{stable_topological_order, strongly_connected_components};

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

#[cfg(test)]
mod tests;
