use std::time::Duration;

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, EvaluationCtx, ManagedItemId, ManagedItemInstance, ManagedItemUiState,
    ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, RuntimeInputSnapshot,
    RuntimeRegistries, StableRef, SurfaceItemKind, TriggerValue, ValueTypeId, ValueTypeRegistry,
};
use golden_values::Value as RuntimeValue;

use crate::{
    COMMAND_INTENT_KIND, OUTPUT_TARGET_FIELD, OutputSetItem, OutputSetRuntime, ValueLaneKey, ValueSet, ValueSetEntry,
};

fn command_target(id: &str) -> StableRef {
    StableRef::new(ValueTypeId::new("chataigne.command_target"), id)
}

fn eval_ctx<'a>(
    logical_tick: u64,
    inputs: &'a RuntimeInputSnapshot,
    registries: &'a RuntimeRegistries<'a>,
) -> EvaluationCtx<'a> {
    EvaluationCtx {
        logical_tick,
        delta_time: Duration::ZERO,
        events: &[],
        inputs,
        registries,
    }
}

fn output_region_definition() -> ManagedRegionDefinition {
    ManagedRegionDefinition {
        id: ManagedRegionId::new("outputs"),
        kind: ManagedRegionKind::OutputSet,
        label: "Outputs".into(),
        input_socket: None,
        output_socket: None,
        accepted_roles: vec![SurfaceItemKind::Output],
    }
}

fn managed_output_item(label: &str, target: StableRef, enabled: bool) -> ManagedItemInstance {
    let mut anode = ANodeInstance::new(ANodeTypeId::new("chataigne.output_target"), label);
    anode.config.set(OUTPUT_TARGET_FIELD, RuntimeValue::Ref(target));
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode,
        enabled,
        ui_state: ManagedItemUiState::default(),
    }
}

fn managed_region(items: Vec<ManagedItemInstance>) -> ManagedRegionInstance {
    ManagedRegionInstance {
        region_id: ManagedRegionId::new("outputs"),
        items,
    }
}

fn context() -> (RuntimeInputSnapshot, ValueTypeRegistry) {
    (RuntimeInputSnapshot::default(), ValueTypeRegistry::with_primitives())
}

#[test]
fn single_value_output_creates_expected_intent() {
    let target = command_target("module/fader");
    let runtime = OutputSetRuntime::new(vec![OutputSetItem::new("Fader", target.clone())]);
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(11, &inputs, &registries);

    let materialized = runtime.materialize(&RuntimeValue::Float(0.5), &ctx);

    assert!(materialized.diagnostics.is_empty());
    assert_eq!(materialized.output.intents.len(), 1);
    assert_eq!(materialized.output.intents[0].kind.as_ref(), COMMAND_INTENT_KIND);
    assert_eq!(materialized.output.intents[0].target.as_ref(), Some(&target));
    assert_eq!(materialized.output.intents[0].payload, RuntimeValue::Float(0.5));
    assert_eq!(materialized.output.intents[0].logical_tick, 11);
}

#[test]
fn valueset_output_creates_per_entry_intents() {
    let left = command_target("module/left");
    let right = command_target("module/right");
    let definition = output_region_definition();
    let region = managed_region(vec![
        managed_output_item("Left", left.clone(), true),
        managed_output_item("Right", right.clone(), true),
    ]);
    let runtime = OutputSetRuntime::from_managed_region(&definition, &region).unwrap();
    let value_set = ValueSet::with_entries(
        4,
        vec![
            ValueSetEntry::new(ValueLaneKey::new("left").unwrap(), "Left", RuntimeValue::Float(1.0)),
            ValueSetEntry::new(ValueLaneKey::new("right").unwrap(), "Right", RuntimeValue::Float(2.0)),
        ],
    );
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(12, &inputs, &registries);

    let materialized = runtime.materialize(&value_set.to_runtime_value().unwrap(), &ctx);

    assert!(materialized.diagnostics.is_empty());
    assert_eq!(materialized.output.intents.len(), 2);
    assert_eq!(materialized.output.intents[0].target.as_ref(), Some(&left));
    assert_eq!(materialized.output.intents[0].payload, RuntimeValue::Float(1.0));
    assert_eq!(materialized.output.intents[1].target.as_ref(), Some(&right));
    assert_eq!(materialized.output.intents[1].payload, RuntimeValue::Float(2.0));
}

#[test]
fn idle_trigger_output_creates_no_intent() {
    let runtime = OutputSetRuntime::new(vec![OutputSetItem::new(
        "Trigger Target",
        command_target("module/trigger"),
    )]);
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(13, &inputs, &registries);

    let materialized = runtime.materialize(&RuntimeValue::Trigger(TriggerValue::default()), &ctx);

    assert!(materialized.diagnostics.is_empty());
    assert!(materialized.output.intents.is_empty());
}

#[test]
fn single_value_with_multiple_outputs_reports_diagnostic_without_broadcast() {
    let runtime = OutputSetRuntime::new(vec![
        OutputSetItem::new("Left", command_target("module/left")),
        OutputSetItem::new("Right", command_target("module/right")),
    ]);
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(14, &inputs, &registries);

    let materialized = runtime.materialize(&RuntimeValue::Float(0.5), &ctx);

    assert!(materialized.output.intents.is_empty());
    assert_eq!(materialized.diagnostics.len(), 1);
    assert_eq!(
        materialized.diagnostics[0].code,
        "output_set_single_value_requires_single_output"
    );
}

#[test]
fn valueset_output_count_mismatch_reports_diagnostic_without_partial_dispatch() {
    let runtime = OutputSetRuntime::new(vec![OutputSetItem::new("Only Output", command_target("module/only"))]);
    let value_set = ValueSet::with_entries(
        1,
        vec![
            ValueSetEntry::new(ValueLaneKey::new("one").unwrap(), "One", RuntimeValue::Float(1.0)),
            ValueSetEntry::new(ValueLaneKey::new("two").unwrap(), "Two", RuntimeValue::Float(2.0)),
        ],
    );
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(15, &inputs, &registries);

    let materialized = runtime.materialize(&value_set.to_runtime_value().unwrap(), &ctx);

    assert!(materialized.output.intents.is_empty());
    assert_eq!(materialized.diagnostics.len(), 1);
    assert_eq!(materialized.diagnostics[0].code, "output_set_valueset_output_mismatch");
}

#[test]
fn disabled_output_is_excluded() {
    let enabled = command_target("module/enabled");
    let disabled = command_target("module/disabled");
    let definition = output_region_definition();
    let runtime = OutputSetRuntime::from_managed_region(
        &definition,
        &managed_region(vec![
            managed_output_item("Enabled", enabled.clone(), true),
            managed_output_item("Disabled", disabled, false),
        ]),
    )
    .unwrap();
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(16, &inputs, &registries);

    let materialized = runtime.materialize(&RuntimeValue::Bool(true), &ctx);

    assert!(materialized.diagnostics.is_empty());
    assert_eq!(materialized.output.intents.len(), 1);
    assert_eq!(materialized.output.intents[0].target.as_ref(), Some(&enabled));
}
