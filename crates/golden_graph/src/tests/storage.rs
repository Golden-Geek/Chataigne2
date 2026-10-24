use super::*;
use crate::{GraphDocumentData, GraphNodeId};

fn ordered_fixture() -> TestDocument {
    let mut graph = GraphDocument::new(TestGraphData::default());
    let mut tx = GraphTransaction::for_document(&graph);
    let nodes = [
        node(TestNodeKind::Source, "source-a"),
        node(TestNodeKind::Sink, "sink-a"),
        node(TestNodeKind::Source, "source-b"),
        node(TestNodeKind::Sink, "sink-b"),
    ];
    for (index, node) in nodes.iter().enumerate() {
        tx.insert_node(
            node.clone(),
            Some(NodePresentation {
                position: [index as f64, 10.0],
                ..Default::default()
            }),
        );
    }
    tx.connect(edge(&nodes[0], &nodes[1]));
    tx.connect(edge(&nodes[2], &nodes[3]));
    tx.commit(&mut graph, &TestDomain).unwrap();
    graph
}

fn encoded(graph: &TestDocument) -> String {
    serde_json::to_string(&GraphEnvelope::from_document("test", 1, graph)).unwrap()
}

#[test]
fn presentation_replacement_keeps_authored_node_order_and_only_advances_layout() {
    let mut graph = ordered_fixture();
    let nodes = graph.nodes().map(|node| node.id).collect::<Vec<_>>();
    let layout_order = graph.presentation().nodes.keys().copied().collect::<Vec<_>>();
    let before = graph.revision();
    let mut tx = GraphTransaction::for_document(&graph);
    tx.set_node_presentation(
        nodes[0],
        Some(NodePresentation {
            position: [42.0, 24.0],
            ..Default::default()
        }),
    );
    let result = tx.commit(&mut graph, &TestDomain).unwrap();
    assert_eq!(graph.nodes().map(|node| node.id).collect::<Vec<_>>(), nodes);
    assert_eq!(
        graph.presentation().nodes.keys().copied().collect::<Vec<_>>(),
        layout_order
    );
    assert_eq!(graph.presentation().nodes[&nodes[0]].position, [42.0, 24.0]);
    assert_eq!(result.delta.to.topology, before.topology);
    assert_eq!(result.delta.to.payload, before.payload);
    assert_eq!(result.delta.to.presentation, before.presentation + 1);
    assert_eq!(result.delta.changes.presentation_nodes, BTreeSet::from([nodes[0]]));
}

#[test]
fn failed_mixed_removals_and_layout_edits_restore_exact_persisted_order() {
    let mut graph = ordered_fixture();
    let nodes = graph.nodes().map(|node| node.id).collect::<Vec<_>>();
    let edges = graph.edges().map(|edge| edge.id).collect::<Vec<_>>();
    let before = graph.clone();
    let before_json = encoded(&graph);
    let mut tx = GraphTransaction::for_document(&graph);
    tx.set_node_presentation(nodes[1], None);
    tx.set_node_presentation(nodes[2], Some(NodePresentation::default()));
    tx.remove_node(nodes[0]);
    tx.disconnect(edges[1]);
    tx.remove_node(nodes[3]);
    tx.insert_node(
        node(TestNodeKind::Source, "temporary"),
        Some(NodePresentation::default()),
    );
    tx.remove_node(GraphNodeId::new());
    assert!(tx.commit(&mut graph, &TestDomain).is_err());
    assert_eq!(graph, before);
    assert_eq!(encoded(&graph), before_json);
    assert_eq!(graph.nodes().map(|node| node.id).collect::<Vec<_>>(), nodes);
    assert_eq!(graph.edges().map(|edge| edge.id).collect::<Vec<_>>(), edges);
}

#[test]
fn committed_order_survives_removal_append_and_persistence_reload() {
    let mut graph = ordered_fixture();
    let nodes = graph.nodes().map(|node| node.id).collect::<Vec<_>>();
    let appended = node(TestNodeKind::Source, "appended");
    let appended_id = appended.id;
    let mut tx = GraphTransaction::for_document(&graph);
    tx.remove_node(nodes[1]);
    tx.insert_node(appended, Some(NodePresentation::default()));
    tx.commit(&mut graph, &TestDomain).unwrap();
    let expected = vec![nodes[0], nodes[2], nodes[3], appended_id];
    assert_eq!(graph.nodes().map(|node| node.id).collect::<Vec<_>>(), expected);
    let persisted = GraphDocumentData::from_document(&graph);
    assert_eq!(persisted.nodes.keys().copied().collect::<Vec<_>>(), expected);
    let restored = persisted.into_document().unwrap();
    assert_eq!(graph, restored);
    assert_eq!(encoded(&graph), encoded(&restored));
}
