use std::time::Duration;

use crate::test_support::TestGraph;
use crate::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistRuntime, ColorValue, CompileCtx, DebugCaptureMode, EvaluationCtx,
    InputSocketRef, OutputSocketRef, RuntimeOutput, RuntimeRegistries, RuntimeValue, ValueTypeRegistry, compile_graph,
    primitive_node_registry,
};

fn node(type_id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(type_id), type_id)
}

fn constant(value: RuntimeValue) -> ANodeInstance {
    let mut node = node("constant");
    node.config.set("value", value);
    node
}

fn runtime(graph: &TestGraph) -> AlchemistRuntime {
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
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    AlchemistRuntime::new(result.compiled.unwrap())
}

fn evaluate(runtime: &mut AlchemistRuntime) -> RuntimeOutput {
    let value_types = ValueTypeRegistry::with_primitives();
    runtime.evaluate_with_capture_mode(
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
    )
}

fn evaluate_binary_math(operator: &str, left: RuntimeValue, right: RuntimeValue) -> (RuntimeOutput, ANodeId) {
    let mut graph = TestGraph::new();
    let left = graph.add_node(constant(left)).unwrap();
    let right = graph.add_node(constant(right)).unwrap();
    let mut math = node("math");
    math.config.set("operator", RuntimeValue::String(operator.into()));
    let math = graph.add_node(math).unwrap();
    graph
        .connect(OutputSocketRef::new(left, "value"), InputSocketRef::new(math, "value1"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(right, "value"),
            InputSocketRef::new(math, "value2"),
        )
        .unwrap();
    let mut runtime = runtime(&graph);
    (evaluate(&mut runtime), math)
}

#[test]
fn math_operator_matrix_covers_every_mode_numeric_shape_and_zero_error() {
    for (operator, expected) in [
        ("add", RuntimeValue::Float(8.0)),
        ("subtract", RuntimeValue::Float(4.0)),
        ("multiply", RuntimeValue::Float(12.0)),
        ("divide", RuntimeValue::Float(3.0)),
        ("modulo", RuntimeValue::Float(0.0)),
    ] {
        let (output, math) = evaluate_binary_math(operator, RuntimeValue::Float(6.0), RuntimeValue::Float(2.0));
        assert!(output.diagnostics.is_empty(), "{operator}: {:?}", output.diagnostics);
        assert!(
            output.debug_samples.iter().any(|sample| {
                sample.author_node_id == math && sample.output_socket.as_str() == "result" && sample.value == expected
            }),
            "{operator}: {:?}",
            output.debug_samples
        );
    }

    for (left, right, expected) in [
        (RuntimeValue::Int(2), RuntimeValue::Int(3), RuntimeValue::Int(5)),
        (
            RuntimeValue::Vec2([1.0, 2.0]),
            RuntimeValue::Vec2([3.0, 4.0]),
            RuntimeValue::Vec2([4.0, 6.0]),
        ),
        (
            RuntimeValue::Vec3([1.0, 2.0, 3.0]),
            RuntimeValue::Vec3([4.0, 5.0, 6.0]),
            RuntimeValue::Vec3([5.0, 7.0, 9.0]),
        ),
        (
            RuntimeValue::Color(ColorValue {
                red: 0.1,
                green: 0.2,
                blue: 0.3,
                alpha: 0.4,
            }),
            RuntimeValue::Color(ColorValue {
                red: 0.4,
                green: 0.3,
                blue: 0.2,
                alpha: 0.1,
            }),
            RuntimeValue::Color(ColorValue {
                red: 0.5,
                green: 0.5,
                blue: 0.5,
                alpha: 0.5,
            }),
        ),
    ] {
        let (output, math) = evaluate_binary_math("add", left, right);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert!(
            output.debug_samples.iter().any(|sample| {
                sample.author_node_id == math && sample.output_socket.as_str() == "result" && sample.value == expected
            }),
            "{:?}",
            output.debug_samples
        );
    }

    for (operator, message) in [
        ("divide", "divide input cannot be zero"),
        ("modulo", "modulo input cannot be zero"),
    ] {
        let (output, _) = evaluate_binary_math(operator, RuntimeValue::Float(6.0), RuntimeValue::Float(0.0));
        assert_eq!(output.diagnostics.len(), 1, "{operator}: {:?}", output.diagnostics);
        assert!(
            output.diagnostics[0].message.contains(message),
            "{operator}: {:?}",
            output.diagnostics
        );
    }
}

mod pure_nodes {
    use super::*;
    use crate::{SocketId, TriggerValue};

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
        let mut runtime = runtime(&graph);
        (evaluate(&mut runtime), target)
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

    fn assert_float_output(
        type_id: &str,
        config: &[(&str, RuntimeValue)],
        inputs: &[(&str, RuntimeValue)],
        expected: f64,
    ) {
        let (output, target) = evaluate_node(type_id, config, inputs);
        assert!(output.diagnostics.is_empty(), "{type_id}: {:?}", output.diagnostics);
        let RuntimeValue::Float(actual) = output_value(&output, target, "result") else {
            panic!("{type_id} did not produce a float Result");
        };
        assert!(
            (actual - expected).abs() <= 1.0e-12,
            "{type_id}: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn function_matrix_covers_every_mode_dynamic_arity_and_non_finite_domains() {
        for (function, value, expected) in [
            ("sqrt", 9.0, 3.0),
            ("log", std::f64::consts::E, 1.0),
            ("log10", 100.0, 2.0),
            ("exp", 1.0, std::f64::consts::E),
            ("abs", -2.5, 2.5),
            ("floor", 2.9, 2.0),
            ("ceil", 2.1, 3.0),
            ("round", 2.5, 3.0),
            ("sin", std::f64::consts::FRAC_PI_2, 1.0),
            ("cos", std::f64::consts::PI, -1.0),
            ("tan", std::f64::consts::FRAC_PI_4, 1.0),
            ("asin", 1.0, std::f64::consts::FRAC_PI_2),
            ("acos", 0.0, std::f64::consts::FRAC_PI_2),
            ("atan", 1.0, std::f64::consts::FRAC_PI_4),
        ] {
            assert_float_output(
                "function",
                &[("function", RuntimeValue::String(function.into()))],
                &[("value", RuntimeValue::Float(value))],
                expected,
            );
        }
        assert_float_output(
            "function",
            &[("function", RuntimeValue::String("atan2".into()))],
            &[("y", RuntimeValue::Float(1.0)), ("x", RuntimeValue::Float(-1.0))],
            3.0 * std::f64::consts::FRAC_PI_4,
        );

        for (function, value, expected_nan, expected_infinite) in
            [("sqrt", -1.0, true, false), ("log", 0.0, false, true)]
        {
            let (output, target) = evaluate_node(
                "function",
                &[("function", RuntimeValue::String(function.into()))],
                &[("value", RuntimeValue::Float(value))],
            );
            assert!(output.diagnostics.is_empty(), "{function}: {:?}", output.diagnostics);
            let RuntimeValue::Float(actual) = output_value(&output, target, "result") else {
                panic!("Function did not produce a float Result");
            };
            assert_eq!(actual.is_nan(), expected_nan, "{function}: {actual}");
            assert_eq!(actual.is_infinite(), expected_infinite, "{function}: {actual}");
        }
    }

    #[test]
    fn string_node_matrix_covers_formatting_joining_and_split_policies() {
        let (output, concatenate) = evaluate_node(
            "concatenate",
            &[
                ("num_inputs", RuntimeValue::Int(3)),
                ("prefix", RuntimeValue::String("<".into())),
                ("suffix", RuntimeValue::String(">".into())),
                ("separator", RuntimeValue::String("|".into())),
            ],
            &[
                ("part1", RuntimeValue::String("alpha".into())),
                ("part2", RuntimeValue::String("beta".into())),
                ("part3", RuntimeValue::String("gamma".into())),
            ],
        );
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(
            output_value(&output, concatenate, "result"),
            RuntimeValue::String("<alpha|beta|gamma>".into())
        );

        for (format, decimals, input, expected) in [
            ("decimal", 2, RuntimeValue::Float(12.3456), "12.35"),
            ("hexadecimal", 3, RuntimeValue::Int(255), "0xFF"),
            ("time", 2, RuntimeValue::Float(3661.25), "01:01:01.25"),
            ("time", 2, RuntimeValue::Float(-1.0), "00:00:00.00"),
            ("compact", 1, RuntimeValue::Vec2([1.2, 2.3]), "[1.2,2.3]"),
        ] {
            let (output, convert) = evaluate_node(
                "convert_to_string",
                &[
                    ("format", RuntimeValue::String(format.into())),
                    ("decimals", RuntimeValue::Int(decimals)),
                ],
                &[("value", input)],
            );
            assert!(output.diagnostics.is_empty(), "{format}: {:?}", output.diagnostics);
            assert_eq!(
                output_value(&output, convert, "result"),
                RuntimeValue::String(expected.into()),
                "{format}"
            );
        }

        for (separator, trim, omit_empty, input, expected) in [
            (
                ",",
                true,
                true,
                " a, ,b,,",
                vec![RuntimeValue::String("a".into()), RuntimeValue::String("b".into())],
            ),
            (
                ",",
                false,
                false,
                "a,,b",
                vec![
                    RuntimeValue::String("a".into()),
                    RuntimeValue::String("".into()),
                    RuntimeValue::String("b".into()),
                ],
            ),
            (
                "",
                false,
                false,
                "éA",
                vec![RuntimeValue::String("é".into()), RuntimeValue::String("A".into())],
            ),
        ] {
            let (output, split) = evaluate_node(
                "split",
                &[
                    ("separator", RuntimeValue::String(separator.into())),
                    ("trim", RuntimeValue::Bool(trim)),
                    ("omit_empty", RuntimeValue::Bool(omit_empty)),
                ],
                &[("value", RuntimeValue::String(input.into()))],
            );
            assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
            assert_eq!(output_value(&output, split, "values"), RuntimeValue::Array(expected));
        }
    }

    #[test]
    fn boolean_operation_matrix_covers_complete_two_input_truth_tables() {
        for operator in ["and", "or", "xor"] {
            for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
                let expected = match operator {
                    "and" => a && b,
                    "or" => a || b,
                    "xor" => a ^ b,
                    _ => unreachable!(),
                };
                let (output, operation) = evaluate_node(
                    "boolean_operation",
                    &[("operator", RuntimeValue::String(operator.into()))],
                    &[("a", RuntimeValue::Bool(a)), ("b", RuntimeValue::Bool(b))],
                );
                assert!(output.diagnostics.is_empty(), "{operator}: {:?}", output.diagnostics);
                assert_eq!(
                    output_value(&output, operation, "result"),
                    RuntimeValue::Bool(expected),
                    "{operator}({a}, {b})"
                );
            }
        }
    }

    #[test]
    fn compare_matrix_covers_every_comparator_and_open_generic_typing() {
        let red = RuntimeValue::Color(ColorValue {
            red: 1.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        });
        let black = RuntimeValue::Color(ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        });
        let cases = [
            ("equal", RuntimeValue::Bool(true), RuntimeValue::Bool(true), true),
            (
                "not_equal",
                RuntimeValue::String("left".into()),
                RuntimeValue::String("right".into()),
                true,
            ),
            ("greater", RuntimeValue::Float(3.0), RuntimeValue::Float(2.0), true),
            ("greater_or_equal", RuntimeValue::Int(3), RuntimeValue::Int(3), true),
            ("less", RuntimeValue::Float(2.0), RuntimeValue::Float(3.0), true),
            ("less_or_equal", RuntimeValue::Int(3), RuntimeValue::Int(3), true),
            (
                "longer",
                RuntimeValue::String("abcd".into()),
                RuntimeValue::String("xy".into()),
                true,
            ),
            (
                "shorter",
                RuntimeValue::String("a".into()),
                RuntimeValue::String("xy".into()),
                true,
            ),
            (
                "contains",
                RuntimeValue::String("alphabet".into()),
                RuntimeValue::String("pha".into()),
                true,
            ),
            ("brighter", red.clone(), black.clone(), true),
            ("darker", black, red, true),
        ];

        for (comparator, left, right, expected) in cases {
            let (output, compare) = evaluate_node(
                "compare",
                &[("comparator", RuntimeValue::String(comparator.into()))],
                &[("left", left), ("right", right)],
            );
            assert!(output.diagnostics.is_empty(), "{comparator}: {:?}", output.diagnostics);
            assert_eq!(
                output_value(&output, compare, "result"),
                RuntimeValue::Bool(expected),
                "{comparator}"
            );
        }
    }

    #[test]
    fn gate_matrix_preserves_trigger_identity_while_controlling_fired_state() {
        for (trigger, open, expected_fired) in [
            (TriggerValue::fired(42, 7), true, true),
            (TriggerValue::fired(42, 7), false, false),
            (TriggerValue::default(), true, false),
        ] {
            let (output, gate) = evaluate_node(
                "gate",
                &[],
                &[
                    ("trigger", RuntimeValue::Trigger(trigger)),
                    ("open", RuntimeValue::Bool(open)),
                ],
            );
            assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
            let RuntimeValue::Trigger(result) = output_value(&output, gate, "trigger") else {
                panic!("Gate did not produce a trigger");
            };
            assert_eq!(result.fired, expected_fired);
            assert_eq!(result.edge_id, trigger.edge_id);
            assert_eq!(result.logical_tick, trigger.logical_tick);
        }
    }
}

mod transforms {
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

    fn assert_close(actual: &RuntimeValue, expected: &RuntimeValue) {
        fn scalar(actual: f64, expected: f64) {
            assert!(
                (actual - expected).abs() <= 1.0e-12,
                "expected {expected}, got {actual}"
            );
        }

        match (actual, expected) {
            (RuntimeValue::Float(actual), RuntimeValue::Float(expected)) => scalar(*actual, *expected),
            (RuntimeValue::Vec2(actual), RuntimeValue::Vec2(expected)) => {
                for index in 0..2 {
                    scalar(actual[index], expected[index]);
                }
            }
            (RuntimeValue::Vec3(actual), RuntimeValue::Vec3(expected)) => {
                for index in 0..3 {
                    scalar(actual[index], expected[index]);
                }
            }
            (RuntimeValue::Color(actual), RuntimeValue::Color(expected)) => {
                scalar(actual.red, expected.red);
                scalar(actual.green, expected.green);
                scalar(actual.blue, expected.blue);
                scalar(actual.alpha, expected.alpha);
            }
            _ => assert_eq!(actual, expected),
        }
    }

    fn assert_result(type_id: &str, inputs: &[(&str, RuntimeValue)], expected: RuntimeValue) {
        let (output, target) = evaluate_node(type_id, &[], inputs);
        assert!(output.diagnostics.is_empty(), "{type_id}: {:?}", output.diagnostics);
        assert_close(&output_value(&output, target, "result"), &expected);
    }

    #[test]
    fn remap_and_clamp_cover_bounds_extrapolation_numeric_shapes_and_errors() {
        for (value, expected) in [(5.0, 0.0), (15.0, 2.0), (-5.0, -2.0)] {
            assert_result(
                "remap",
                &[
                    ("value", RuntimeValue::Float(value)),
                    ("in_min", RuntimeValue::Float(0.0)),
                    ("in_max", RuntimeValue::Float(10.0)),
                    ("out_min", RuntimeValue::Float(-1.0)),
                    ("out_max", RuntimeValue::Float(1.0)),
                ],
                RuntimeValue::Float(expected),
            );
        }

        let (output, _) = evaluate_node(
            "remap",
            &[],
            &[
                ("value", RuntimeValue::Float(3.0)),
                ("in_min", RuntimeValue::Float(2.0)),
                ("in_max", RuntimeValue::Float(2.0)),
                ("out_min", RuntimeValue::Float(0.0)),
                ("out_max", RuntimeValue::Float(1.0)),
            ],
        );
        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert!(output.diagnostics[0].message.contains("range cannot be zero"));

        for (value, minimum, maximum, expected) in [
            (
                RuntimeValue::Int(-2),
                RuntimeValue::Int(0),
                RuntimeValue::Int(5),
                RuntimeValue::Int(0),
            ),
            (
                RuntimeValue::Float(2.0),
                RuntimeValue::Float(0.0),
                RuntimeValue::Float(5.0),
                RuntimeValue::Float(2.0),
            ),
            (
                RuntimeValue::Vec2([-1.0, 7.0]),
                RuntimeValue::Vec2([0.0, 1.0]),
                RuntimeValue::Vec2([5.0, 6.0]),
                RuntimeValue::Vec2([0.0, 6.0]),
            ),
            (
                RuntimeValue::Vec3([-1.0, 2.0, 8.0]),
                RuntimeValue::Vec3([0.0, 1.0, 3.0]),
                RuntimeValue::Vec3([4.0, 5.0, 6.0]),
                RuntimeValue::Vec3([0.0, 2.0, 6.0]),
            ),
            (
                RuntimeValue::Color(ColorValue {
                    red: -0.1,
                    green: 0.3,
                    blue: 1.2,
                    alpha: 0.5,
                }),
                RuntimeValue::Color(ColorValue {
                    red: 0.0,
                    green: 0.2,
                    blue: 0.4,
                    alpha: 0.6,
                }),
                RuntimeValue::Color(ColorValue {
                    red: 1.0,
                    green: 0.8,
                    blue: 1.0,
                    alpha: 0.9,
                }),
                RuntimeValue::Color(ColorValue {
                    red: 0.0,
                    green: 0.3,
                    blue: 1.0,
                    alpha: 0.6,
                }),
            ),
        ] {
            assert_result(
                "clamp",
                &[("value", value), ("minimum", minimum), ("maximum", maximum)],
                expected,
            );
        }

        let (output, _) = evaluate_node(
            "clamp",
            &[],
            &[
                ("value", RuntimeValue::Float(2.0)),
                ("minimum", RuntimeValue::Float(3.0)),
                ("maximum", RuntimeValue::Float(1.0)),
            ],
        );
        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert!(output.diagnostics[0].message.contains("minimum cannot exceed maximum"));
    }

    #[test]
    fn unary_transform_matrix_covers_every_numeric_shape_and_inverse_zero_errors() {
        let cases = [
            (
                RuntimeValue::Int(2),
                RuntimeValue::Int(-1),
                RuntimeValue::Int(0),
                RuntimeValue::Int(-2),
            ),
            (
                RuntimeValue::Float(4.0),
                RuntimeValue::Float(-3.0),
                RuntimeValue::Float(0.25),
                RuntimeValue::Float(-4.0),
            ),
            (
                RuntimeValue::Vec2([2.0, 0.5]),
                RuntimeValue::Vec2([-1.0, 0.5]),
                RuntimeValue::Vec2([0.5, 2.0]),
                RuntimeValue::Vec2([-2.0, -0.5]),
            ),
            (
                RuntimeValue::Vec3([2.0, 4.0, -0.5]),
                RuntimeValue::Vec3([-1.0, -3.0, 1.5]),
                RuntimeValue::Vec3([0.5, 0.25, -2.0]),
                RuntimeValue::Vec3([-2.0, -4.0, 0.5]),
            ),
            (
                RuntimeValue::Color(ColorValue {
                    red: 0.25,
                    green: 0.5,
                    blue: 2.0,
                    alpha: 4.0,
                }),
                RuntimeValue::Color(ColorValue {
                    red: 0.75,
                    green: 0.5,
                    blue: -1.0,
                    alpha: -3.0,
                }),
                RuntimeValue::Color(ColorValue {
                    red: 4.0,
                    green: 2.0,
                    blue: 0.5,
                    alpha: 0.25,
                }),
                RuntimeValue::Color(ColorValue {
                    red: -0.25,
                    green: -0.5,
                    blue: -2.0,
                    alpha: -4.0,
                }),
            ),
        ];

        for (input, one_minus, inverse, negate) in cases {
            assert_result("one_minus", &[("value", input.clone())], one_minus);
            assert_result("inverse", &[("value", input.clone())], inverse);
            assert_result("negate", &[("value", input)], negate);
        }

        for input in [RuntimeValue::Float(0.0), RuntimeValue::Vec2([1.0, 0.0])] {
            let (output, _) = evaluate_node("inverse", &[], &[("value", input)]);
            assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
            assert!(output.diagnostics[0].message.contains("input cannot be zero"));
        }
    }

    #[test]
    fn geometry_transform_matrix_covers_pack_angles_and_both_coordinate_directions() {
        let (output, pack) = evaluate_node(
            "pack_vec3",
            &[],
            &[
                ("x", RuntimeValue::Float(-1.5)),
                ("y", RuntimeValue::Float(2.25)),
                ("z", RuntimeValue::Float(8.0)),
            ],
        );
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_close(
            &output_value(&output, pack, "value"),
            &RuntimeValue::Vec3([-1.5, 2.25, 8.0]),
        );

        for (mode, value, expected) in [
            ("degrees_to_radians", 180.0, std::f64::consts::PI),
            ("radians_to_degrees", std::f64::consts::FRAC_PI_2, 90.0),
        ] {
            let (output, angle) = evaluate_node(
                "angle_conversion",
                &[("mode", RuntimeValue::String(mode.into()))],
                &[("value", RuntimeValue::Float(value))],
            );
            assert!(output.diagnostics.is_empty(), "{mode}: {:?}", output.diagnostics);
            assert_close(&output_value(&output, angle, "result"), &RuntimeValue::Float(expected));
        }

        for (mode, value, expected) in [
            (
                "cartesian_to_polar",
                RuntimeValue::Vec2([3.0, 4.0]),
                RuntimeValue::Vec2([5.0, 4.0_f64.atan2(3.0)]),
            ),
            (
                "cartesian_to_polar",
                RuntimeValue::Vec2([0.0, 0.0]),
                RuntimeValue::Vec2([0.0, 0.0]),
            ),
            (
                "polar_to_cartesian",
                RuntimeValue::Vec2([2.0, std::f64::consts::FRAC_PI_2]),
                RuntimeValue::Vec2([0.0, 2.0]),
            ),
        ] {
            let (output, coordinate) = evaluate_node(
                "coordinate_system",
                &[("mode", RuntimeValue::String(mode.into()))],
                &[("value", value)],
            );
            assert!(output.diagnostics.is_empty(), "{mode}: {:?}", output.diagnostics);
            assert_close(&output_value(&output, coordinate, "result"), &expected);
        }
    }
}
