use std::{collections::HashMap, sync::Arc};

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
            parent: None,
            transition_input: GraphPortId::new(),
            transition_output: GraphPortId::new(),
            entry_processors: vec![format!("enter-{label}").into()],
            exit_processors: vec![format!("exit-{label}").into()],
        },
    }
}

fn transition(
    from: &GraphNode<StatechartGraphDomain>,
    to: &GraphNode<StatechartGraphDomain>,
    event: Option<&str>,
    guard: Option<&str>,
    priority: u16,
) -> GraphEdge<StatechartGraphDomain> {
    GraphEdge {
        id: GraphEdgeId::new(),
        from: PortRef {
            node: from.id,
            port: from.data.transition_output,
        },
        to: PortRef {
            node: to.id,
            port: to.data.transition_input,
        },
        data: TransitionData {
            event: event.map(Into::into),
            guard: guard.map(Into::into),
            priority,
            internal: false,
        },
    }
}

#[test]
fn compiled_runtime_keeps_one_non_multiplexed_statechart_truth() {
    let initial = state("initial", StateKind::Initial);
    let idle = state("idle", StateKind::Atomic);
    let active = state("active", StateKind::Atomic);
    let mut graph = GraphDocument::new(
        GraphId::new(),
        StatechartGraphDomain,
        StatechartData { name: "test".into() },
    );
    let mut transaction = GraphTransaction::new(Revision::ZERO);
    for node in [&initial, &idle, &active] {
        transaction.push(GraphOperation::InsertNode {
            node: node.clone(),
            presentation: None,
        });
    }
    for edge in [
        transition(&initial, &idle, None, None, 0),
        transition(&idle, &active, Some("go"), Some("allowed"), 0),
    ] {
        transaction.push(GraphOperation::Connect { edge });
    }
    graph.apply(transaction).unwrap();
    let plan = Arc::new(StatechartCompiler.compile(&graph).unwrap());
    let mut runtime = StatechartRuntime::new(plan);
    assert!(runtime.active().states.contains(&idle.id));

    let blocked = runtime.step(Some("go"), &HashMap::new());
    assert!(!blocked.transitioned);
    let step = runtime.step(Some("go"), &HashMap::from([("allowed".into(), true)]));
    assert!(step.transitioned);
    assert_eq!(step.exited, vec![idle.id]);
    assert_eq!(step.entered, vec![active.id]);
    assert!(runtime.active().states.contains(&active.id));
}
