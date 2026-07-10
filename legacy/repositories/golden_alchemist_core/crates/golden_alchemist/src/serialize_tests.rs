use crate::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, RuntimeValue,
    serialize::{from_json, to_json_pretty},
};

#[test]
fn authored_graph_round_trips_through_json() {
    let mut graph = AlchemistGraph::new();
    graph.metadata.label = "Round trip".into();
    let mut node = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    node.config.set("value", RuntimeValue::Float(42.5));
    graph.add_node(node).unwrap();

    let encoded = to_json_pretty(&graph).unwrap();
    let decoded = from_json(&encoded).unwrap();

    assert_eq!(decoded, graph);
}

#[test]
fn newer_schema_is_rejected() {
    let mut graph = AlchemistGraph::new();
    graph.schema_version += 1;

    let encoded = to_json_pretty(&graph).unwrap();

    assert!(from_json(&encoded).is_err());
}
