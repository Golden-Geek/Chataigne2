use golden_model::EntityId;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::{GraphChangeSet, GraphDocument, GraphNodeId, GraphPortId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PortRef {
    pub node: GraphNodeId,
    pub port: GraphPortId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PortDescriptor<P> {
    pub id: GraphPortId,
    pub direction: PortDirection,
    pub data: P,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraphDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: SmolStr,
    pub message: String,
    pub entities: Vec<EntityId>,
}

impl GraphDiagnostic {
    pub fn error(code: impl Into<SmolStr>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            entities: Vec::new(),
        }
    }
}

pub trait GraphDomain: Send + Sync + 'static {
    type GraphData: Clone + Send + Sync;
    type NodeData: Clone + Send + Sync;
    type PortData: Clone + Eq + Send + Sync;
    type EdgeData: Clone + Send + Sync;

    fn node_ports(&self, node: &Self::NodeData, graph: &GraphDocument<Self>) -> Vec<PortDescriptor<Self::PortData>>
    where
        Self: Sized;

    fn validate_connection(
        &self,
        graph: &GraphDocument<Self>,
        from: PortRef,
        to: PortRef,
        edge: &Self::EdgeData,
    ) -> Result<(), GraphDiagnostic>
    where
        Self: Sized;

    fn validate_graph(&self, _graph: &GraphDocument<Self>, _changes: &GraphChangeSet) -> Vec<GraphDiagnostic>
    where
        Self: Sized,
    {
        Vec::new()
    }
}
