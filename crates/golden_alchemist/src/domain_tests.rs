use golden_graph::{GraphDomain, GraphEnvelope, GraphNode, GraphTransaction, GraphTransactionError};

use crate::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, AlchemistGraphAdapter, AlchemistGraphAdapterError,
    AlchemistGraphDomain, AlchemistGraphEnvelope, AlchemistNodeData, GraphComment, GraphGroup, InputSocketRef,
    OutputSocketRef, SocketId,
};

fn representative_graph() -> AlchemistGraph {
    let mut graph = AlchemistGraph::new();
    graph.metadata.label = "Phase 3 domain fixture".into();
    graph.layout.viewport_origin = [12.5, -8.0];
    graph.layout.viewport_zoom = 1.25;

    let mut source = ANodeInstance::new(ANodeTypeId::new("constant"), "Source");
    source.ui.position = [4.0, 5.0];
    let source = graph.add_node(source).unwrap();
    let mut target = ANodeInstance::new(ANodeTypeId::new("debug_value"), "Preview");
    target.ui.position = [18.0, 5.0];
    target.ui.collapsed = true;
    let target = graph.add_node(target).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(target, "value"),
        )
        .unwrap();
    graph.layout.comments.push(GraphComment {
        text: "real Alchemist graph".into(),
        position: [2.0, 2.0],
        size: [20.0, 8.0],
    });
    graph.layout.groups.push(GraphGroup {
        label: "fixture".into(),
        nodes: vec![source, target],
        position: [0.0, 0.0],
        size: [30.0, 15.0],
    });
    graph
}

#[test]
fn real_alchemist_graph_round_trips_through_common_graph_contract() {
    let graph = representative_graph();
    let domain = AlchemistGraphDomain::with_primitives();

    let document = AlchemistGraphAdapter::to_document(&graph).unwrap();

    assert_eq!(document.id().as_uuid(), graph.id.as_uuid());
    assert_eq!(document.nodes().len(), graph.nodes.len());
    assert_eq!(document.edges().len(), graph.edges.len());
    assert!(domain.validate_document(&document).is_empty());
    let mut restored = AlchemistGraphAdapter::to_legacy(&document, &domain).unwrap();
    let mut expected = graph;
    for group in &mut restored.layout.groups {
        group.nodes.sort_unstable();
    }
    for group in &mut expected.layout.groups {
        group.nodes.sort_unstable();
    }
    assert_eq!(restored, expected);
}

#[test]
fn alchemist_port_ids_are_stable_and_domain_declared() {
    let graph = representative_graph();
    let source = graph.nodes.values().find(|node| node.label == "Source").unwrap();
    let domain = AlchemistGraphDomain::with_primitives();
    let document = AlchemistGraphAdapter::to_document(&graph).unwrap();
    let node = document.node(AlchemistGraphDomain::node_id(source.id)).unwrap();

    let ports = domain.node_ports(&node.data, &document);

    assert!(
        ports
            .get(AlchemistGraphDomain::output_port_id(&SocketId::new("value")))
            .is_some()
    );
    assert_eq!(
        AlchemistGraphDomain::output_port_id(&SocketId::new("value")),
        AlchemistGraphDomain::output_port_id(&SocketId::new("value"))
    );
}

#[test]
fn generic_transaction_rejects_unknown_anode_without_mutating_document() {
    let domain = AlchemistGraphDomain::with_primitives();
    let mut document = AlchemistGraphAdapter::to_document(&representative_graph()).unwrap();
    let before = document.clone();
    let unknown = ANodeInstance::new(ANodeTypeId::new("app.missing"), "Missing");
    let mut transaction = GraphTransaction::for_document(&document);
    transaction.insert_node(GraphNode::new(AlchemistNodeData::from_instance(&unknown)), None);

    let error = transaction.commit(&mut document, &domain).unwrap_err();

    assert!(matches!(error, GraphTransactionError::Validation(_)));
    assert_eq!(document, before);
}

#[test]
fn adapter_rejects_duplicate_legacy_connections_instead_of_dropping_data() {
    let mut graph = representative_graph();
    graph.edges.push(graph.edges[0].clone());

    let error = AlchemistGraphAdapter::to_document(&graph).unwrap_err();

    assert!(matches!(error, AlchemistGraphAdapterError::DuplicateConnection { .. }));
}

#[test]
fn common_graph_envelope_rebuilds_indexes_and_preserves_alchemist_semantics() {
    let graph = representative_graph();
    let domain = AlchemistGraphDomain::with_primitives();
    let document = AlchemistGraphAdapter::to_document(&graph).unwrap();
    let envelope = GraphEnvelope::from_document(domain.domain_id(), domain.schema_version(), &document);
    let encoded = serde_json::to_string(&envelope).unwrap();
    let decoded: AlchemistGraphEnvelope = serde_json::from_str(&encoded).unwrap();

    let restored_document = decoded
        .into_document(domain.domain_id(), domain.schema_version())
        .unwrap();
    let edge = restored_document.edges().next().unwrap();

    assert_eq!(
        restored_document.incoming_edges_for_port(edge.to).collect::<Vec<_>>(),
        vec![edge.id]
    );
    let mut restored = AlchemistGraphAdapter::to_legacy(&restored_document, &domain).unwrap();
    let mut expected = graph;
    for group in &mut restored.layout.groups {
        group.nodes.sort_unstable();
    }
    for group in &mut expected.layout.groups {
        group.nodes.sort_unstable();
    }
    assert_eq!(restored, expected);
}
