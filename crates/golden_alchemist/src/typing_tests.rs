use crate::{
    ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeSignature, ANodeTypeId, AlchemistGraph, ExecutionKind,
    FacetId, InputSocketDecl, OutputSocketDecl, RuntimeValue, SignatureCtx, StableRef, TypeBindingSource, TypeBindings,
    TypeConstraint, TypeSolveCtx, TypeVar, ValueStorageKind, ValueTypeDescriptor, ValueTypeId, ValueTypeRegistry,
    primitive_node_registry, solve_types,
};

fn solve(graph: &AlchemistGraph, value_types: &ValueTypeRegistry, nodes: &ANodeRegistry) -> crate::TypeSolveResult {
    solve_types(graph, &TypeSolveCtx { value_types, nodes })
}

fn constant(value: RuntimeValue) -> ANodeInstance {
    let mut node = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    node.config.set("value", value);
    node
}

#[test]
fn add_defaults_to_float() {
    let mut graph = AlchemistGraph::new();
    let add = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("add"), "Add"))
        .unwrap();
    let result = solve(
        &graph,
        &ValueTypeRegistry::with_primitives(),
        &primitive_node_registry(),
    );

    assert!(!result.has_errors());
    assert_eq!(
        result.graph.nodes[&add].signature.outputs[&crate::SocketId::new("result")].value_type,
        Some(ValueTypeId::new("float"))
    );
}

#[test]
fn vec3_connection_reshapes_add_inputs_and_output() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Vec3([1.0, 2.0, 3.0]))).unwrap();
    let add = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("add"), "Add"))
        .unwrap();
    graph
        .connect(
            crate::OutputSocketRef::new(source, "value"),
            crate::InputSocketRef::new(add, "a"),
        )
        .unwrap();

    let result = solve(
        &graph,
        &ValueTypeRegistry::with_primitives(),
        &primitive_node_registry(),
    );
    let signature = &result.graph.nodes[&add].signature;

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    for socket in ["a", "b"] {
        assert_eq!(
            signature.inputs[&crate::SocketId::new(socket)].value_type,
            Some(ValueTypeId::new("vec3"))
        );
    }
    assert_eq!(
        signature.outputs[&crate::SocketId::new("result")].value_type,
        Some(ValueTypeId::new("vec3"))
    );
}

#[test]
fn forced_float_rejects_vec3_connection() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Vec3([1.0, 2.0, 3.0]))).unwrap();
    let mut add_node = ANodeInstance::new(ANodeTypeId::new("add"), "Add");
    add_node.forced_type_bindings.insert(
        TypeVar::new("TNumeric"),
        ValueTypeId::new("float"),
        TypeBindingSource::ForcedByUser,
    );
    let add = graph.add_node(add_node).unwrap();
    graph
        .connect(
            crate::OutputSocketRef::new(source, "value"),
            crate::InputSocketRef::new(add, "a"),
        )
        .unwrap();

    let result = solve(
        &graph,
        &ValueTypeRegistry::with_primitives(),
        &primitive_node_registry(),
    );

    assert!(result.has_errors());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "type_mismatch")
    );
}

struct FacetSink;

impl ANodeDeclaration for FacetSink {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new("test.facet_sink")
    }

    fn label(&self) -> &'static str {
        "Facet Sink"
    }

    fn category(&self) -> &'static str {
        "Tests"
    }

    fn execution_kind(&self) -> ExecutionKind {
        ExecutionKind::Pure
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        ANodeSignature {
            inputs: vec![InputSocketDecl::new(
                "target",
                "Target",
                TypeConstraint::Facet(FacetId::new("command_target")),
            )],
            outputs: vec![OutputSocketDecl::new(
                "accepted",
                "Accepted",
                TypeConstraint::Exact(ValueTypeId::new("bool")),
            )],
            ..ANodeSignature::default()
        }
    }
}

#[test]
fn facet_socket_accepts_registered_app_value() {
    let module_type = ValueTypeId::new("example.module");
    let mut value_types = ValueTypeRegistry::with_primitives();
    value_types
        .register(
            ValueTypeDescriptor::new(module_type.clone(), "Module", ValueStorageKind::StableRef, {
                let module_type = module_type.clone();
                move || RuntimeValue::Ref(StableRef::new(module_type.clone(), "default"))
            })
            .with_facets([FacetId::new("command_target")]),
        )
        .unwrap();
    let mut nodes = primitive_node_registry();
    nodes.register(FacetSink).unwrap();
    let mut graph = AlchemistGraph::new();
    let source = graph
        .add_node(constant(RuntimeValue::Ref(StableRef::new(module_type, "module-1"))))
        .unwrap();
    let sink = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("test.facet_sink"), "Sink"))
        .unwrap();
    graph
        .connect(
            crate::OutputSocketRef::new(source, "value"),
            crate::InputSocketRef::new(sink, "target"),
        )
        .unwrap();

    let result = solve(&graph, &value_types, &nodes);

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    assert_eq!(
        result.graph.nodes[&sink].signature.inputs[&crate::SocketId::new("target")].value_type,
        Some(ValueTypeId::new("example.module"))
    );
}
