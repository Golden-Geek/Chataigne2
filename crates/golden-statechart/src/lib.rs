//! Statechart's typed graph-domain boundary, independent of Alchemist.

use golden_graph::{GraphDiagnostic, GraphDocument, GraphDomain, GraphPortId, PortDescriptor, PortDirection, PortRef};
use smol_str::SmolStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatechartData {
    pub name: SmolStr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateKind {
    Initial,
    Atomic,
    Compound,
    Final,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateNode {
    pub label: SmolStr,
    pub kind: StateKind,
    pub transition_input: GraphPortId,
    pub transition_output: GraphPortId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatechartPort {
    Transition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionData {
    pub event: Option<SmolStr>,
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
        _edge: &Self::EdgeData,
    ) -> Result<(), GraphDiagnostic> {
        if from.node == to.node {
            Err(GraphDiagnostic::error(
                "statechart.self_transition",
                "self transitions require an explicit internal-transition node",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
