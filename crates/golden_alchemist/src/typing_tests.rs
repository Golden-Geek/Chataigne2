use crate::{
    ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeSignature, ANodeTypeId, AlchemistGraph, ExecutionKind,
    FacetId, InputSocketDecl, OutputSocketDecl, RuntimeValue, SignatureCtx, StableRef, TriggerValue, TypeBindingSource,
    TypeBindings, TypeConstraint, TypeSolveCtx, TypeVar, ValueStorageKind, ValueTypeDescriptor, ValueTypeId,
    ValueTypeRegistry, primitive_node_registry, solve_types,
};

fn solve(graph: &AlchemistGraph, value_types: &ValueTypeRegistry, nodes: &ANodeRegistry) -> crate::TypeSolveResult {
    solve_types(
        graph,
        &TypeSolveCtx {
            value_types,
            nodes,
            properties: None,
        },
    )
}

fn constant(value: RuntimeValue) -> ANodeInstance {
    let mut node = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    node.config.set("value", value);
    node
}

#[derive(Clone, Copy)]
struct NodeSocketSpec {
    type_id: &'static str,
    source_input: &'static str,
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
}

fn numeric_value_types() -> &'static [&'static str] {
    &["int", "float", "vec2", "vec3", "color"]
}

fn primitive_value_types() -> &'static [&'static str] {
    &[
        "unit", "bool", "trigger", "int", "float", "string", "vec2", "vec3", "color", "duration",
    ]
}

fn runtime_value(value_type: &str) -> RuntimeValue {
    match value_type {
        "unit" => RuntimeValue::Unit,
        "bool" => RuntimeValue::Bool(true),
        "trigger" => RuntimeValue::Trigger(TriggerValue::default()),
        "int" => RuntimeValue::Int(1),
        "float" => RuntimeValue::Float(1.0),
        "string" => RuntimeValue::String("1".into()),
        "vec2" => RuntimeValue::Vec2([1.0, 2.0]),
        "vec3" => RuntimeValue::Vec3([1.0, 2.0, 3.0]),
        "color" => RuntimeValue::Color(crate::ColorValue {
            red: 1.0,
            green: 0.5,
            blue: 0.25,
            alpha: 1.0,
        }),
        "duration" => RuntimeValue::Duration(std::time::Duration::from_secs(1)),
        _ => panic!("unsupported test value type `{value_type}`"),
    }
}

fn assert_resolved_socket_types(
    result: &crate::TypeSolveResult,
    node: crate::ANodeId,
    inputs: &[&str],
    outputs: &[&str],
    expected_type: &str,
) {
    let signature = &result.graph.nodes[&node].signature;
    let expected = Some(ValueTypeId::new(expected_type));
    for input in inputs {
        assert_eq!(
            signature.inputs[&crate::SocketId::new(*input)].value_type,
            expected,
            "input `{input}` should resolve to `{expected_type}`"
        );
    }
    for output in outputs {
        assert_eq!(
            signature.outputs[&crate::SocketId::new(*output)].value_type,
            expected,
            "output `{output}` should resolve to `{expected_type}`"
        );
    }
}

#[test]
fn math_defaults_to_float() {
    let mut graph = AlchemistGraph::new();
    let add = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("math"), "Math"))
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
fn vec3_connection_reshapes_math_inputs_and_output() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Vec3([1.0, 2.0, 3.0]))).unwrap();
    let add = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("math"), "Math"))
        .unwrap();
    graph
        .connect(
            crate::OutputSocketRef::new(source, "value"),
            crate::InputSocketRef::new(add, "value1"),
        )
        .unwrap();

    let result = solve(
        &graph,
        &ValueTypeRegistry::with_primitives(),
        &primitive_node_registry(),
    );
    let signature = &result.graph.nodes[&add].signature;

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    for socket in ["value1", "value2"] {
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
fn first_generic_input_decides_numeric_node_type() {
    let mut graph = AlchemistGraph::new();
    let vec3_source = graph.add_node(constant(RuntimeValue::Vec3([1.0, 2.0, 3.0]))).unwrap();
    let float_source = graph.add_node(constant(RuntimeValue::Float(1.0))).unwrap();
    let add = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("math"), "Math"))
        .unwrap();
    graph
        .connect(
            crate::OutputSocketRef::new(vec3_source, "value"),
            crate::InputSocketRef::new(add, "value2"),
        )
        .unwrap();
    graph
        .connect(
            crate::OutputSocketRef::new(float_source, "value"),
            crate::InputSocketRef::new(add, "value1"),
        )
        .unwrap();

    let result = solve(
        &graph,
        &ValueTypeRegistry::with_primitives(),
        &primitive_node_registry(),
    );

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    assert_resolved_socket_types(&result, add, &["value1", "value2"], &["result"], "float");
}

#[test]
fn primitive_registry_allows_runtime_supported_conversions() {
    let value_types = ValueTypeRegistry::with_primitives();
    let targets = [
        "unit", "bool", "int", "float", "string", "vec2", "vec3", "color", "duration",
    ];

    for source in primitive_value_types() {
        for target in targets {
            if *source == "trigger" && target == "trigger" {
                continue;
            }
            assert!(
                value_types.can_convert_automatically(&ValueTypeId::new(*source), &ValueTypeId::new(target)),
                "`{source}` should automatically convert to `{target}`"
            );
        }
    }
}

#[test]
fn numeric_node_keeps_supported_type_for_convertible_non_numeric_input() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::String("12.5".into()))).unwrap();
    let add = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("math"), "Math"))
        .unwrap();
    graph
        .connect(
            crate::OutputSocketRef::new(source, "value"),
            crate::InputSocketRef::new(add, "value1"),
        )
        .unwrap();

    let result = solve(
        &graph,
        &ValueTypeRegistry::with_primitives(),
        &primitive_node_registry(),
    );

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    assert_resolved_socket_types(&result, add, &["value1", "value2"], &["result"], "float");
}

#[test]
fn numeric_generic_nodes_infer_each_supported_connected_type() {
    let specs = [
        NodeSocketSpec {
            type_id: "math",
            source_input: "value1",
            inputs: &["value1", "value2"],
            outputs: &["result"],
        },
        NodeSocketSpec {
            type_id: "one_minus",
            source_input: "value",
            inputs: &["value"],
            outputs: &["result"],
        },
        NodeSocketSpec {
            type_id: "inverse",
            source_input: "value",
            inputs: &["value"],
            outputs: &["result"],
        },
        NodeSocketSpec {
            type_id: "negate",
            source_input: "value",
            inputs: &["value"],
            outputs: &["result"],
        },
    ];

    for spec in specs {
        for value_type in numeric_value_types() {
            let mut graph = AlchemistGraph::new();
            let source = graph.add_node(constant(runtime_value(value_type))).unwrap();
            let target = graph
                .add_node(ANodeInstance::new(ANodeTypeId::new(spec.type_id), spec.type_id))
                .unwrap();
            graph
                .connect(
                    crate::OutputSocketRef::new(source, "value"),
                    crate::InputSocketRef::new(target, spec.source_input),
                )
                .unwrap();

            let result = solve(
                &graph,
                &ValueTypeRegistry::with_primitives(),
                &primitive_node_registry(),
            );

            assert!(!result.has_errors(), "{:?}", result.diagnostics);
            assert_resolved_socket_types(&result, target, spec.inputs, spec.outputs, value_type);
        }
    }
}

#[test]
fn open_generic_nodes_infer_each_connected_primitive_type() {
    let specs = [
        NodeSocketSpec {
            type_id: "compare",
            source_input: "left",
            inputs: &["left", "right"],
            outputs: &[],
        },
        NodeSocketSpec {
            type_id: "delay_one_tick",
            source_input: "value",
            inputs: &["value"],
            outputs: &["value"],
        },
    ];

    for spec in specs {
        for value_type in primitive_value_types() {
            let mut graph = AlchemistGraph::new();
            let source = graph.add_node(constant(runtime_value(value_type))).unwrap();
            let target = graph
                .add_node(ANodeInstance::new(ANodeTypeId::new(spec.type_id), spec.type_id))
                .unwrap();
            graph
                .connect(
                    crate::OutputSocketRef::new(source, "value"),
                    crate::InputSocketRef::new(target, spec.source_input),
                )
                .unwrap();

            let result = solve(
                &graph,
                &ValueTypeRegistry::with_primitives(),
                &primitive_node_registry(),
            );

            assert!(!result.has_errors(), "{:?}", result.diagnostics);
            assert_resolved_socket_types(&result, target, spec.inputs, spec.outputs, value_type);
        }
    }
}

#[test]
fn forced_float_accepts_vec3_connection_with_coercion() {
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(constant(RuntimeValue::Vec3([1.0, 2.0, 3.0]))).unwrap();
    let mut add_node = ANodeInstance::new(ANodeTypeId::new("math"), "Math");
    add_node.forced_type_bindings.insert(
        TypeVar::new("TNumeric"),
        ValueTypeId::new("float"),
        TypeBindingSource::ForcedByUser,
    );
    let add = graph.add_node(add_node).unwrap();
    graph
        .connect(
            crate::OutputSocketRef::new(source, "value"),
            crate::InputSocketRef::new(add, "value1"),
        )
        .unwrap();

    let result = solve(
        &graph,
        &ValueTypeRegistry::with_primitives(),
        &primitive_node_registry(),
    );

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    let signature = &result.graph.nodes[&add].signature;
    for socket in ["value1", "value2"] {
        assert_eq!(
            signature.inputs[&crate::SocketId::new(socket)].value_type,
            Some(ValueTypeId::new("float"))
        );
    }
    assert_eq!(
        signature.outputs[&crate::SocketId::new("result")].value_type,
        Some(ValueTypeId::new("float"))
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
