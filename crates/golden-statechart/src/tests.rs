use golden_graph::{
    GraphDocument, GraphEdge, GraphEdgeId, GraphId, GraphNode, GraphNodeId, GraphOperation, GraphPortId,
    GraphTransaction, PortRef,
};
use golden_model::Revision;

use super::*;

fn state(label: &str, kind: StateKind) -> GraphNode<StatechartGraphDomain> {
    GraphNode {
        id: GraphNodeId::new(),
        data: StateNode {
            label: label.into(),
            kind,
            transition_input: GraphPortId::new(),
            transition_output: GraphPortId::new(),
        },
    }
}

#[test]
fn statecharts_share_graph_mechanics_without_alchemist() {
    let initial = state("Initial", StateKind::Initial);
    let active = state("Active", StateKind::Atomic);
    let mut graph = GraphDocument::new(
        GraphId::new(),
        StatechartGraphDomain,
        StatechartData { name: "machine".into() },
    );
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    transaction.push(GraphOperation::InsertNode {
        node: initial.clone(),
        presentation: None,
    });
    transaction.push(GraphOperation::InsertNode {
        node: active.clone(),
        presentation: None,
    });
    transaction.push(GraphOperation::Connect {
        edge: GraphEdge {
            id: GraphEdgeId::new(),
            from: PortRef {
                node: initial.id,
                port: initial.data.transition_output,
            },
            to: PortRef {
                node: active.id,
                port: active.data.transition_input,
            },
            data: TransitionData { event: None },
        },
    });
    graph.apply(transaction).unwrap();
    assert_eq!(graph.edges().len(), 1);
}
