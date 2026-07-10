use golden_graph::{
    GraphDiagnostic, GraphDocument, GraphDomain, GraphNodeId, GraphPortId, PortDescriptor, PortDirection, PortRef,
};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StatechartData {
    pub name: SmolStr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StateKind {
    Initial,
    Atomic,
    Compound,
    Final,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StateNode {
    pub label: SmolStr,
    pub kind: StateKind,
    pub parent: Option<GraphNodeId>,
    pub transition_input: GraphPortId,
    pub transition_output: GraphPortId,
    pub entry_processors: Vec<SmolStr>,
    pub exit_processors: Vec<SmolStr>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StatechartPort {
    Transition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransitionData {
    pub event: Option<SmolStr>,
    pub guard: Option<SmolStr>,
    pub priority: u16,
    pub internal: bool,
}

#[derive(Clone, Copy)]
pub struct StatechartGraphDomain;

impl GraphDomain for StatechartGraphDomain {
    type GraphData = StatechartData;
    type NodeData = StateNode;
    type PortData = StatechartPort;
    type EdgeData = TransitionData;

    fn node_ports(&self, node: &Self::NodeData, _graph: &GraphDocument<Self>) -> Vec<PortDescriptor<Self::PortData>> {
        let mut ports = vec![PortDescriptor {
            id: node.transition_input,
            direction: PortDirection::Input,
            data: StatechartPort::Transition,
        }];
        if node.kind != StateKind::Final {
            ports.push(PortDescriptor {
                id: node.transition_output,
                direction: PortDirection::Output,
                data: StatechartPort::Transition,
            });
        }
        ports
    }

    fn validate_connection(
        &self,
        _graph: &GraphDocument<Self>,
        from: PortRef,
        to: PortRef,
        edge: &Self::EdgeData,
    ) -> Result<(), GraphDiagnostic> {
        if from.node == to.node && !edge.internal {
            Err(GraphDiagnostic::error(
                "statechart.external_self_transition",
                "self transitions must be explicitly marked internal",
            ))
        } else {
            Ok(())
        }
    }
}
