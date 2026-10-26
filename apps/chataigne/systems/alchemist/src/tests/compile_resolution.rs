use super::support::TestGraph;
use crate::{
    ANodeInstance, ANodeTypeId, CompileCtx, InputSocketRef, OutputSocketRef, RuntimeValue, TypeSolveCtx,
    ValueTypeRegistry, compile_graph, primitive_node_registry, solve_document_types,
};

#[test]
fn compilation_exposes_its_socket_resolution_even_when_a_cycle_prevents_execution() {
    let nodes = primitive_node_registry();
    let value_types = ValueTypeRegistry::with_primitives();
    for cycle in [false, true] {
        let mut graph = TestGraph::new();
        let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Source");
        constant.config.set("value", RuntimeValue::Float(2.0));
        let source = graph.add_node(constant).unwrap();
        let first = graph
            .add_node(ANodeInstance::new(ANodeTypeId::new("math"), "First"))
            .unwrap();
        let second = graph
            .add_node(ANodeInstance::new(ANodeTypeId::new("math"), "Second"))
            .unwrap();
        graph
            .connect(
                OutputSocketRef::new(source, "value"),
                InputSocketRef::new(first, "value1"),
            )
            .unwrap();
        graph
            .connect(
                OutputSocketRef::new(first, "result"),
                InputSocketRef::new(second, "value1"),
            )
            .unwrap();
        if cycle {
            graph
                .connect(
                    OutputSocketRef::new(second, "result"),
                    InputSocketRef::new(first, "value2"),
                )
                .unwrap();
        }
        let document = graph.to_document();
        let compilation = compile_graph(
            &document,
            &CompileCtx {
                nodes: &nodes,
                value_types: &value_types,
                properties: None,
            },
        );
        let expected = solve_document_types(
            &document,
            &TypeSolveCtx {
                nodes: &nodes,
                value_types: &value_types,
                properties: None,
            },
        );
        assert!(!expected.has_errors());
        assert_eq!(compilation.resolved.nodes.len(), 3);
        for (id, resolved) in &expected.graph.nodes {
            let actual = &compilation.resolved.nodes[id];
            assert_eq!(actual.bindings, resolved.bindings);
            assert_eq!(actual.signature, resolved.signature);
        }
        assert_eq!(compilation.has_errors(), cycle);
        if cycle {
            assert!(compilation.compiled.is_none());
            assert!(
                compilation
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "cycle_without_delay")
            );
        } else {
            let compiled = compilation.compiled.unwrap();
            for executable in &compiled.exec_nodes {
                let resolved = &compilation.resolved.nodes[&executable.authored_id];
                for (socket, value_type) in executable.input_sockets.iter().zip(&executable.input_types) {
                    assert_eq!(&resolved.signature.inputs[socket].value_type, value_type);
                }
                for (socket, value_type) in executable.output_sockets.iter().zip(&executable.output_types) {
                    assert_eq!(&resolved.signature.outputs[socket].value_type, value_type);
                }
            }
        }
    }
}
