use std::time::Duration;

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, EvaluationCtx, ManagedItemId, ManagedItemInstance, ManagedItemUiState,
    ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, RuntimeInputSnapshot,
    RuntimeRegistries, StableRef, SurfaceItemKind, ValueTypeId, ValueTypeRegistry,
};
use golden_values::Value as RuntimeValue;

use crate::{INPUT_SOURCE_FIELD, InputSetRuntime, ValueLaneKey};

fn input_ref(id: &str) -> StableRef {
    StableRef::new(ValueTypeId::new("chataigne.module_endpoint"), id)
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

fn input_region_definition() -> ManagedRegionDefinition {
    ManagedRegionDefinition {
        id: ManagedRegionId::new("inputs"),
        kind: ManagedRegionKind::InputSet,
        label: "Inputs".into(),
        input_socket: None,
        output_socket: None,
        accepted_roles: vec![SurfaceItemKind::Input],
    }
}

fn managed_input_item(label: &str, source: StableRef, enabled: bool) -> ManagedItemInstance {
    let mut anode = ANodeInstance::new(ANodeTypeId::new("chataigne.input_source"), label);
    anode.config.set(INPUT_SOURCE_FIELD, RuntimeValue::Ref(source));
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode,
        enabled,
        ui_state: ManagedItemUiState::default(),
    }
}

fn managed_region(items: Vec<ManagedItemInstance>) -> ManagedRegionInstance {
    ManagedRegionInstance {
        region_id: ManagedRegionId::new("inputs"),
        items,
    }
}

#[test]
fn single_input_materializes_valueset_entry() {
    let source = input_ref("module/fader");
    let runtime = InputSetRuntime::new(vec![crate::InputSetItem::new(
        ValueLaneKey::new("fader").unwrap(),
        "Fader",
        source.clone(),
    )]);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source.clone(), RuntimeValue::Float(0.75));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(42, &inputs, &registries);

    let materialized = runtime.materialize(&ctx);

    assert!(materialized.diagnostics.is_empty());
    assert_eq!(materialized.value_set.logical_tick, 42);
    assert_eq!(materialized.value_set.entries.len(), 1);
    assert_eq!(materialized.value_set.entries[0].key.as_str(), "fader");
    assert_eq!(materialized.value_set.entries[0].label, "Fader");
    assert_eq!(materialized.value_set.entries[0].source.as_ref(), Some(&source));
    assert_eq!(materialized.value_set.entries[0].value, RuntimeValue::Float(0.75));
}

#[test]
fn multiple_inputs_materialize_in_authored_order() {
    let x = input_ref("module/x");
    let y = input_ref("module/y");
    let definition = input_region_definition();
    let region = managed_region(vec![
        managed_input_item("X", x.clone(), true),
        managed_input_item("Y", y.clone(), true),
    ]);
    let runtime = InputSetRuntime::from_managed_region(&definition, &region).unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(x, RuntimeValue::Float(1.0));
    inputs.insert(y, RuntimeValue::Float(2.0));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(7, &inputs, &registries);

    let materialized = runtime.materialize(&ctx);

    assert!(materialized.diagnostics.is_empty());
    assert_eq!(
        materialized
            .value_set
            .entries
            .iter()
            .map(|entry| (entry.label.as_str(), entry.value.clone()))
            .collect::<Vec<_>>(),
        vec![("X", RuntimeValue::Float(1.0)), ("Y", RuntimeValue::Float(2.0))]
    );
}

#[test]
fn input_reorder_preserves_lane_identity() {
    let x = input_ref("module/x");
    let y = input_ref("module/y");
    let first = managed_input_item("X", x, true);
    let second = managed_input_item("Y", y, true);
    let definition = input_region_definition();

    let original =
        InputSetRuntime::from_managed_region(&definition, &managed_region(vec![first.clone(), second.clone()]))
            .unwrap();
    let reordered = InputSetRuntime::from_managed_region(&definition, &managed_region(vec![second, first])).unwrap();

    assert_eq!(original.items()[0].label, "X");
    assert_eq!(reordered.items()[1].label, "X");
    assert_eq!(original.items()[0].key, reordered.items()[1].key);
}

#[test]
fn disabled_input_is_excluded() {
    let enabled = input_ref("module/enabled");
    let disabled = input_ref("module/disabled");
    let definition = input_region_definition();
    let runtime = InputSetRuntime::from_managed_region(
        &definition,
        &managed_region(vec![
            managed_input_item("Enabled", enabled.clone(), true),
            managed_input_item("Disabled", disabled.clone(), false),
        ]),
    )
    .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(enabled, RuntimeValue::Bool(true));
    inputs.insert(disabled, RuntimeValue::Bool(false));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(1, &inputs, &registries);

    let materialized = runtime.materialize(&ctx);

    assert!(materialized.diagnostics.is_empty());
    assert_eq!(materialized.value_set.entries.len(), 1);
    assert_eq!(materialized.value_set.entries[0].label, "Enabled");
}

#[test]
fn missing_input_reports_diagnostic_without_fake_value() {
    let missing = input_ref("module/missing");
    let runtime = InputSetRuntime::new(vec![crate::InputSetItem::new(
        ValueLaneKey::new("missing").unwrap(),
        "Missing",
        missing,
    )]);
    let inputs = RuntimeInputSnapshot::default();
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(9, &inputs, &registries);

    let materialized = runtime.materialize(&ctx);

    assert!(materialized.value_set.entries.is_empty());
    assert_eq!(materialized.diagnostics.len(), 1);
    assert_eq!(materialized.diagnostics[0].code, "input_set_missing_source");
}
