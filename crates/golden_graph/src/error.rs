use crate::{GraphDiagnostic, GraphEdgeId, GraphNodeId, PortRef};

/// Structural graph edit failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GraphEditError {
    #[error("node `{0}` is already present")]
    DuplicateNode(GraphNodeId),
    #[error("node `{0}` is not present")]
    MissingNode(GraphNodeId),
    #[error("edge `{0}` is already present")]
    DuplicateEdge(GraphEdgeId),
    #[error("edge `{0}` is not present")]
    MissingEdge(GraphEdgeId),
    #[error("port `{0:?}` is not declared by its node")]
    MissingPort(PortRef),
    #[error("port `{port:?}` has the wrong direction; expected {expected}")]
    WrongPortDirection { port: PortRef, expected: &'static str },
    #[error("input `{0:?}` already has a connection")]
    InputAlreadyConnected(PortRef),
    #[error("edge endpoints `{from:?}` -> `{to:?}` are already connected")]
    DuplicateConnection { from: PortRef, to: PortRef },
    #[error("presentation references missing node `{0}`")]
    PresentationForMissingNode(GraphNodeId),
    #[error("domain rejected connection: {0:?}")]
    DomainConnection(GraphDiagnostic),
}

/// Atomic transaction failure. The document is restored before this is returned.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GraphTransactionError {
    #[error("transaction base revision {expected} does not match document revision {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error(transparent)]
    Edit(#[from] GraphEditError),
    #[error("domain validation rejected the graph")]
    Validation(Vec<GraphDiagnostic>),
}

/// Generic graph persistence envelope failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GraphPersistenceError {
    #[error("unsupported graph envelope schema {found}; maximum supported is {supported}")]
    UnsupportedEnvelopeSchema { found: u32, supported: u32 },
    #[error("graph domain `{found}` does not match expected domain `{expected}`")]
    DomainMismatch { expected: String, found: String },
    #[error("unsupported `{domain}` schema {found}; maximum supported is {supported}")]
    UnsupportedDomainSchema { domain: String, found: u32, supported: u32 },
    #[error(transparent)]
    InvalidDocument(#[from] GraphEditError),
}
