use super::*;

#[test]
fn reused_node_identity_publishes_changed_authored_order_as_topology() {
    let mut graph = GraphDocument::new(TestGraphData::default());
    let nodes = (0..3)
        .map(|n| node(TestNodeKind::Source, &format!("node-{n}")))
        .collect::<Vec<_>>();
    let mut tx = GraphTransaction::for_document(&graph);
    for node in &nodes {
        tx.insert_node(node.clone(), None);
    }
    tx.commit(&mut graph, &TestDomain).unwrap();
    let before = graph.revision();
    let mut tx = GraphTransaction::for_document(&graph);
    tx.remove_node(nodes[0].id);
    tx.insert_node(nodes[0].clone(), None);
    let result = tx.commit(&mut graph, &TestDomain).unwrap();
    assert_eq!(
        graph.nodes().map(|node| node.id).collect::<Vec<_>>(),
        [nodes[1].id, nodes[2].id, nodes[0].id]
    );
    assert_eq!(result.delta.to.topology, before.topology + 1);
    assert_eq!(result.delta.changes.reinserted_nodes, BTreeSet::from([nodes[0].id]));
    assert_eq!(result.delta.changes.updated_nodes, BTreeSet::from([nodes[0].id]));
}

#[test]
fn reused_edge_identity_publishes_changed_endpoints_as_topology() {
    let (mut graph, original, sink) = connected_document();
    let source = node(TestNodeKind::Source, "replacement source");
    let mut tx = GraphTransaction::for_document(&graph);
    tx.insert_node(source.clone(), None);
    tx.commit(&mut graph, &TestDomain).unwrap();
    let before = graph.revision();
    let edge_id = graph.edges().next().unwrap().id;
    let mut replacement = edge(&source, &sink);
    replacement.id = edge_id;
    let mut tx = GraphTransaction::for_document(&graph);
    tx.disconnect(edge_id);
    tx.connect(replacement);
    let result = tx.commit(&mut graph, &TestDomain).unwrap();
    assert_eq!(graph.outgoing_edges(original.id).count(), 0);
    assert_eq!(graph.outgoing_edges(source.id).collect::<Vec<_>>(), [edge_id]);
    assert_eq!(result.delta.to.topology, before.topology + 1);
    assert_eq!(result.delta.changes.reinserted_edges, BTreeSet::from([edge_id]));
    assert_eq!(result.delta.changes.updated_edges, BTreeSet::from([edge_id]));
}

#[test]
fn reinsertion_then_final_removal_does_not_leave_a_reinsertion_marker() {
    let (mut graph, source, sink) = connected_document();
    let connection = graph.edges().next().unwrap().clone();
    let mut tx = GraphTransaction::for_document(&graph);
    tx.disconnect(connection.id);
    tx.connect(connection.clone());
    tx.disconnect(connection.id);
    tx.remove_node(source.id);
    tx.insert_node(source.clone(), None);
    tx.remove_node(source.id);
    let result = tx.commit(&mut graph, &TestDomain).unwrap();
    assert!(result.delta.changes.reinserted_nodes.is_empty());
    assert!(result.delta.changes.reinserted_edges.is_empty());
    assert!(result.delta.changes.updated_nodes.is_empty());
    assert!(result.delta.changes.updated_edges.is_empty());
    assert_eq!(result.delta.changes.removed_nodes, BTreeSet::from([source.id]));
    assert_eq!(result.delta.changes.removed_edges, BTreeSet::from([connection.id]));
    assert!(graph.node(sink.id).is_some());
}

#[test]
fn transient_insert_remove_leaves_no_delta_or_revision_change() {
    let mut graph = GraphDocument::new(TestGraphData::default());
    let before = graph.revision();
    let temporary = node(TestNodeKind::Source, "temporary");
    let mut tx = GraphTransaction::for_document(&graph);
    tx.insert_node(temporary.clone(), Some(NodePresentation::default()));
    tx.remove_node(temporary.id);
    let result = tx.commit(&mut graph, &TestDomain).unwrap();
    assert!(result.delta.changes.is_empty());
    assert_eq!(result.delta.to, before);
}
