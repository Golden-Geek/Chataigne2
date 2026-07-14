use crate::{GraphChangeSet, GraphDocument, GraphEdgeId, GraphNodeId, GraphPortId, PortRef};

/// Severity attached to a domain validation diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ts_rs::TS)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// One typed-domain validation result.
#[derive(Clone, Debug, PartialEq, Eq, ts_rs::TS)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphDiagnostic {
    pub code: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub node: Option<GraphNodeId>,
    pub edge: Option<GraphEdgeId>,
}

impl GraphDiagnostic {
    #[must_use]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: DiagnosticSeverity::Error,
            node: None,
            edge: None,
        }
    }

    #[must_use]
    pub fn at_node(mut self, node: GraphNodeId) -> Self {
        self.node = Some(node);
        self
    }

    #[must_use]
    pub fn at_edge(mut self, edge: GraphEdgeId) -> Self {
        self.edge = Some(edge);
        self
    }
}

/// Direction of a domain-declared node port.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ts_rs::TS)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PortDirection {
    Input,
    Output,
}

/// One stable port exposed by a domain node payload.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PortDefinition<P> {
    pub id: GraphPortId,
    pub direction: PortDirection,
    pub data: P,
}

/// Complete port schema for one node payload.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PortSchema<P> {
    pub ports: Vec<PortDefinition<P>>,
}

impl<P> PortSchema<P> {
    #[must_use]
    pub fn get(&self, port: GraphPortId) -> Option<&PortDefinition<P>> {
        self.ports.iter().find(|candidate| candidate.id == port)
    }
}

/// Domain policy for connections targeting one declared port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionPolicy {
    pub incoming: IncomingConnectionPolicy,
    pub allow_parallel: bool,
}

impl Default for ConnectionPolicy {
    fn default() -> Self {
        Self {
            incoming: IncomingConnectionPolicy::Single,
            allow_parallel: false,
        }
    }
}

/// Whether a target port accepts one edge or an arbitrary number of edges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IncomingConnectionPolicy {
    #[default]
    Single,
    Multiple,
}

/// Typed semantic adapter for a generic graph document.
pub trait GraphDomain: Send + Sync + 'static {
    type GraphData: Clone + std::fmt::Debug + PartialEq + Send + Sync + 'static;
    type NodeData: Clone + std::fmt::Debug + PartialEq + Send + Sync + 'static;
    type PortData: Clone + std::fmt::Debug + PartialEq + Send + Sync + 'static;
    type EdgeData: Clone + std::fmt::Debug + PartialEq + Send + Sync + 'static;

    fn domain_id(&self) -> &'static str;

    fn schema_version(&self) -> u32;

    fn node_ports(
        &self,
        node: &Self::NodeData,
        graph: &GraphDocument<Self::GraphData, Self::NodeData, Self::EdgeData>,
    ) -> PortSchema<Self::PortData>;

    fn connection_policy(
        &self,
        _graph: &GraphDocument<Self::GraphData, Self::NodeData, Self::EdgeData>,
        _from: PortRef,
        _to: PortRef,
    ) -> ConnectionPolicy {
        ConnectionPolicy::default()
    }

    fn validate_connection(
        &self,
        graph: &GraphDocument<Self::GraphData, Self::NodeData, Self::EdgeData>,
        from: PortRef,
        to: PortRef,
    ) -> Result<(), GraphDiagnostic>;

    fn validate_graph(
        &self,
        graph: &GraphDocument<Self::GraphData, Self::NodeData, Self::EdgeData>,
        changes: &GraphChangeSet,
    ) -> Vec<GraphDiagnostic>;
}
