//! Public DSL/attribute metadata survives construction, binding and project loading.
use golden_core::{
    app::{ProjectLifecycle, from_sparse_project_json, prepare_engine_for_runtime, to_sparse_project_json_pretty},
    engine::Engine,
    node::{Folder, Node, ParameterHandle},
    parameter::ParamValue,
};

#[golden_core::node("time.dsl")]
#[children(
    milliseconds: f64 = 1250.0 (widget = "time", unit = "ms");
    seconds: f64 = 1.25 (widget = "time", unit = "s");
)]
struct DslClock {}
#[golden_core::node("time.dsl", from_struct)]
impl Node for DslClock {}

#[golden_core::node("time.attribute")]
struct AttributeClock {
    #[param(default = 250.0, widget = "time", unit = "ms")]
    duration: ParameterHandle<f64>,
}
#[golden_core::node("time.attribute", from_struct)]
impl Node for AttributeClock {}

golden_core::define_node_enum!(
    enum ClockNode {
        Dsl(DslClock),
        Attribute(AttributeClock),
    }
);
impl ProjectLifecycle for ClockNode {}

fn assert_units(engine: &Engine<ClockNode>) {
    let mut seen = Vec::new();
    for (_, node) in engine.nodes.iter() {
        if let Some(parameter) = node.engine_param_snapshot() {
            assert_eq!(parameter.ui_hints.widget.as_deref(), Some("time"));
            seen.push((parameter.value, parameter.ui_hints.unit));
        }
    }
    assert_eq!(seen.len(), 3);
    for (value, unit) in [(1250.0, "ms"), (1.25, "s"), (250.0, "ms")] {
        assert!(seen.contains(&(ParamValue::Float(value), Some(unit.into()))));
    }
}

#[test]
fn public_time_hints_preserve_storage_units_through_bind_and_roundtrip() {
    let mut engine = Engine::new(ClockNode::from(Folder::new("root")));
    engine.add_node(DslClock::new().into(), None);
    engine.add_node(AttributeClock::new().into(), None);
    prepare_engine_for_runtime(&mut engine).unwrap();
    assert_units(&engine);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<ClockNode>(&saved).unwrap();
    prepare_engine_for_runtime(&mut loaded).unwrap();
    assert_units(&loaded);
}
