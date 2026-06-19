use std::time::Duration;

use golden_alchemist::{
    ANodeDeclaration, ANodeInstance, EvaluationCtx, ManagedItemId, ManagedItemInstance, ManagedItemUiState,
    PipelineLoweringCtx, PrimitiveNodeDeclaration, PrimitiveNodeKind, RuntimeInputSnapshot, RuntimeRegistries,
    RuntimeValue, SocketId, ValueTypeId,
};

use crate::alchemist::{node_registry, value_type_registry};
use crate::value_set_pipeline::{ValueSetPipelineRuntime, ValueSetProjectionRuntime};
use crate::{ValueLaneKey, ValueSet, ValueSetEntry};

#[test]
fn elementwise_remap_preserves_lanes_and_values() {
    let value_types = value_type_registry();
    let nodes = node_registry();
    let lowering_ctx = PipelineLoweringCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let mut runtime = ValueSetPipelineRuntime::compile_elementwise(
        vec![identity_remap_item()],
        ValueTypeId::new("float"),
        &lowering_ctx,
    )
    .unwrap();
    let values = ValueSet::with_entries(
        1,
        vec![
            ValueSetEntry::new(ValueLaneKey::new("a").unwrap(), "A", RuntimeValue::Float(0.25)),
            ValueSetEntry::new(ValueLaneKey::new("b").unwrap(), "B", RuntimeValue::Float(0.75)),
        ],
    );

    let (mapped, output) = runtime.evaluate(&values, &eval_ctx(&value_types, 2)).unwrap();

    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    assert_eq!(mapped.logical_tick, 2);
    assert_eq!(mapped.entries[0].key.as_str(), "a");
    assert_eq!(mapped.entries[0].value, RuntimeValue::Float(0.25));
    assert_eq!(mapped.entries[1].key.as_str(), "b");
    assert_eq!(mapped.entries[1].value, RuntimeValue::Float(0.75));
}

#[test]
fn remap_clamp_chain_maps_each_lane() {
    let value_types = value_type_registry();
    let nodes = node_registry();
    let lowering_ctx = PipelineLoweringCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let mut runtime = ValueSetPipelineRuntime::compile_elementwise(
        vec![remap_item(0.0, 10.0, 0.0, 2.0), clamp_item(0.0, 1.0)],
        ValueTypeId::new("float"),
        &lowering_ctx,
    )
    .unwrap();
    let values = float_value_set(1, [("a", "A", 2.5), ("b", "B", 7.5)]);

    let (mapped, output) = runtime.evaluate(&values, &eval_ctx(&value_types, 2)).unwrap();

    assert_clean(&output);
    assert_eq!(mapped.entries[0].value, RuntimeValue::Float(0.5));
    assert_eq!(mapped.entries[1].value, RuntimeValue::Float(1.0));
}

#[test]
fn smooth_filter_keeps_independent_lane_memory() {
    let value_types = value_type_registry();
    let nodes = node_registry();
    let lowering_ctx = PipelineLoweringCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let mut runtime = ValueSetPipelineRuntime::compile_elementwise(
        vec![one_euro_smooth_item()],
        ValueTypeId::new("float"),
        &lowering_ctx,
    )
    .unwrap();
    assert_eq!(runtime.state_slot_count(), 1);
    let first = float_value_set(1, [("a", "A", 0.0), ("b", "B", 10.0)]);
    let second = float_value_set(2, [("a", "A", 2.0), ("b", "B", 10.0)]);

    let (first_mapped, first_output) = runtime.evaluate(&first, &eval_ctx(&value_types, 1)).unwrap();
    assert_eq!(runtime.lane_memory_count(), 2);
    let (mapped, second_output) = runtime.evaluate(&second, &eval_ctx(&value_types, 2)).unwrap();

    assert_clean(&first_output);
    assert_clean(&second_output);
    let first_b = float_value(&first_mapped.entries[1].value);
    let second_a = float_value(&mapped.entries[0].value);
    let second_b = float_value(&mapped.entries[1].value);
    assert!(
        second_a < 0.5,
        "lane a should not inherit lane b smoothing state: {second_a}"
    );
    assert!(
        second_b > first_b,
        "lane b should continue from its own previous smoothing state: {first_b} -> {second_b}"
    );
}

#[test]
fn aggregate_reduces_multiple_lanes_to_one_value() {
    let value_types = value_type_registry();
    let nodes = node_registry();
    let lowering_ctx = PipelineLoweringCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode.config.set("num_inputs", RuntimeValue::Int(3));
    let runtime =
        ValueSetProjectionRuntime::compile_aggregate(math, 3, ValueTypeId::new("float"), &lowering_ctx).unwrap();
    let values = float_value_set(1, [("x", "X", 1.0), ("y", "Y", 2.0), ("z", "Z", 3.0)]);

    let (value, output) = runtime.evaluate(&values, &eval_ctx(&value_types, 2)).unwrap();

    assert_clean(&output);
    assert_eq!(value, RuntimeValue::Float(6.0));
}

#[test]
fn pack_vec3_projects_three_lanes_to_vector() {
    let value_types = value_type_registry();
    let nodes = node_registry();
    let lowering_ctx = PipelineLoweringCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let runtime = ValueSetProjectionRuntime::compile_pack_vec3(
        managed_item_for_primitive(PrimitiveNodeKind::PackVec3),
        &lowering_ctx,
    )
    .unwrap();
    let values = float_value_set(1, [("x", "X", 1.0), ("y", "Y", 2.0), ("z", "Z", 3.0)]);

    let (value, output) = runtime.evaluate(&values, &eval_ctx(&value_types, 2)).unwrap();

    assert_clean(&output);
    assert_eq!(value, RuntimeValue::Vec3([1.0, 2.0, 3.0]));
}

#[test]
fn whole_set_condition_gate_can_run_per_lane_with_defaults() {
    let value_types = value_type_registry();
    let nodes = node_registry();
    let lowering_ctx = PipelineLoweringCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::ConditionGate);
    item.anode
        .input_defaults
        .insert(SocketId::new("condition"), RuntimeValue::Bool(true));
    let mut runtime =
        ValueSetPipelineRuntime::compile_elementwise(vec![item], ValueTypeId::new("float"), &lowering_ctx).unwrap();
    let values = ValueSet::with_entries(
        1,
        vec![ValueSetEntry::new(
            ValueLaneKey::new("a").unwrap(),
            "A",
            RuntimeValue::Float(0.5),
        )],
    );

    let (mapped, output) = runtime.evaluate(&values, &eval_ctx(&value_types, 3)).unwrap();

    assert_clean(&output);
    assert_eq!(mapped.entries[0].value, RuntimeValue::Float(0.5));
}

fn assert_clean(output: &golden_alchemist::RuntimeOutput) {
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
}

fn managed_item_for_primitive(kind: PrimitiveNodeKind) -> ManagedItemInstance {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode: ANodeInstance::new(declaration.type_id(), declaration.label()),
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    }
}

fn identity_remap_item() -> ManagedItemInstance {
    remap_item(0.0, 1.0, 0.0, 1.0)
}

fn remap_item(in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> ManagedItemInstance {
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::Remap);
    item.anode
        .input_defaults
        .insert(SocketId::new("in_min"), RuntimeValue::Float(in_min));
    item.anode
        .input_defaults
        .insert(SocketId::new("in_max"), RuntimeValue::Float(in_max));
    item.anode
        .input_defaults
        .insert(SocketId::new("out_min"), RuntimeValue::Float(out_min));
    item.anode
        .input_defaults
        .insert(SocketId::new("out_max"), RuntimeValue::Float(out_max));
    item
}

fn clamp_item(minimum: f64, maximum: f64) -> ManagedItemInstance {
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::Clamp);
    item.anode
        .input_defaults
        .insert(SocketId::new("minimum"), RuntimeValue::Float(minimum));
    item.anode
        .input_defaults
        .insert(SocketId::new("maximum"), RuntimeValue::Float(maximum));
    item
}

fn one_euro_smooth_item() -> ManagedItemInstance {
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    item.anode.config.set("method", RuntimeValue::String("one_euro".into()));
    item
}

fn float_value(value: &RuntimeValue) -> f64 {
    match value {
        RuntimeValue::Float(value) => *value,
        other => panic!("expected float value, got {other:?}"),
    }
}

fn float_value_set<const N: usize>(tick: u64, entries: [(&str, &str, f64); N]) -> ValueSet {
    ValueSet::with_entries(
        tick,
        entries
            .into_iter()
            .map(|(key, label, value)| {
                ValueSetEntry::new(ValueLaneKey::new(key).unwrap(), label, RuntimeValue::Float(value))
            })
            .collect(),
    )
}

fn eval_ctx<'a>(value_types: &'a golden_alchemist::ValueTypeRegistry, tick: u64) -> EvaluationCtx<'a> {
    let registries = Box::leak(Box::new(RuntimeRegistries { value_types }));
    let inputs = Box::leak(Box::new(RuntimeInputSnapshot::default()));
    EvaluationCtx {
        logical_tick: tick,
        delta_time: Duration::from_millis(16),
        events: &[],
        inputs,
        registries,
    }
}
