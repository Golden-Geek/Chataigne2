use golden_graph::{
    GraphDocument, GraphEdge, GraphEdgeId, GraphId, GraphNode, GraphNodeId, GraphOperation, GraphPresentation,
    GraphTransaction, GraphTransactionError, NodePresentation, PortRef,
};
use golden_model::Revision;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AlchemistFormula, AlchemistGraphData, AlchemistGraphDomain, AlchemistNode, ConversionPolicy, FormulaDefaults,
    FormulaId, FormulaMetadata, FormulaProperty, FormulaSchema, FormulaSurface, ManagedRegionDefinition,
};

pub const FORMULA_FILE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormulaNodeV1 {
    pub id: GraphNodeId,
    pub data: AlchemistNode,
    pub presentation: Option<NodePresentation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormulaEdgeV1 {
    pub id: GraphEdgeId,
    pub from: PortRef,
    pub to: PortRef,
    pub policy: ConversionPolicy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormulaFileV1 {
    pub file_version: u32,
    pub id: FormulaId,
    pub schema: FormulaSchema,
    pub graph_id: GraphId,
    pub graph: AlchemistGraphData,
    pub nodes: Vec<FormulaNodeV1>,
    pub edges: Vec<FormulaEdgeV1>,
    pub presentation: GraphPresentation,
    pub properties: Vec<FormulaProperty>,
    pub surface: FormulaSurface,
    pub managed_regions: Vec<ManagedRegionDefinition>,
    pub metadata: FormulaMetadata,
    pub defaults: FormulaDefaults,
}

impl FormulaFileV1 {
    pub fn from_formula(formula: &AlchemistFormula) -> Self {
        Self {
            file_version: FORMULA_FILE_VERSION,
            id: formula.id,
            schema: formula.schema,
            graph_id: formula.graph.id(),
            graph: formula.graph.graph_data().clone(),
            nodes: formula
                .graph
                .nodes()
                .map(|node| FormulaNodeV1 {
                    id: node.id,
                    data: node.data.clone(),
                    presentation: formula.graph.presentation().nodes.get(&node.id).cloned(),
                })
                .collect(),
            edges: formula
                .graph
                .edges()
                .map(|edge| FormulaEdgeV1 {
                    id: edge.id,
                    from: edge.from,
                    to: edge.to,
                    policy: edge.data,
                })
                .collect(),
            presentation: formula.graph.presentation().clone(),
            properties: formula.properties.clone(),
            surface: formula.surface.clone(),
            managed_regions: formula.managed_regions.clone(),
            metadata: formula.metadata.clone(),
            defaults: formula.defaults.clone(),
        }
    }

    pub fn into_formula(self) -> Result<AlchemistFormula, FormulaCodecError> {
        if self.file_version != FORMULA_FILE_VERSION {
            return Err(FormulaCodecError::UnsupportedVersion(self.file_version));
        }
        let mut graph = GraphDocument::new(self.graph_id, AlchemistGraphDomain, self.graph);
        let mut transaction = GraphTransaction::new(Revision::ZERO);
        for node in self.nodes {
            transaction.push(GraphOperation::InsertNode {
                node: GraphNode {
                    id: node.id,
                    data: node.data,
                },
                presentation: node.presentation,
            });
        }
        for edge in self.edges {
            transaction.push(GraphOperation::Connect {
                edge: GraphEdge {
                    id: edge.id,
                    from: edge.from,
                    to: edge.to,
                    data: edge.policy,
                },
            });
        }
        for comment in self.presentation.comments.into_values() {
            transaction.push(GraphOperation::UpsertComment(comment));
        }
        for group in self.presentation.groups.into_values() {
            transaction.push(GraphOperation::UpsertGroup(group));
        }
        for bookmark in self.presentation.bookmarks.into_values() {
            transaction.push(GraphOperation::UpsertBookmark(bookmark));
        }
        if !transaction.operations.is_empty() {
            graph.apply(transaction)?;
        }
        Ok(AlchemistFormula {
            id: self.id,
            schema: self.schema,
            graph,
            properties: self.properties,
            surface: self.surface,
            managed_regions: self.managed_regions,
            metadata: self.metadata,
            defaults: self.defaults,
        })
    }
}

pub fn encode_formula(formula: &AlchemistFormula) -> Result<String, FormulaCodecError> {
    Ok(serde_json::to_string_pretty(&FormulaFileV1::from_formula(formula))?)
}

pub fn decode_formula(source: &str) -> Result<AlchemistFormula, FormulaCodecError> {
    serde_json::from_str::<FormulaFileV1>(source)?.into_formula()
}

#[derive(Debug, Error)]
pub enum FormulaCodecError {
    #[error("unsupported formula file version {0}")]
    UnsupportedVersion(u32),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Graph(#[from] GraphTransactionError),
}
