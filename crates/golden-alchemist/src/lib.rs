//! Alchemist's typed formula graph-domain boundary.

use golden_graph::{GraphDiagnostic, GraphDocument, GraphDomain, GraphPortId, PortDescriptor, PortDirection, PortRef};
use golden_values::ValueTypeId;
use smol_str::SmolStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlchemistGraphData {
    pub name: SmolStr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlchemistPort {
    pub id: GraphPortId,
    pub value_type: ValueTypeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlchemistNode {
    pub operation: SmolStr,
    pub inputs: Vec<AlchemistPort>,
    pub outputs: Vec<AlchemistPort>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversionPolicy {
    Exact,
    Explicit,
}

#[derive(Clone, Copy)]
pub struct AlchemistGraphDomain;

impl GraphDomain for AlchemistGraphDomain {
    type GraphData = AlchemistGraphData;
    type NodeData = AlchemistNode;
    type PortData = ValueTypeId;
    type EdgeData = ConversionPolicy;

    fn node_ports(&self, node: &Self::NodeData, _graph: &GraphDocument<Self>) -> Vec<PortDescriptor<Self::PortData>> {
        node.inputs
            .iter()
            .map(|port| PortDescriptor {
                id: port.id,
                direction: PortDirection::Input,
                data: port.value_type.clone(),
            })
            .chain(node.outputs.iter().map(|port| PortDescriptor {
                id: port.id,
                direction: PortDirection::Output,
                data: port.value_type.clone(),
            }))
            .collect()
    }

    fn validate_connection(
        &self,
        graph: &GraphDocument<Self>,
        from: PortRef,
        to: PortRef,
        policy: &Self::EdgeData,
    ) -> Result<(), GraphDiagnostic> {
        let output = graph
            .node(from.node)
            .and_then(|node| node.data.outputs.iter().find(|port| port.id == from.port));
        let input = graph
            .node(to.node)
            .and_then(|node| node.data.inputs.iter().find(|port| port.id == to.port));
        match (output, input, policy) {
            (Some(output), Some(input), ConversionPolicy::Exact) if output.value_type != input.value_type => {
                Err(GraphDiagnostic::error(
                    "alchemist.type_mismatch",
                    format!(
                        "{} cannot connect to {} without an explicit conversion",
                        output.value_type.as_str(),
                        input.value_type.as_str()
                    ),
                ))
            }
            (Some(_), Some(_), _) => Ok(()),
            _ => Err(GraphDiagnostic::error(
                "alchemist.missing_port",
                "formula port is missing",
            )),
        }
    }
}

#[cfg(test)]
mod tests;
