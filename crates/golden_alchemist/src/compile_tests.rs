use crate::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, CompileCtx, InputSocketRef, OutputSocketRef, RuntimeValue,
    ValueTypeRegistry, compile_graph, primitive_node_registry,
};

fn node(type_id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(type_id), type_id)
}

fn compile(graph: &AlchemistGraph) -> crate::CompileResult {
    compile_graph(
        graph,
        &CompileCtx {
            value_types: &ValueTypeRegistry::with_primitives(),
            nodes: &primitive_node_registry(),
        },
    )
}

#[test]
fn compiler_builds_dense_schedule_and_memory_layout() {
    let mut graph = AlchemistGraph::new();
    let mut constant = node("constant");
    constant.config.set("value", RuntimeValue::Float(2.0));
    let constant = graph.add_node(constant).unwrap();
    let add = graph.add_node(node("add")).unwrap();
    let edge = graph.add_node(node("edge")).unwrap();
    graph
        .connect(OutputSocketRef::new(constant, "value"), InputSocketRef::new(add, "a"))
        .unwrap();

    let result = compile(&graph);
    let compiled = result.compiled.unwrap();

    assert_eq!(compiled.exec_nodes.len(), 3);
    assert_eq!(compiled.topo_order.len(), 3);
    assert_eq!(compiled.exec_nodes[0].exec_id.index(), 0);
    assert_eq!(compiled.exec_nodes[1].exec_id.index(), 1);
    assert_eq!(compiled.debug_map.exec_to_authored, vec![constant, add, edge]);
    assert_eq!(compiled.state_layout.state_slot_count, 1);
}

#[test]
fn cycle_without_delay_is_reported() {
    let mut graph = AlchemistGraph::new();
    let first = graph.add_node(node("add")).unwrap();
    let second = graph.add_node(node("add")).unwrap();
    graph
        .connect(OutputSocketRef::new(first, "result"), InputSocketRef::new(second, "a"))
        .unwrap();
    graph
        .connect(OutputSocketRef::new(second, "result"), InputSocketRef::new(first, "a"))
        .unwrap();

    let result = compile(&graph);

    assert!(result.compiled.is_none());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "cycle_without_delay")
    );
}

#[test]
fn delay_node_allows_feedback() {
    let mut graph = AlchemistGraph::new();
    let add = graph.add_node(node("add")).unwrap();
    let delay = graph.add_node(node("delay_one_tick")).unwrap();
    graph
        .connect(OutputSocketRef::new(add, "result"), InputSocketRef::new(delay, "value"))
        .unwrap();
    graph
        .connect(OutputSocketRef::new(delay, "value"), InputSocketRef::new(add, "a"))
        .unwrap();

    let result = compile(&graph);

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    assert_eq!(result.compiled.unwrap().topo_order.len(), 2);
}
