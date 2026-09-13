use golden_engine::node::{Curve, CurveEasing, CurveKey};
use std::time::Duration;

use crate::test_support::TestGraph;
use crate::{
    ANodeInstance, ANodeTypeId, AlchemistRuntime, CompileCtx, DebugCaptureMode, EvaluationCtx, InputSocketRef,
    OutputSocketRef, RuntimeOutput, RuntimeRegistries, RuntimeValue, ValueTypeRegistry, compile_graph,
    curve_config_value, primitive_node_registry,
};

fn node(kind: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(kind), kind)
}

fn evaluate(graph: &TestGraph) -> RuntimeOutput {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compiled = compile_graph(
        &graph.to_document(),
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    AlchemistRuntime::new(compiled.compiled.unwrap()).evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick: 1,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &Default::default(),
            registries: &RuntimeRegistries {
                value_types: &value_types,
            },
        },
        DebugCaptureMode::All { history_len: 8 },
    )
}

fn unary(kind: &str, value: RuntimeValue) -> RuntimeOutput {
    let mut graph = TestGraph::new();
    let mut source = node("constant");
    source.config.set("value", value);
    let source = graph.add_node(source).unwrap();
    let target = graph.add_node(node(kind)).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(target, "value"),
        )
        .unwrap();
    evaluate(&graph)
}

#[test]
fn explicit_scalar_conversion_matrix_and_parse_errors() {
    for (kind, source, expected) in [
        (
            "convert_to_int",
            RuntimeValue::String("42".into()),
            RuntimeValue::Int(42),
        ),
        (
            "convert_to_float",
            RuntimeValue::String("2.5".into()),
            RuntimeValue::Float(2.5),
        ),
        (
            "convert_to_bool",
            RuntimeValue::String("true".into()),
            RuntimeValue::Bool(true),
        ),
        (
            "convert_to_string",
            RuntimeValue::Int(17),
            RuntimeValue::String("17".into()),
        ),
    ] {
        let output = unary(kind, source);
        assert!(output.diagnostics.is_empty(), "{kind}: {:?}", output.diagnostics);
        assert!(
            output.debug_samples.iter().any(|sample| sample.value == expected),
            "{kind}"
        );
    }
    for (kind, source, message) in [
        ("convert_to_int", RuntimeValue::String("4.2".into()), "valid integer"),
        ("convert_to_float", RuntimeValue::String("oops".into()), "valid float"),
        ("convert_to_bool", RuntimeValue::String("maybe".into()), "valid boolean"),
        (
            "convert_to_int",
            RuntimeValue::Float(9_223_372_036_854_775_808.0),
            "outside the integer range",
        ),
    ] {
        let output = unary(kind, source);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(message))
        );
    }
}

#[test]
fn pack_and_extract_vec2_preserve_component_order() {
    let mut graph = TestGraph::new();
    let sources = [3.0, 4.0].map(|value| {
        let mut constant = node("constant");
        constant.config.set("value", RuntimeValue::Float(value));
        graph.add_node(constant).unwrap()
    });
    let pack = graph.add_node(node("pack_vec2")).unwrap();
    let extract = graph.add_node(node("extract_vec2")).unwrap();
    for (source, socket) in sources.into_iter().zip(["x", "y"]) {
        graph
            .connect(OutputSocketRef::new(source, "value"), InputSocketRef::new(pack, socket))
            .unwrap();
    }
    graph
        .connect(
            OutputSocketRef::new(pack, "value"),
            InputSocketRef::new(extract, "value"),
        )
        .unwrap();
    let output = evaluate(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == pack && sample.value == RuntimeValue::Vec2([3.0, 4.0]))
    );
    for (socket, value) in [("x", 3.0), ("y", 4.0)] {
        assert!(output.debug_samples.iter().any(|sample| {
            sample.author_node_id == extract
                && sample.output_socket.as_str() == socket
                && sample.value == RuntimeValue::Float(value)
        }));
    }
}

#[test]
fn explicit_compound_conversion_preserves_components_and_sets_missing_components() {
    let mut graph = TestGraph::new();
    let mut source = node("constant");
    source.config.set("value", RuntimeValue::Vec2([0.25, 0.5]));
    let source = graph.add_node(source).unwrap();
    let vec3 = graph.add_node(node("convert_compound")).unwrap();
    let mut color_node = node("convert_compound");
    color_node.config.set("target", RuntimeValue::String("color".into()));
    let color = graph.add_node(color_node).unwrap();
    let mut vec2_node = node("convert_compound");
    vec2_node.config.set("target", RuntimeValue::String("vec2".into()));
    let vec2 = graph.add_node(vec2_node).unwrap();
    for (from, to, socket) in [
        (source, vec3, "value"),
        (vec3, color, "result"),
        (color, vec2, "result"),
    ] {
        graph
            .connect(OutputSocketRef::new(from, socket), InputSocketRef::new(to, "value"))
            .unwrap();
    }
    let output = evaluate(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == vec3 && sample.value == RuntimeValue::Vec3([0.25, 0.5, 0.0]))
    );
    assert!(output.debug_samples.iter().any(|sample| sample.author_node_id == color
        && sample.value
            == RuntimeValue::Color(crate::ColorValue {
                red: 0.25,
                green: 0.5,
                blue: 0.0,
                alpha: 1.0,
            })));
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == vec2 && sample.value == RuntimeValue::Vec2([0.25, 0.5]))
    );
}

#[test]
fn curve_remap_uses_golden_curve_sampling_and_rejects_invalid_resources() {
    let mut graph = TestGraph::new();
    let mut source = node("constant");
    source.config.set("value", RuntimeValue::Float(0.5));
    let source = graph.add_node(source).unwrap();
    let mut remap = node("curve_remap");
    remap.config.set(
        "curve",
        curve_config_value(&Curve::new(vec![
            CurveKey::new(0.0, 10.0, CurveEasing::Linear),
            CurveKey::new(1.0, 20.0, CurveEasing::Linear),
        ])),
    );
    let remap = graph.add_node(remap).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(remap, "value"),
        )
        .unwrap();
    let output = evaluate(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == remap && sample.value == RuntimeValue::Float(15.0))
    );

    let mut invalid = TestGraph::new();
    let mut remap = node("curve_remap");
    remap.config.set("curve", RuntimeValue::String("not a curve".into()));
    invalid.add_node(remap).unwrap();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compiled = compile_graph(
        &invalid.to_document(),
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "invalid_curve_resource")
    );
}

#[test]
fn malformed_gradient_and_non_finite_compound_inputs_diagnose() {
    let mut invalid = TestGraph::new();
    let mut sampler = node("gradient_sampler");
    sampler.config.set(
        "gradient",
        RuntimeValue::Array(vec![RuntimeValue::String("bad stop".into())]),
    );
    invalid.add_node(sampler).unwrap();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let compiled = compile_graph(
        &invalid.to_document(),
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "invalid_gradient_resource")
    );

    let output = unary("convert_compound", RuntimeValue::Vec2([f64::NAN, 0.0]));
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("finite components"))
    );
}
