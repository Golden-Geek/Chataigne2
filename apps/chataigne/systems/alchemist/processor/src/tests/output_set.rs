use std::time::Duration;

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, EvaluationCtx, ManagedItemId, ManagedItemInstance, ManagedItemUiState,
    ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, RuntimeInputSnapshot,
    RuntimeRegistries, StableRef, SurfaceItemKind, TriggerValue, ValueComponent, ValueTypeId, ValueTypeRegistry,
};
use golden_values::Value as RuntimeValue;

use crate::{
    COMMAND_INTENT_KIND, CommandArgumentValues, OUTPUT_BINDINGS_FIELD, OUTPUT_TARGET_FIELD, OutputArgumentBinding,
    OutputBindingConfig, OutputSendPolicy, OutputSetItem, OutputSetRuntime, OutputValueSource, ValueLaneKey, ValueSet,
    ValueSetEntry,
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
        filter_value_mode: Default::default(),
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
fn authored_command_bindings_round_trip_and_reject_unknown_schema() {
    let config = OutputBindingConfig {
        value: OutputValueSource::Whole,
        arguments: vec![OutputArgumentBinding {
            parameter: StableRef::new(ValueTypeId::new("float"), "argument"),
            source: OutputValueSource::Constant(RuntimeValue::Float(2.5)),
        }],
        send_policy: OutputSendPolicy::OnChange,
    };
    let json = config.to_authoring_json().unwrap();
    assert_eq!(OutputBindingConfig::from_authoring_json(&json).unwrap(), config);
    let mut unknown: serde_json::Value = serde_json::from_str(&json).unwrap();
    unknown["route_by_position"] = serde_json::json!(true);
    assert!(
        OutputBindingConfig::from_authoring_json(&unknown.to_string())
            .unwrap_err()
            .contains("unknown field")
    );
    unknown.as_object_mut().unwrap().remove("route_by_position");
    unknown["arguments"][0]["index"] = serde_json::json!(0);
    assert!(
        OutputBindingConfig::from_authoring_json(&unknown.to_string())
            .unwrap_err()
            .contains("unknown field")
    );
}

#[test]
fn large_integer_constant_uses_lossless_raw_authoring_encoding() {
    let config = OutputBindingConfig {
        value: OutputValueSource::Constant(RuntimeValue::Int(i64::MAX)),
        ..OutputBindingConfig::default()
    };
    let json = config.to_authoring_json().unwrap();
    assert!(json.contains("\"raw\""));
    assert_eq!(OutputBindingConfig::from_authoring_json(&json).unwrap(), config);
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
fn valueset_output_uses_stable_element_bindings() {
    let left = command_target("module/left");
    let right = command_target("module/right");
    let definition = output_region_definition();
    let mut left_item = managed_output_item("Left", left.clone(), true);
    left_item.anode.config.set(
        OUTPUT_BINDINGS_FIELD,
        OutputBindingConfig {
            value: OutputValueSource::Element(ValueLaneKey::new("left").unwrap()),
            ..OutputBindingConfig::default()
        }
        .to_runtime_value()
        .unwrap(),
    );
    let mut right_item = managed_output_item("Right", right.clone(), true);
    right_item.anode.config.set(
        OUTPUT_BINDINGS_FIELD,
        OutputBindingConfig {
            value: OutputValueSource::Element(ValueLaneKey::new("right").unwrap()),
            ..OutputBindingConfig::default()
        }
        .to_runtime_value()
        .unwrap(),
    );
    let region = managed_region(vec![right_item, left_item]);
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
    assert_eq!(materialized.output.intents[0].target.as_ref(), Some(&right));
    assert_eq!(materialized.output.intents[0].payload, RuntimeValue::Float(2.0));
    assert_eq!(materialized.output.intents[1].target.as_ref(), Some(&left));
    assert_eq!(materialized.output.intents[1].payload, RuntimeValue::Float(1.0));
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
fn idle_trigger_argument_does_not_invoke_a_command() {
    let runtime = OutputSetRuntime::new(vec![
        OutputSetItem::new("Trigger", command_target("module/trigger")).with_bindings(OutputBindingConfig {
            value: OutputValueSource::Constant(RuntimeValue::Unit),
            arguments: vec![OutputArgumentBinding {
                parameter: command_target("module/trigger/fire"),
                source: OutputValueSource::Whole,
            }],
            ..OutputBindingConfig::default()
        }),
    ]);
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let materialized = runtime.materialize(
        &RuntimeValue::Trigger(TriggerValue::default()),
        &eval_ctx(13, &inputs, &registries),
    );
    assert!(materialized.diagnostics.is_empty());
    assert!(materialized.output.intents.is_empty());
}

#[test]
fn single_value_fans_out_to_multiple_outputs() {
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

    assert!(materialized.diagnostics.is_empty());
    assert_eq!(materialized.output.intents.len(), 2);
    assert!(
        materialized
            .output
            .intents
            .iter()
            .all(|intent| intent.payload == RuntimeValue::Float(0.5))
    );
}

#[test]
fn unbound_tuple_output_reports_actionable_diagnostic() {
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
    assert_eq!(materialized.diagnostics[0].code, "output_set_invalid_binding");
    assert!(materialized.diagnostics[0].message.contains("select a stable element"));
}

#[test]
fn invalid_binding_prevents_partial_output_batch() {
    let runtime = OutputSetRuntime::new(vec![
        OutputSetItem::new("Valid", command_target("module/valid")),
        OutputSetItem::new("Invalid", command_target("module/invalid")).with_bindings(OutputBindingConfig {
            value: OutputValueSource::Element(ValueLaneKey::new("missing").unwrap()),
            ..OutputBindingConfig::default()
        }),
    ]);
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let materialized = runtime.materialize(&RuntimeValue::Float(2.0), &eval_ctx(16, &inputs, &registries));
    assert_eq!(materialized.diagnostics.len(), 1);
    assert!(materialized.output.intents.is_empty());
}

#[test]
fn one_tuple_binds_multiple_command_arguments_and_a_constant() {
    let runtime = OutputSetRuntime::new(vec![
        OutputSetItem::new("3D", command_target("module/3d")).with_bindings(OutputBindingConfig {
            arguments: vec![
                OutputArgumentBinding {
                    parameter: command_target("module/3d/x"),
                    source: OutputValueSource::Element(ValueLaneKey::new("x").unwrap()),
                },
                OutputArgumentBinding {
                    parameter: command_target("module/3d/y"),
                    source: OutputValueSource::Element(ValueLaneKey::new("y").unwrap()),
                },
                OutputArgumentBinding {
                    parameter: command_target("module/3d/scale"),
                    source: OutputValueSource::Constant(RuntimeValue::Float(2.0)),
                },
            ],
            send_policy: OutputSendPolicy::OnChange,
            ..OutputBindingConfig::default()
        }),
    ]);
    let values = ValueSet::with_entries(
        17,
        vec![
            ValueSetEntry::new(ValueLaneKey::new("y").unwrap(), "Y", RuntimeValue::Float(4.0)),
            ValueSetEntry::new(ValueLaneKey::new("x").unwrap(), "X", RuntimeValue::Float(3.0)),
        ],
    );
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(17, &inputs, &registries);
    let materialized = runtime.materialize_values(&values, &ctx);
    assert!(materialized.diagnostics.is_empty());
    let payload = CommandArgumentValues::from_runtime_value(&materialized.output.intents[0].payload)
        .unwrap()
        .unwrap();
    assert_eq!(payload.value, RuntimeValue::Unit);
    assert_eq!(payload.send_policy, OutputSendPolicy::OnChange);
    assert_eq!(payload.arguments[0].value, RuntimeValue::Float(3.0));
    assert_eq!(payload.arguments[1].value, RuntimeValue::Float(4.0));
    assert_eq!(payload.arguments[2].value, RuntimeValue::Float(2.0));
}

#[test]
fn compound_component_and_disabled_outputs_keep_their_own_bindings() {
    let red = command_target("module/red");
    let green = command_target("module/green");
    let disabled = command_target("module/disabled");
    let runtime = OutputSetRuntime::new(vec![
        OutputSetItem::new("Green", green.clone()).with_bindings(OutputBindingConfig {
            value: OutputValueSource::Component {
                element: None,
                component: ValueComponent::G,
            },
            ..OutputBindingConfig::default()
        }),
        OutputSetItem::new("Disabled", disabled)
            .with_enabled(false)
            .with_bindings(OutputBindingConfig {
                value: OutputValueSource::Element(ValueLaneKey::new("missing").unwrap()),
                ..OutputBindingConfig::default()
            }),
        OutputSetItem::new("Red", red.clone()).with_bindings(OutputBindingConfig {
            value: OutputValueSource::Component {
                element: None,
                component: ValueComponent::R,
            },
            ..OutputBindingConfig::default()
        }),
    ]);
    let (inputs, value_types) = context();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let result = runtime.materialize(
        &RuntimeValue::Color(chataigne_alchemist::ColorValue {
            red: 0.2,
            green: 0.4,
            blue: 0.6,
            alpha: 1.0,
        }),
        &eval_ctx(20, &inputs, &registries),
    );
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.output.intents.len(), 2);
    assert_eq!(result.output.intents[0].target.as_ref(), Some(&green));
    assert_eq!(result.output.intents[0].payload, RuntimeValue::Float(0.4));
    assert_eq!(result.output.intents[1].target.as_ref(), Some(&red));
    assert_eq!(result.output.intents[1].payload, RuntimeValue::Float(0.2));
}

#[test]
fn changed_tuple_shape_and_duplicate_arguments_fail_local_validation() {
    let source = StableRef::new(ValueTypeId::new("source"), "source");
    let layout = chataigne_alchemist::ChannelLayout::new(vec![chataigne_alchemist::ChannelDescriptor::input(
        ValueLaneKey::new("x").unwrap(),
        "X",
        source,
        Some(ValueTypeId::new("float")),
    )])
    .unwrap();
    let missing = OutputSetRuntime::new(vec![
        OutputSetItem::new("Missing", command_target("module/missing")).with_bindings(OutputBindingConfig {
            value: OutputValueSource::Element(ValueLaneKey::new("removed").unwrap()),
            ..OutputBindingConfig::default()
        }),
    ]);
    assert!(
        missing
            .validate_layout(&layout)
            .unwrap_err()
            .to_string()
            .contains("removed")
    );

    let parameter = command_target("module/command/value");
    let duplicate = OutputSetRuntime::new(vec![
        OutputSetItem::new("Duplicate", command_target("module/command")).with_bindings(OutputBindingConfig {
            arguments: vec![
                OutputArgumentBinding {
                    parameter: parameter.clone(),
                    source: OutputValueSource::Whole,
                },
                OutputArgumentBinding {
                    parameter,
                    source: OutputValueSource::Constant(RuntimeValue::Float(1.0)),
                },
            ],
            ..OutputBindingConfig::default()
        }),
    ]);
    assert!(
        duplicate
            .validate_layout(&layout)
            .unwrap_err()
            .to_string()
            .contains("bound twice")
    );
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
