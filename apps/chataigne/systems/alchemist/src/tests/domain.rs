#[cfg(feature = "serde")]
use crate::AlchemistGraphEnvelope;
#[cfg(feature = "serde")]
use golden_graph::GraphEnvelope;
use golden_graph::{GraphDomain, GraphNode, GraphTransaction, GraphTransactionError};

use crate::{
    ANodeInstance, ANodeTypeId, AlchemistGraphDocument, AlchemistGraphDomain, AlchemistGraphId, AlchemistNodeData,
    InputSocketRef, OutputSocketRef, SocketId,
};

fn representative_document() -> (AlchemistGraphDocument, AlchemistGraphId, crate::ANodeId) {
    let graph_id = AlchemistGraphId::new();
    let domain = AlchemistGraphDomain::with_primitives();
    let mut document = AlchemistGraphDomain::new_document_with_identity(graph_id, "Typed graph fixture");
    let mut transaction = GraphTransaction::for_document(&document);

    let mut source = ANodeInstance::new(ANodeTypeId::new("constant"), "Source");
    source.ui.position = [4.0, 5.0];
    let source_id = source.id;
    AlchemistGraphDomain::insert_node(&mut transaction, source);

    let mut target = ANodeInstance::new(ANodeTypeId::new("debug_value"), "Preview");
    target.ui.position = [18.0, 5.0];
    target.ui.collapsed = true;
    let target_id = target.id;
    AlchemistGraphDomain::insert_node(&mut transaction, target);
    AlchemistGraphDomain::connect(
        &mut transaction,
        &document,
        OutputSocketRef::new(source_id, "value"),
        InputSocketRef::new(target_id, "value"),
    );
    transaction.commit(&mut document, &domain).unwrap();
    (document, graph_id, source_id)
}

#[test]
fn typed_alchemist_document_uses_common_graph_contract() {
    let (document, graph_id, _) = representative_document();
    let domain = AlchemistGraphDomain::with_primitives();

    assert_eq!(document.id().as_uuid(), graph_id.as_uuid());
    assert_eq!(document.nodes().len(), 2);
    assert_eq!(document.edges().len(), 1);
    assert_eq!(document.data().metadata.label, "Typed graph fixture");
    assert!(domain.validate_document(&document).is_empty());
}

#[test]
fn alchemist_port_ids_are_stable_and_domain_declared() {
    let (document, _, source) = representative_document();
    let domain = AlchemistGraphDomain::with_primitives();
    let node = document.node(AlchemistGraphDomain::node_id(source)).unwrap();

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
    let (mut document, _, _) = representative_document();
    let before = document.clone();
    let unknown = ANodeInstance::new(ANodeTypeId::new("app.missing"), "Missing");
    let mut transaction = GraphTransaction::for_document(&document);
    transaction.insert_node(GraphNode::new(AlchemistNodeData::from_instance(&unknown)), None);

    let error = transaction.commit(&mut document, &domain).unwrap_err();

    assert!(matches!(error, GraphTransactionError::Validation(_)));
    assert_eq!(document, before);
}

#[test]
#[cfg(feature = "serde")]
fn common_graph_envelope_rebuilds_indexes_and_preserves_alchemist_semantics() {
    let (document, _, _) = representative_document();
    let domain = AlchemistGraphDomain::with_primitives();
    let envelope = GraphEnvelope::from_document(domain.domain_id(), domain.schema_version(), &document);
    let encoded = serde_json::to_string(&envelope).unwrap();
    let decoded: AlchemistGraphEnvelope = serde_json::from_str(&encoded).unwrap();

    let restored = decoded
        .into_document(domain.domain_id(), domain.schema_version())
        .unwrap();
    let edge = restored.edges().next().unwrap();

    assert_eq!(
        restored.incoming_edges_for_port(edge.to).collect::<Vec<_>>(),
        vec![edge.id]
    );
    assert_eq!(restored, document);
}

#[test]
fn vector_component_ports_have_stable_ids_and_scalar_compatibility() {
    let domain = AlchemistGraphDomain::with_primitives();
    let mut graph = AlchemistGraphDomain::new_document();
    let source = ANodeInstance::new(ANodeTypeId::new("pack_vec3"), "Vector");
    let target = ANodeInstance::new(ANodeTypeId::new("extract_vec3"), "Components");
    let mut transaction = GraphTransaction::for_document(&graph);
    AlchemistGraphDomain::insert_node(&mut transaction, source.clone());
    AlchemistGraphDomain::insert_node(&mut transaction, target.clone());
    transaction.commit(&mut graph, &domain).unwrap();
    let mut transaction = GraphTransaction::for_document(&graph);
    AlchemistGraphDomain::connect(
        &mut transaction,
        &graph,
        OutputSocketRef::new(source.id, "value.x"),
        InputSocketRef::new(target.id, "value.y"),
    );
    transaction.commit(&mut graph, &domain).unwrap();
    let ports = domain.node_ports(
        &graph.node(AlchemistGraphDomain::node_id(source.id)).unwrap().data,
        &graph,
    );
    assert!(
        ports
            .get(AlchemistGraphDomain::output_port_id(&SocketId::new("value.x")))
            .is_some()
    );
    assert!(
        ports
            .get(AlchemistGraphDomain::output_port_id(&SocketId::new("value.a")))
            .is_none()
    );
    let mut invalid = GraphTransaction::for_document(&graph);
    AlchemistGraphDomain::connect(
        &mut invalid,
        &graph,
        OutputSocketRef::new(source.id, "value.a"),
        InputSocketRef::new(target.id, "value.x"),
    );
    assert!(invalid.commit(&mut graph, &domain).is_err());
    assert_eq!(graph.edges().len(), 1);
}

#[test]
fn generic_port_admission_requires_the_graph_type_solver_for_cross_input_constraints() {
    let domain = AlchemistGraphDomain::with_primitives();
    let mut graph = AlchemistGraphDomain::new_document();
    let trigger = ANodeInstance::new(ANodeTypeId::new("trigger_on_off"), "Trigger");
    let scalar = ANodeInstance::new(ANodeTypeId::new("constant"), "Scalar");
    let compare = ANodeInstance::new(ANodeTypeId::new("compare"), "Compare");
    let mut transaction = GraphTransaction::for_document(&graph);
    for node in [&trigger, &scalar, &compare] {
        AlchemistGraphDomain::insert_node(&mut transaction, node.clone());
    }
    AlchemistGraphDomain::connect(
        &mut transaction,
        &graph,
        OutputSocketRef::new(trigger.id, "on"),
        InputSocketRef::new(compare.id, "left"),
    );
    AlchemistGraphDomain::connect(
        &mut transaction,
        &graph,
        OutputSocketRef::new(scalar.id, "value"),
        InputSocketRef::new(compare.id, "right"),
    );
    transaction.commit(&mut graph, &domain).unwrap();
    let solved = crate::solve_document_types(
        &graph,
        &crate::TypeSolveCtx {
            nodes: &crate::primitive_node_registry(),
            value_types: &crate::ValueTypeRegistry::with_primitives(),
            properties: None,
        },
    );
    assert!(solved.has_errors());
}
