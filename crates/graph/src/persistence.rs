use indexmap::IndexMap;

use crate::{
    GraphDocument, GraphEdge, GraphEdgeId, GraphId, GraphNode, GraphNodeId, GraphPersistenceError, GraphPresentation,
    GraphRevision,
};

/// Serializable graph document body. Runtime topology indexes are rebuilt on load.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphDocumentData<G, N, E> {
    pub id: GraphId,
    pub revision: GraphRevision,
    pub data: G,
    pub nodes: IndexMap<GraphNodeId, GraphNode<N>>,
    pub edges: IndexMap<GraphEdgeId, GraphEdge<E>>,
    pub presentation: GraphPresentation,
}

impl<G, N, E> GraphDocumentData<G, N, E> {
    #[must_use]
    pub fn from_document(document: &GraphDocument<G, N, E>) -> Self
    where
        G: Clone,
        N: Clone,
        E: Clone,
    {
        Self {
            id: document.id,
            revision: document.revision,
            data: document.data.clone(),
            nodes: document.nodes.clone(),
            edges: document.edges.clone(),
            presentation: document.presentation.clone(),
        }
    }

    pub fn into_document(self) -> Result<GraphDocument<G, N, E>, GraphPersistenceError> {
        GraphDocument::from_parts(
            self.id,
            self.revision,
            self.data,
            self.nodes,
            self.edges,
            self.presentation,
        )
        .map_err(Into::into)
    }
}

/// Versioned generic persistence envelope around typed domain payloads.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphEnvelope<G, N, E> {
    pub schema_version: u32,
    pub domain_id: String,
    pub domain_schema_version: u32,
    pub document: GraphDocumentData<G, N, E>,
}

impl<G, N, E> GraphEnvelope<G, N, E> {
    pub const SCHEMA_VERSION: u32 = 1;

    #[must_use]
    pub fn from_document(
        domain_id: impl Into<String>,
        domain_schema_version: u32,
        document: &GraphDocument<G, N, E>,
    ) -> Self
    where
        G: Clone,
        N: Clone,
        E: Clone,
    {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            domain_id: domain_id.into(),
            domain_schema_version,
            document: GraphDocumentData::from_document(document),
        }
    }

    pub fn into_document(
        self,
        expected_domain: &str,
        maximum_domain_schema: u32,
    ) -> Result<GraphDocument<G, N, E>, GraphPersistenceError> {
        if self.schema_version > Self::SCHEMA_VERSION {
            return Err(GraphPersistenceError::UnsupportedEnvelopeSchema {
                found: self.schema_version,
                supported: Self::SCHEMA_VERSION,
            });
        }
        if self.domain_id != expected_domain {
            return Err(GraphPersistenceError::DomainMismatch {
                expected: expected_domain.to_string(),
                found: self.domain_id,
            });
        }
        if self.domain_schema_version > maximum_domain_schema {
            return Err(GraphPersistenceError::UnsupportedDomainSchema {
                domain: expected_domain.to_string(),
                found: self.domain_schema_version,
                supported: maximum_domain_schema,
            });
        }
        self.document.into_document()
    }
}
