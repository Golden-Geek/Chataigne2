use golden_graph::{
    GraphDocument, GraphEdge, GraphEdgeId, GraphId, GraphNode, GraphNodeId, GraphOperation, GraphPortId,
    GraphTransaction, PortRef,
};
use golden_model::Revision;

use super::*;

fn typed_port(value_type: &str) -> AlchemistPort {
    AlchemistPort {
        id: GraphPortId::new(),
        value_type: ValueTypeId::new(value_type).unwrap(),
    }
}

#[test]
fn alchemist_uses_generic_graph_transactions_with_typed_ports() {
    let output = typed_port("float");
    let input = typed_port("float");
    let source = GraphNode {
        id: GraphNodeId::new(),
        data: AlchemistNode {
            operation: "constant".into(),
            inputs: vec![],
            outputs: vec![output.clone()],
        },
    };
    let sink = GraphNode {
        id: GraphNodeId::new(),
        data: AlchemistNode {
            operation: "output".into(),
            inputs: vec![input.clone()],
            outputs: vec![],
        },
    };
    let mut graph = GraphDocument::new(
        GraphId::new(),
        AlchemistGraphDomain,
        AlchemistGraphData { name: "formula".into() },
    );
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    transaction.push(GraphOperation::InsertNode {
        node: source.clone(),
        presentation: None,
    });
    transaction.push(GraphOperation::InsertNode {
        node: sink.clone(),
        presentation: None,
    });
    transaction.push(GraphOperation::Connect {
        edge: GraphEdge {
            id: GraphEdgeId::new(),
            from: PortRef {
                node: source.id,
                port: output.id,
            },
            to: PortRef {
                node: sink.id,
                port: input.id,
            },
            data: ConversionPolicy::Exact,
        },
    });
    graph.apply(transaction).unwrap();
    assert_eq!(graph.nodes().len(), 2);
    assert_eq!(graph.edges().len(), 1);
}
