use crate::{
    ANodeFieldPath, ANodeInstance, ANodeTypeId, AlchemistGraph, ExposedDeclId, ExposedParam, InputSocketRef,
    OutputSocketRef, ParamUiHints, ValueTypeId, ValueTypeSpec,
};

fn node(type_id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(type_id), type_id)
}

#[test]
fn graph_supports_basic_node_and_edge_edits() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(node("constant")).unwrap();
    let target = graph.add_node(node("debug")).unwrap();
    let from = OutputSocketRef::new(source, "value");
    let to = InputSocketRef::new(target, "value");

    graph.connect(from.clone(), to.clone()).unwrap();
    assert_eq!(graph.edges.len(), 1);
    assert!(graph.connect(from.clone(), to.clone()).is_err());

    graph.disconnect(&from, &to).unwrap();
    assert!(graph.edges.is_empty());

    graph.connect(from, to).unwrap();
    graph.remove_node(source).unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert!(graph.edges.is_empty());
}

#[test]
fn removing_internal_node_preserves_broken_exposed_contract() {
    let mut graph = AlchemistGraph::new();
    let node = graph.add_node(node("constant")).unwrap();
    graph.exposed.params.push(ExposedParam {
        decl_id: ExposedDeclId::new("gain"),
        label: "Gain".into(),
        description: None,
        target: ANodeFieldPath::new(node, "value"),
        value_type: ValueTypeSpec::Exact(ValueTypeId::new("float")),
        ui: ParamUiHints::default(),
    });

    graph.remove_node(node).unwrap();

    assert_eq!(graph.exposed.params.len(), 1);
    assert_eq!(graph.exposed.params[0].decl_id.as_str(), "gain");
}

#[test]
fn each_input_accepts_only_one_connection() {
    let mut graph = AlchemistGraph::new();
    let first = graph.add_node(node("constant")).unwrap();
    let second = graph.add_node(node("constant")).unwrap();
    let target = graph.add_node(node("add")).unwrap();
    let target_input = InputSocketRef::new(target, "a");

    graph
        .connect(OutputSocketRef::new(first, "value"), target_input.clone())
        .unwrap();

    assert!(
        graph
            .connect(OutputSocketRef::new(second, "value"), target_input)
            .is_err()
    );
}
