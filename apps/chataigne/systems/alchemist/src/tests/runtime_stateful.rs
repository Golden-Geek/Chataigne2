use std::time::Duration;

use crate::test_support::TestGraph;
use crate::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistGraphId, AlchemistRuntime, CompileCtx, DebugCaptureMode,
    DebugCaptureSink, EvaluationCtx, EvaluationFrame, RuntimeContextFrame, RuntimeInputSnapshot, RuntimeOutput,
    RuntimeRegistries, RuntimeValue, SocketId, TriggerValue, ValueTypeRegistry, compile_graph, evaluate_compiled_graph,
    formula_input_value_ref, primitive_node_registry,
};

struct RuntimeHarness {
    graph_id: AlchemistGraphId,
    node_id: ANodeId,
    runtime: AlchemistRuntime,
}

impl RuntimeHarness {
    fn new(type_id: &str, config: &[(&str, RuntimeValue)]) -> Self {
        let mut graph = TestGraph::new();
        let mut node = ANodeInstance::new(ANodeTypeId::new(type_id), type_id);
        for (field, value) in config {
            node.config.set(*field, value.clone());
        }
        let node_id = graph.add_node(node).unwrap();
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
        Self {
            graph_id: graph.id,
            node_id,
            runtime: AlchemistRuntime::new(result.compiled.unwrap()),
        }
    }

    fn evaluate(&mut self, logical_tick: u64, delta_time: Duration, values: &[(&str, RuntimeValue)]) -> RuntimeOutput {
        let mut inputs = RuntimeInputSnapshot::default();
        for (socket, value) in values {
            inputs.insert(
                formula_input_value_ref(self.graph_id, self.node_id, &SocketId::new(*socket)),
                value.clone(),
            );
        }
        let value_types = ValueTypeRegistry::with_primitives();
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let ctx = EvaluationCtx {
            logical_tick,
            delta_time,
            events: &[],
            inputs: &inputs,
            registries: &registries,
        };
        let context = RuntimeContextFrame::default_lane();
        let mut debug = DebugCaptureSink::new(DebugCaptureMode::All { history_len: 128 });
        evaluate_compiled_graph(
            &self.runtime.compiled,
            &mut self.runtime.memory,
            EvaluationFrame {
                ctx: &ctx,
                properties: &self.runtime.properties,
                context: &context,
                debug: &mut debug,
                force_process_unchanged_inputs: false,
                capture_unchanged_outputs: true,
            },
        )
    }

    fn output(&self, output: &RuntimeOutput, socket: &str) -> RuntimeValue {
        output
            .debug_samples
            .iter()
            .find(|sample| sample.author_node_id == self.node_id && sample.output_socket == SocketId::new(socket))
            .unwrap_or_else(|| {
                panic!(
                    "missing sample for {:?}.{socket}: {:?}",
                    self.node_id, output.debug_samples
                )
            })
            .value
            .clone()
    }
}

fn assert_float(actual: RuntimeValue, expected: f64) {
    let RuntimeValue::Float(actual) = actual else {
        panic!("expected float, got {actual:?}");
    };
    assert!(
        (actual - expected).abs() <= 1.0e-10,
        "expected {expected}, got {actual}"
    );
}

fn trigger(output: RuntimeValue) -> TriggerValue {
    let RuntimeValue::Trigger(trigger) = output else {
        panic!("expected trigger, got {output:?}");
    };
    trigger
}

#[test]
fn trigger_on_off_covers_edge_and_toggle_state_machines() {
    let dt = Duration::from_millis(16);
    let mut edge = RuntimeHarness::new("trigger_on_off", &[]);
    let first = edge.evaluate(1, dt, &[("value", RuntimeValue::Bool(false))]);
    assert!(!trigger(edge.output(&first, "on")).fired);
    assert!(!trigger(edge.output(&first, "off")).fired);
    let rising = edge.evaluate(2, dt, &[("value", RuntimeValue::Bool(true))]);
    assert!(trigger(edge.output(&rising, "on")).fired);
    assert!(!trigger(edge.output(&rising, "off")).fired);
    let falling = edge.evaluate(3, dt, &[("value", RuntimeValue::Bool(false))]);
    assert!(!trigger(edge.output(&falling, "on")).fired);
    assert!(trigger(edge.output(&falling, "off")).fired);

    let mut toggle = RuntimeHarness::new("trigger_on_off", &[("toggle", RuntimeValue::Bool(true))]);
    toggle.evaluate(1, dt, &[("value", RuntimeValue::Bool(false))]);
    let first_on = toggle.evaluate(2, dt, &[("value", RuntimeValue::Bool(true))]);
    assert!(trigger(toggle.output(&first_on, "on")).fired);
    toggle.evaluate(3, dt, &[("value", RuntimeValue::Bool(false))]);
    let second_on = toggle.evaluate(4, dt, &[("value", RuntimeValue::Bool(true))]);
    assert!(!trigger(toggle.output(&second_on, "on")).fired);
    assert!(trigger(toggle.output(&second_on, "off")).fired);
}

#[test]
fn counter_covers_accumulation_idle_and_reset_precedence() {
    let dt = Duration::from_millis(16);
    let idle = TriggerValue::default();
    let mut counter = RuntimeHarness::new("counter", &[]);
    let initial = counter.evaluate(
        1,
        dt,
        &[
            ("add", RuntimeValue::Trigger(idle)),
            ("amount", RuntimeValue::Float(2.0)),
            ("reset", RuntimeValue::Trigger(idle)),
        ],
    );
    assert_float(counter.output(&initial, "count"), 0.0);
    let added = counter.evaluate(
        2,
        dt,
        &[
            ("add", RuntimeValue::Trigger(TriggerValue::fired(1, 2))),
            ("amount", RuntimeValue::Float(2.0)),
            ("reset", RuntimeValue::Trigger(idle)),
        ],
    );
    assert_float(counter.output(&added, "count"), 2.0);
    let added_again = counter.evaluate(
        3,
        dt,
        &[
            ("add", RuntimeValue::Trigger(TriggerValue::fired(2, 3))),
            ("amount", RuntimeValue::Float(3.0)),
            ("reset", RuntimeValue::Trigger(idle)),
        ],
    );
    assert_float(counter.output(&added_again, "count"), 5.0);
    let reset = counter.evaluate(
        4,
        dt,
        &[
            ("add", RuntimeValue::Trigger(TriggerValue::fired(3, 4))),
            ("amount", RuntimeValue::Float(9.0)),
            ("reset", RuntimeValue::Trigger(TriggerValue::fired(4, 4))),
        ],
    );
    assert_float(counter.output(&reset, "count"), 0.0);
}

#[test]
fn delay_one_tick_bootstraps_then_emits_the_previous_value() {
    let dt = Duration::from_millis(16);
    let mut delay = RuntimeHarness::new("delay_one_tick", &[]);
    let first = delay.evaluate(1, dt, &[("value", RuntimeValue::Float(1.0))]);
    assert_float(delay.output(&first, "value"), 1.0);
    let second = delay.evaluate(2, dt, &[("value", RuntimeValue::Float(2.0))]);
    assert_float(delay.output(&second, "value"), 1.0);
    let third = delay.evaluate(3, dt, &[("value", RuntimeValue::Float(3.0))]);
    assert_float(delay.output(&third, "value"), 2.0);

    let mut unit_delay = RuntimeHarness::new("delay_one_tick", &[]);
    let unit = unit_delay.evaluate(1, dt, &[("value", RuntimeValue::Unit)]);
    assert_eq!(unit_delay.output(&unit, "value"), RuntimeValue::Unit);
    let after_unit = unit_delay.evaluate(2, dt, &[("value", RuntimeValue::Float(4.0))]);
    assert_eq!(unit_delay.output(&after_unit, "value"), RuntimeValue::Unit);
}

#[test]
fn speed_reports_first_sample_zero_and_windowed_derivatives() {
    let dt = Duration::from_millis(100);
    let mut speed = RuntimeHarness::new("speed", &[("window_seconds", RuntimeValue::Float(0.0))]);
    let first = speed.evaluate(1, dt, &[("value", RuntimeValue::Float(0.0))]);
    assert_float(speed.output(&first, "result"), 0.0);
    let second = speed.evaluate(2, dt, &[("value", RuntimeValue::Float(1.0))]);
    assert_float(speed.output(&second, "result"), 10.0);
    let third = speed.evaluate(3, dt, &[("value", RuntimeValue::Float(3.0))]);
    assert_float(speed.output(&third, "result"), 20.0);

    let mut windowed = RuntimeHarness::new("speed", &[("window_seconds", RuntimeValue::Float(0.2))]);
    windowed.evaluate(1, dt, &[("value", RuntimeValue::Float(0.0))]);
    let smoothed = windowed.evaluate(2, dt, &[("value", RuntimeValue::Float(1.0))]);
    assert_float(windowed.output(&smoothed, "result"), 5.0);
}

fn smooth_values(method: &str, config: &[(&str, RuntimeValue)], values: &[f64]) -> Vec<f64> {
    let mut owned_config = vec![("method", RuntimeValue::String(method.into()))];
    owned_config.extend_from_slice(config);
    let mut filter = RuntimeHarness::new("smooth_filter", &owned_config);
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let output = filter.evaluate(
                index as u64 + 1,
                Duration::from_millis(100),
                &[("value", RuntimeValue::Float(*value))],
            );
            let RuntimeValue::Float(value) = filter.output(&output, "result") else {
                panic!("Smooth Filter did not produce a float");
            };
            value
        })
        .collect()
}

#[test]
fn smooth_filter_matrix_covers_every_method_and_history_policy() {
    assert_eq!(
        smooth_values("sma", &[("window", RuntimeValue::Int(3))], &[1.0, 2.0, 9.0, 4.0]),
        vec![1.0, 1.5, 4.0, 5.0]
    );
    assert_eq!(
        smooth_values("median", &[("window", RuntimeValue::Int(3))], &[1.0, 100.0, 3.0, 4.0]),
        vec![1.0, 50.5, 3.0, 4.0]
    );
    let savitzky = smooth_values(
        "savitzky_golay",
        &[("window", RuntimeValue::Int(5))],
        &[1.0, 2.0, 3.0, 4.0, 5.0],
    );
    assert_float(RuntimeValue::Float(savitzky[4]), 3.0);

    let damping = smooth_values(
        "damping",
        &[
            ("mass", RuntimeValue::Float(1.0)),
            ("friction", RuntimeValue::Float(0.0)),
        ],
        &[10.0, 10.0],
    );
    assert_float(RuntimeValue::Float(damping[0]), 0.1);
    assert_float(RuntimeValue::Float(damping[1]), 0.299);

    let one_euro = smooth_values(
        "one_euro",
        &[
            ("min_cutoff", RuntimeValue::Float(1.0)),
            ("beta", RuntimeValue::Float(0.0)),
        ],
        &[10.0, 10.0],
    );
    let alpha = 0.1 / (1.0 / (2.0 * std::f64::consts::PI) + 0.1);
    assert_float(RuntimeValue::Float(one_euro[0]), 10.0 * alpha);
    assert_float(
        RuntimeValue::Float(one_euro[1]),
        10.0 * alpha + (10.0 - 10.0 * alpha) * alpha,
    );
}

#[test]
fn lfo_matrix_covers_every_shape_range_and_update_rate_hold() {
    for (shape, expected) in [
        ("sine", 1.0),
        ("triangle", 0.0),
        ("saw", -0.5),
        ("square", 1.0),
        ("pulse", -1.0),
    ] {
        let mut lfo = RuntimeHarness::new(
            "lfo",
            &[
                ("shape", RuntimeValue::String(shape.into())),
                ("frequency", RuntimeValue::Float(1.0)),
                ("update_rate", RuntimeValue::Float(0.0)),
                ("minimum", RuntimeValue::Float(-1.0)),
                ("maximum", RuntimeValue::Float(1.0)),
            ],
        );
        let output = lfo.evaluate(1, Duration::from_millis(250), &[]);
        assert_float(lfo.output(&output, "value"), expected);
    }

    let mut held = RuntimeHarness::new(
        "lfo",
        &[
            ("shape", RuntimeValue::String("saw".into())),
            ("frequency", RuntimeValue::Float(1.0)),
            ("update_rate", RuntimeValue::Float(2.0)),
            ("minimum", RuntimeValue::Float(0.0)),
            ("maximum", RuntimeValue::Float(1.0)),
        ],
    );
    let first = held.evaluate(1, Duration::from_millis(100), &[]);
    let first = held.output(&first, "value");
    let second = held.evaluate(2, Duration::from_millis(100), &[]);
    assert_eq!(held.output(&second, "value"), first);
}

fn noise_sequence(algorithm: &str) -> Vec<f64> {
    let mut noise = RuntimeHarness::new(
        "noise_generator",
        &[
            ("algorithm", RuntimeValue::String(algorithm.into())),
            ("seed", RuntimeValue::Int(42)),
            ("scale", RuntimeValue::Float(1.5)),
            ("octaves", RuntimeValue::Int(4)),
            ("persistence", RuntimeValue::Float(0.5)),
            ("lacunarity", RuntimeValue::Float(2.0)),
            ("jitter", RuntimeValue::Float(0.75)),
        ],
    );
    (1..=4)
        .map(|tick| {
            let output = noise.evaluate(
                tick,
                Duration::from_millis(100),
                &[("position", RuntimeValue::Float(0.25))],
            );
            let RuntimeValue::Float(value) = noise.output(&output, "value") else {
                panic!("Noise Generator did not produce a float");
            };
            assert!(
                value.is_finite() && (-1.0..=1.0).contains(&value),
                "{algorithm}: {value}"
            );
            value
        })
        .collect()
}

#[test]
fn noise_generator_matrix_covers_every_deterministic_bounded_algorithm() {
    for algorithm in ["random", "perlin", "simplex", "brownian", "cellular", "fractal"] {
        let first = noise_sequence(algorithm);
        let second = noise_sequence(algorithm);
        assert_eq!(first, second, "{algorithm}");
        assert!(
            first.windows(2).any(|pair| pair[0] != pair[1]),
            "{algorithm}: {first:?}"
        );
    }
}

#[test]
fn metronome_matrix_covers_modes_phase_outputs_and_seeded_randomness() {
    for (mode, value) in [("frequency", 2.0), ("bpm", 120.0), ("time", 0.5)] {
        let mut metronome = RuntimeHarness::new(
            "metronome",
            &[
                ("mode", RuntimeValue::String(mode.into())),
                ("value", RuntimeValue::Float(value)),
                ("on_ratio", RuntimeValue::Float(0.5)),
                ("randomness", RuntimeValue::Float(0.0)),
            ],
        );
        let half = metronome.evaluate(1, Duration::from_millis(250), &[]);
        assert!(!trigger(metronome.output(&half, "tick")).fired, "{mode}");
        assert_eq!(metronome.output(&half, "on"), RuntimeValue::Bool(false), "{mode}");
        let full = metronome.evaluate(2, Duration::from_millis(250), &[]);
        let tick = trigger(metronome.output(&full, "tick"));
        assert!(tick.fired, "{mode}");
        assert_eq!(tick.logical_tick, 2, "{mode}");
        assert_eq!(metronome.output(&full, "on"), RuntimeValue::Bool(true), "{mode}");
    }

    fn randomized_sequence() -> Vec<bool> {
        let mut metronome = RuntimeHarness::new(
            "metronome",
            &[
                ("mode", RuntimeValue::String("time".into())),
                ("value", RuntimeValue::Float(0.25)),
                ("on_ratio", RuntimeValue::Float(0.5)),
                ("randomness", RuntimeValue::Float(0.5)),
            ],
        );
        (1..=12)
            .map(|tick| {
                let output = metronome.evaluate(tick, Duration::from_millis(50), &[]);
                trigger(metronome.output(&output, "tick")).fired
            })
            .collect()
    }
    assert_eq!(randomized_sequence(), randomized_sequence());
}
