use std::collections::BTreeMap;

use golden_graph::{
    GraphDocument, GraphId, GraphNode, GraphNodeId, GraphOperation, GraphPortId, GraphTransaction, PortRef,
};
use golden_model::Revision;
use golden_values::{Value, ValueTypeId};
use smol_str::SmolStr;

use crate::{
    ANodeTypeId, AlchemistFormula, AlchemistGraphData, AlchemistGraphDomain, AlchemistNode, AlchemistPort,
    FormulaDefaults, FormulaId, FormulaMetadata, FormulaSchema, FormulaSurface, SurfaceInput, SurfaceItemId,
    SurfaceOutput,
};

pub struct SingleNodeInputSpec {
    pub name: SmolStr,
    pub label: SmolStr,
    pub value_type: ValueTypeId,
    pub default: Value,
}

pub struct SingleNodeOutputSpec {
    pub name: SmolStr,
    pub label: SmolStr,
    pub value_type: ValueTypeId,
}

pub struct SingleNodeFormulaSpec {
    pub id: FormulaId,
    pub name: SmolStr,
    pub description: String,
    pub tags: Vec<SmolStr>,
    pub node_type: ANodeTypeId,
    pub inputs: Vec<SingleNodeInputSpec>,
    pub outputs: Vec<SingleNodeOutputSpec>,
}

pub fn build_single_node_formula(spec: SingleNodeFormulaSpec) -> AlchemistFormula {
    let node_id = GraphNodeId::new();
    let inputs = spec
        .inputs
        .into_iter()
        .map(|input| {
            let port = AlchemistPort {
                id: GraphPortId::new(),
                name: input.name,
                value_type: input.value_type.clone(),
            };
            let surface = SurfaceInput {
                id: SurfaceItemId::new(),
                label: input.label,
                target: PortRef {
                    node: node_id,
                    port: port.id,
                },
                value_type: input.value_type,
                default: input.default,
            };
            (port, surface)
        })
        .collect::<Vec<_>>();
    let outputs = spec
        .outputs
        .into_iter()
        .map(|output| {
            let port = AlchemistPort {
                id: GraphPortId::new(),
                name: output.name,
                value_type: output.value_type.clone(),
            };
            let surface = SurfaceOutput {
                id: SurfaceItemId::new(),
                label: output.label,
                source: PortRef {
                    node: node_id,
                    port: port.id,
                },
                value_type: output.value_type,
            };
            (port, surface)
        })
        .collect::<Vec<_>>();
    let mut graph = GraphDocument::new(
        GraphId::new(),
        AlchemistGraphDomain,
        AlchemistGraphData {
            name: spec.name.clone(),
        },
    );
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    transaction.push(GraphOperation::InsertNode {
        node: GraphNode {
            id: node_id,
            data: AlchemistNode {
                node_type: spec.node_type,
                inputs: inputs.iter().map(|(port, _)| port.clone()).collect(),
                outputs: outputs.iter().map(|(port, _)| port.clone()).collect(),
                config: BTreeMap::new(),
            },
        },
        presentation: None,
    });
    graph
        .apply(transaction)
        .expect("single-node formula builder emits a valid graph");
    AlchemistFormula {
        id: spec.id,
        schema: FormulaSchema { version: 1 },
        graph,
        properties: Vec::new(),
        surface: FormulaSurface {
            inputs: inputs.into_iter().map(|(_, surface)| surface).collect(),
            outputs: outputs.into_iter().map(|(_, surface)| surface).collect(),
        },
        managed_regions: Vec::new(),
        metadata: FormulaMetadata {
            name: spec.name,
            description: spec.description,
            tags: spec.tags,
        },
        defaults: FormulaDefaults::default(),
    }
}
