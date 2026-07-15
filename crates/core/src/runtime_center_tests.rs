use std::sync::Arc;
use std::time::Duration;

use golden_runtime::RuntimeMetrics;

use crate::engine::Engine;
use crate::node::Node;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck};
use crate::runtime_center::ProductionState;

fn parameter(label: &str, value: i32) -> Parameter {
    Parameter::new(label, ParamValue::Int(value), ParameterChangeCheck::ValueChange)
}

#[test]
fn production_input_port_drives_the_authoritative_engine_through_dense_slots() {
    let engine = Engine::new(parameter("input", 0));
    let root = engine.root;
    let metrics = Arc::new(RuntimeMetrics::default());
    let (mut state, input) = ProductionState::new(engine, metrics.clone()).unwrap();

    input.publish(root, ParamValue::Int(42), 100).unwrap();
    assert_eq!(
        state
            .engine
            .nodes
            .get(root)
            .and_then(Node::engine_param_snapshot)
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Int(0))
    );

    state.run_tick(Duration::from_millis(1)).unwrap();

    assert_eq!(
        state
            .engine
            .nodes
            .get(root)
            .and_then(Node::engine_param_snapshot)
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Int(42))
    );
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.dense_batches, 1);
    assert_eq!(snapshot.work_units, 1);
    assert_eq!(snapshot.shadow_comparisons, 1);
    assert_eq!(snapshot.shadow_mismatches, 0);
    let tick = state.engine.tick_stats();
    assert_eq!(tick.snapshot_rebuilds, 0);
    assert_eq!(tick.snapshot_builds, 0);
    assert_eq!(tick.snapshot_nodes_cloned, 0);
}

#[test]
fn asynchronous_generation_swap_rebinds_new_parameter_inputs_without_dropping_the_old_generation() {
    let engine = Engine::new(parameter("root", 0));
    let metrics = Arc::new(RuntimeMetrics::default());
    let (mut state, input) = ProductionState::new(engine, metrics.clone()).unwrap();

    let root = state.engine.root;
    state.engine.add_node(parameter("dynamic", 1), Some(root));
    state.engine.apply_edits().unwrap();
    let dynamic = state
        .engine
        .nodes
        .iter()
        .map(|(node, _)| node)
        .find(|node| *node != root)
        .expect("dynamic parameter");
    state.request_compilation("test.structure");

    for _ in 0..100 {
        state.run_tick(Duration::from_millis(1)).unwrap();
        if input.publish(dynamic, ParamValue::Int(9), 200).is_ok() {
            state.run_tick(Duration::from_millis(1)).unwrap();
            break;
        }
        std::thread::yield_now();
    }

    assert_eq!(
        state
            .engine
            .nodes
            .get(dynamic)
            .and_then(Node::engine_param_snapshot)
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Int(9))
    );
    let snapshot = metrics.snapshot();
    assert!(snapshot.compilation_applied >= 1);
    assert!(snapshot.generation_id >= 2);
}
