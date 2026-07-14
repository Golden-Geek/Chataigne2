use crate::testkit::{TestDomain, TestEdgeData, TestGraphData, TestNodeData, TestNodeKind};
use crate::{
    GraphDocument, GraphDomain, GraphEdge, GraphEnvelope, GraphGroup, GraphGroupId, GraphNode, GraphOperation,
    GraphTransaction, NodePresentation, PortRef, stable_topological_order, strongly_connected_components,
};

type TestDocument = GraphDocument<TestGraphData, TestNodeData, TestEdgeData>;

fn node(kind: TestNodeKind, label: &str) -> GraphNode<TestNodeData> {
    GraphNode::new(TestNodeData {
        kind,
        label: label.to_string(),
    })
}

fn edge(from: &GraphNode<TestNodeData>, to: &GraphNode<TestNodeData>) -> GraphEdge<TestEdgeData> {
    GraphEdge::new(
        PortRef::new(from.id, TestDomain::output_port()),
        PortRef::new(to.id, TestDomain::input_port()),
        TestEdgeData,
    )
}

fn connected_document() -> (TestDocument, GraphNode<TestNodeData>, GraphNode<TestNodeData>) {
    let domain = TestDomain;
    let mut graph = GraphDocument::new(TestGraphData {
        label: "test".to_string(),
    });
    let source = node(TestNodeKind::Source, "source");
    let sink = node(TestNodeKind::Sink, "sink");
    let connection = edge(&source, &sink);
    let mut transaction = GraphTransaction::for_document(&graph);
    transaction.insert_node(source.clone(), None);
    transaction.insert_node(sink.clone(), None);
    transaction.connect(connection);
    transaction.commit(&mut graph, &domain).unwrap();
    (graph, source, sink)
}

#[test]
fn transaction_updates_indexes_and_partitioned_revisions_once() {
    let domain = TestDomain;
    let mut graph = GraphDocument::new(TestGraphData::default());
    let source = node(TestNodeKind::Source, "source");
    let sink = node(TestNodeKind::Sink, "sink");
    let connection = edge(&source, &sink);
    let edge_id = connection.id;

    let mut transaction = GraphTransaction::for_document(&graph);
    transaction.insert_node(source.clone(), Some(NodePresentation::default()));
    transaction.insert_node(sink.clone(), None);
    transaction.connect(connection);
    let committed = transaction.commit(&mut graph, &domain).unwrap();

    assert_eq!(committed.delta.from.sequence, 0);
    assert_eq!(committed.delta.to.sequence, 1);
    assert_eq!(committed.delta.to.topology, 1);
    assert_eq!(committed.delta.to.payload, 1);
    assert_eq!(committed.delta.to.presentation, 1);
    assert_eq!(
        graph.incoming_edge(PortRef::new(sink.id, TestDomain::input_port())),
        Some(edge_id)
    );
    assert_eq!(graph.outgoing_edges(source.id).collect::<Vec<_>>(), vec![edge_id]);
}

#[test]
fn failed_transaction_rolls_back_payload_and_topology_exactly() {
    let domain = TestDomain;
    let (mut graph, _, sink) = connected_document();
    let before = graph.clone();
    let second_source = node(TestNodeKind::Source, "second");

    let mut transaction = GraphTransaction::for_document(&graph);
    transaction.replace_graph_data(TestGraphData {
        label: "changed".to_string(),
    });
    transaction.insert_node(second_source.clone(), None);
    transaction.connect(edge(&second_source, &sink));

    assert!(transaction.commit(&mut graph, &domain).is_err());
    assert_eq!(graph, before);
}

#[test]
fn node_payload_change_cannot_invalidate_connected_port_schema() {
    let domain = TestDomain;
    let (mut graph, source, _) = connected_document();
    let before = graph.clone();
    let mut transaction = GraphTransaction::for_document(&graph);
    transaction.replace_node_data(
        source.id,
        TestNodeData {
            kind: TestNodeKind::Sink,
            label: "no-output".to_string(),
        },
    );

    assert!(transaction.commit(&mut graph, &domain).is_err());
    assert_eq!(graph, before);
}

#[test]
fn rollback_restores_group_membership_removed_with_a_node() {
    let domain = TestDomain;
    let (mut graph, source, sink) = connected_document();
    let group_id = GraphGroupId::new();
    graph.presentation.groups.insert(
        group_id,
        GraphGroup {
            id: group_id,
            label: "group".to_string(),
            nodes: BTreeSet::from([source.id]),
            position: [0.0; 2],
            size: [100.0; 2],
        },
    );
    let before = graph.clone();
    let mut transaction = GraphTransaction::for_document(&graph);
    transaction.remove_node(source.id);
    transaction.replace_node_data(
        source.id,
        TestNodeData {
            kind: TestNodeKind::Source,
            label: "missing".to_string(),
        },
    );

    assert!(transaction.commit(&mut graph, &domain).is_err());
    assert_eq!(graph, before);
    assert!(graph.presentation().groups[&group_id].nodes.contains(&source.id));
    assert!(graph.node(sink.id).is_some());
}

#[test]
fn presentation_only_commit_does_not_invalidate_topology_or_payload() {
    let domain = TestDomain;
    let (mut graph, source, _) = connected_document();
    let before = graph.revision();
    let mut transaction = GraphTransaction::for_document(&graph);
    transaction.set_node_presentation(
        source.id,
        Some(NodePresentation {
            position: [12.0, 24.0],
            size: None,
            collapsed: true,
        }),
    );
    transaction.commit(&mut graph, &domain).unwrap();

    assert_eq!(graph.revision().sequence, before.sequence + 1);
    assert_eq!(graph.revision().topology, before.topology);
    assert_eq!(graph.revision().payload, before.payload);
    assert_eq!(graph.revision().presentation, before.presentation + 1);
}

#[test]
fn persistence_round_trip_rebuilds_topology_indexes() {
    let domain = TestDomain;
    let (graph, source, sink) = connected_document();
    let encoded = serde_json::to_string(&GraphEnvelope::from_document(
        domain.domain_id(),
        domain.schema_version(),
        &graph,
    ))
    .unwrap();
    let envelope: GraphEnvelope<TestGraphData, TestNodeData, TestEdgeData> = serde_json::from_str(&encoded).unwrap();
    let restored = envelope
        .into_document(domain.domain_id(), domain.schema_version())
        .unwrap();

    assert_eq!(restored, graph);
    assert_eq!(restored.outgoing_edges(source.id).count(), 1);
    assert_eq!(restored.incoming_edges(sink.id).count(), 1);
}

#[test]
fn deterministic_traversal_reports_dags_and_cycles() {
    let domain = TestDomain;
    let mut dag = GraphDocument::new(TestGraphData::default());
    let first = node(TestNodeKind::Source, "first");
    let middle = node(TestNodeKind::PassThrough, "middle");
    let last = node(TestNodeKind::Sink, "last");
    let mut transaction = GraphTransaction::for_document(&dag);
    for item in [&first, &middle, &last] {
        transaction.insert_node(item.clone(), None);
    }
    transaction.connect(edge(&first, &middle));
    transaction.connect(edge(&middle, &last));
    transaction.commit(&mut dag, &domain).unwrap();
    assert_eq!(
        stable_topological_order(&dag).unwrap(),
        vec![first.id, middle.id, last.id]
    );

    let mut cyclic = GraphDocument::new(TestGraphData::default());
    let a = node(TestNodeKind::PassThrough, "a");
    let b = node(TestNodeKind::PassThrough, "b");
    let c = node(TestNodeKind::PassThrough, "c");
    let mut transaction = GraphTransaction::for_document(&cyclic);
    for item in [&a, &b, &c] {
        transaction.insert_node(item.clone(), None);
    }
    transaction.connect(edge(&a, &b));
    transaction.connect(edge(&b, &c));
    transaction.connect(edge(&c, &a));
    transaction.commit(&mut cyclic, &domain).unwrap();
    assert_eq!(stable_topological_order(&cyclic).unwrap_err(), vec![a.id, b.id, c.id]);
    assert_eq!(strongly_connected_components(&cyclic), vec![vec![a.id, b.id, c.id]]);
}

#[test]
fn indexed_node_removal_scales_to_large_documents_without_snapshot_rebuilds() {
    let domain = TestDomain;
    let mut graph = GraphDocument::new(TestGraphData::default());
    let mut transaction = GraphTransaction::for_document(&graph);
    let mut ids = Vec::with_capacity(10_000);
    for index in 0..10_000 {
        let node = node(TestNodeKind::PassThrough, &format!("node-{index}"));
        ids.push(node.id);
        transaction.push(GraphOperation::InsertNode {
            node,
            presentation: None,
        });
    }
    transaction.commit(&mut graph, &domain).unwrap();

    let mut remove = GraphTransaction::for_document(&graph);
    remove.remove_node(ids[5_000]);
    let committed = remove.commit(&mut graph, &domain).unwrap();
    assert_eq!(graph.nodes().len(), 9_999);
    assert_eq!(committed.delta.changes.removed_nodes.len(), 1);
}
use std::collections::BTreeSet;
