use std::time::Duration;

use crate::test_support::TestGraph;
use crate::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistRuntime, ColorValue, CompileCtx, DebugCaptureMode, EvaluationCtx,
    InputSocketRef, OutputSocketRef, RuntimeOutput, RuntimeRegistries, RuntimeValue, SocketId, ValueTypeRegistry,
    compile_graph, primitive_node_registry,
};

fn node(type_id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(type_id), type_id)
}

fn constant(value: RuntimeValue) -> ANodeInstance {
    let mut node = node("constant");
    node.config.set("value", value);
    node
}

fn evaluate_node(
    type_id: &str,
    config: &[(&str, RuntimeValue)],
    inputs: &[(&str, RuntimeValue)],
) -> (RuntimeOutput, ANodeId) {
    let mut graph = TestGraph::new();
    let mut target = node(type_id);
    for (field, value) in config {
        target.config.set(*field, value.clone());
    }
    let target = graph.add_node(target).unwrap();
    for (socket, value) in inputs {
        let source = graph.add_node(constant(value.clone())).unwrap();
        graph
            .connect(
                OutputSocketRef::new(source, "value"),
                InputSocketRef::new(target, *socket),
            )
            .unwrap();
    }
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let result = compile_graph(
        &graph.to_document(),
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!result.has_errors(), "{type_id}: {:?}", result.diagnostics);
    let mut runtime = AlchemistRuntime::new(result.compiled.unwrap());
    let output = runtime.evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick: 1,
            delta_time: Duration::from_millis(16),
            events: &[],
            inputs: &Default::default(),
            registries: &RuntimeRegistries {
                value_types: &value_types,
            },
        },
        DebugCaptureMode::All { history_len: 64 },
    );
    (output, target)
}

fn output_value(output: &RuntimeOutput, node: ANodeId, socket: &str) -> RuntimeValue {
    output
        .debug_samples
        .iter()
        .find(|sample| sample.author_node_id == node && sample.output_socket == SocketId::new(socket))
        .unwrap_or_else(|| panic!("missing sample for {node:?}.{socket}: {:?}", output.debug_samples))
        .value
        .clone()
}

fn assert_scalar(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-12,
        "expected {expected}, got {actual}"
    );
}

fn assert_color(actual: RuntimeValue, expected: ColorValue) {
    let RuntimeValue::Color(actual) = actual else {
        panic!("expected color, got {actual:?}");
    };
    assert_scalar(actual.red, expected.red);
    assert_scalar(actual.green, expected.green);
    assert_scalar(actual.blue, expected.blue);
    assert_scalar(actual.alpha, expected.alpha);
}

fn gradient_stop(position: f64, color: ColorValue, interpolation: &str) -> RuntimeValue {
    RuntimeValue::Array(vec![
        RuntimeValue::Float(position),
        RuntimeValue::Color(color),
        RuntimeValue::String(interpolation.into()),
    ])
}

#[test]
fn convert_to_color_matrix_covers_every_mode_and_channel_policy() {
    let cases = [
        (
            "rgba",
            ["r", "g", "b", "a"],
            [0.1, 0.2, 0.3, 0.4],
            ColorValue {
                red: 0.1,
                green: 0.2,
                blue: 0.3,
                alpha: 0.4,
            },
        ),
        (
            "hsva",
            ["h", "s", "v", "a"],
            [120.0, 1.0, 1.0, 0.5],
            ColorValue {
                red: 0.0,
                green: 1.0,
                blue: 0.0,
                alpha: 0.5,
            },
        ),
        (
            "hsla",
            ["h", "s", "l", "a"],
            [240.0, 1.0, 0.5, 0.75],
            ColorValue {
                red: 0.0,
                green: 0.0,
                blue: 1.0,
                alpha: 0.75,
            },
        ),
        (
            "cmyka",
            ["c", "m", "y", "k"],
            [0.2, 0.3, 0.4, 0.1],
            ColorValue {
                red: 0.72,
                green: 0.63,
                blue: 0.54,
                alpha: 1.0,
            },
        ),
    ];

    for (mode, sockets, values, expected) in cases {
        let inputs = sockets
            .into_iter()
            .zip(values)
            .map(|(socket, value)| (socket, RuntimeValue::Float(value)))
            .collect::<Vec<_>>();
        let (output, convert) = evaluate_node(
            "convert_to_color",
            &[("mode", RuntimeValue::String(mode.into()))],
            &inputs,
        );
        assert!(output.diagnostics.is_empty(), "{mode}: {:?}", output.diagnostics);
        assert_color(output_value(&output, convert, "color"), expected);
    }
}

#[test]
fn extract_color_matrix_covers_every_mode_and_roundtrip_channels() {
    let color = ColorValue {
        red: 0.2,
        green: 0.4,
        blue: 0.6,
        alpha: 0.8,
    };
    let cases = [
        ("rgba", ["r", "g", "b", "a"], [0.2, 0.4, 0.6, 0.8]),
        ("hsva", ["h", "s", "v", "a"], [210.0, 2.0 / 3.0, 0.6, 0.8]),
        ("hsla", ["h", "s", "l", "a"], [210.0, 0.5, 0.4, 0.8]),
        ("cmyka", ["c", "m", "y", "k"], [2.0 / 3.0, 1.0 / 3.0, 0.0, 0.4]),
    ];

    for (mode, sockets, expected) in cases {
        let (output, extract) = evaluate_node(
            "extract_color",
            &[("mode", RuntimeValue::String(mode.into()))],
            &[("color", RuntimeValue::Color(color))],
        );
        assert!(output.diagnostics.is_empty(), "{mode}: {:?}", output.diagnostics);
        for (socket, expected) in sockets.into_iter().zip(expected) {
            let RuntimeValue::Float(actual) = output_value(&output, extract, socket) else {
                panic!("{mode}.{socket} did not produce a float");
            };
            assert_scalar(actual, expected);
        }
    }
}

#[test]
fn gradient_sampler_covers_default_sorted_clamped_and_interpolation_modes() {
    for (position, expected) in [
        (-1.0, ColorValue::BLACK),
        (
            0.5,
            ColorValue {
                red: 0.5,
                green: 0.5,
                blue: 0.5,
                alpha: 1.0,
            },
        ),
        (
            2.0,
            ColorValue {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
                alpha: 1.0,
            },
        ),
    ] {
        let (output, sampler) = evaluate_node("gradient_sampler", &[], &[("position", RuntimeValue::Float(position))]);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_color(output_value(&output, sampler, "color"), expected);
    }

    let white = ColorValue {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };
    for (interpolation, position, expected) in [
        ("none", 0.75, ColorValue::BLACK),
        (
            "linear",
            0.25,
            ColorValue {
                red: 0.25,
                green: 0.25,
                blue: 0.25,
                alpha: 1.0,
            },
        ),
        (
            "smooth",
            0.25,
            ColorValue {
                red: 0.15625,
                green: 0.15625,
                blue: 0.15625,
                alpha: 1.0,
            },
        ),
    ] {
        let gradient = RuntimeValue::Array(vec![
            gradient_stop(1.5, white, "linear"),
            gradient_stop(-0.5, ColorValue::BLACK, interpolation),
        ]);
        let (output, sampler) = evaluate_node(
            "gradient_sampler",
            &[("gradient", gradient)],
            &[("position", RuntimeValue::Float(position))],
        );
        assert!(
            output.diagnostics.is_empty(),
            "{interpolation}: {:?}",
            output.diagnostics
        );
        assert_color(output_value(&output, sampler, "color"), expected);
    }

    let (output, sampler) = evaluate_node(
        "gradient_sampler",
        &[("gradient", RuntimeValue::Array(Vec::new()))],
        &[("position", RuntimeValue::Float(0.5))],
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_color(
        output_value(&output, sampler, "color"),
        ColorValue {
            red: 0.5,
            green: 0.5,
            blue: 0.5,
            alpha: 1.0,
        },
    );
}
