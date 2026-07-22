//! Small typed graph domain used by contract, adapter, and downstream tests.

use uuid::Uuid;

use crate::{
    GraphChangeSet, GraphDiagnostic, GraphDocument, GraphDomain, GraphPortId, PortDefinition, PortDirection, PortRef,
    PortSchema,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestGraphData {
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TestNodeKind {
    Source,
    PassThrough,
    Sink,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestNodeData {
    pub kind: TestNodeKind,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestPortData;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestEdgeData;

#[derive(Clone, Copy, Debug, Default)]
pub struct TestDomain;

impl TestDomain {
    #[must_use]
    pub const fn input_port() -> GraphPortId {
        GraphPortId::from_uuid(Uuid::from_u128(1))
    }

    #[must_use]
    pub const fn output_port() -> GraphPortId {
        GraphPortId::from_uuid(Uuid::from_u128(2))
    }
}

impl GraphDomain for TestDomain {
    type GraphData = TestGraphData;
    type NodeData = TestNodeData;
    type PortData = TestPortData;
    type EdgeData = TestEdgeData;

    fn domain_id(&self) -> &'static str {
        "golden.test"
    }

    fn schema_version(&self) -> u32 {
        1
    }

    fn node_ports(
        &self,
        node: &Self::NodeData,
        _graph: &GraphDocument<Self::GraphData, Self::NodeData, Self::EdgeData>,
    ) -> PortSchema<Self::PortData> {
        let mut ports = Vec::with_capacity(2);
        if node.kind != TestNodeKind::Source {
            ports.push(PortDefinition {
                id: Self::input_port(),
                direction: PortDirection::Input,
                data: TestPortData,
            });
        }
        if node.kind != TestNodeKind::Sink {
            ports.push(PortDefinition {
                id: Self::output_port(),
                direction: PortDirection::Output,
                data: TestPortData,
            });
        }
        PortSchema { ports }
    }

    fn validate_connection(
        &self,
        _graph: &GraphDocument<Self::GraphData, Self::NodeData, Self::EdgeData>,
        from: PortRef,
        to: PortRef,
    ) -> Result<(), GraphDiagnostic> {
        if from.node == to.node {
            Err(GraphDiagnostic::error("self_connection", "test nodes cannot connect to themselves").at_node(from.node))
        } else {
            Ok(())
        }
    }

    fn validate_graph(
        &self,
        _graph: &GraphDocument<Self::GraphData, Self::NodeData, Self::EdgeData>,
        _changes: &GraphChangeSet,
    ) -> Vec<GraphDiagnostic> {
        Vec::new()
    }
}
