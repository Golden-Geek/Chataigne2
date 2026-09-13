use std::{sync::Arc, time::Duration};

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, ChannelMetadata, EvaluationCtx, ManagedItemId, ManagedItemInstance, ManagedItemUiState,
    ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, MappingValueShape,
    RuntimeInputSnapshot, RuntimeRegistries, StableRef, SurfaceItemKind, ValueTypeId, ValueTypeRegistry,
};
use golden_values::Value as RuntimeValue;

use crate::{ChannelSourceSchema, ChannelValidity, INPUT_SOURCE_FIELD, InputSetRuntime, ValueLaneKey};

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
        filter_value_mode: Default::default(),
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
    let mut runtime = InputSetRuntime::new(vec![crate::InputSetItem::new(
        ValueLaneKey::new("fader").unwrap(),
        "Fader",
        source.clone(),
    )])
    .unwrap();
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
    let mut runtime = InputSetRuntime::from_managed_region(&definition, &region).unwrap();
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
    let mut runtime = InputSetRuntime::from_managed_region(
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
    assert_eq!(materialized.frame.layout().channels().len(), 2);
    assert_eq!(materialized.frame.slots()[1].validity, ChannelValidity::Disabled);
}

#[test]
fn missing_input_reports_diagnostic_without_fake_value() {
    let missing = input_ref("module/missing");
    let mut runtime = InputSetRuntime::new(vec![crate::InputSetItem::new(
        ValueLaneKey::new("missing").unwrap(),
        "Missing",
        missing,
    )])
    .unwrap();
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
    assert_eq!(materialized.frame.layout().channels()[0].id.as_str(), "missing");
    assert_eq!(materialized.frame.slots()[0].validity, ChannelValidity::MissingSource);
}

#[test]
fn mixed_repeated_sources_and_value_edits_keep_one_declared_layout() {
    let source = input_ref("module/shared");
    let flag = input_ref("module/flag");
    let vector = input_ref("module/vector");
    let text = input_ref("module/text");
    let mut runtime = InputSetRuntime::new(vec![
        crate::InputSetItem::new(ValueLaneKey::new("first").unwrap(), "First", source.clone())
            .with_value_type(ValueTypeId::new("float")),
        crate::InputSetItem::new(ValueLaneKey::new("second").unwrap(), "Second", source.clone())
            .with_value_type(ValueTypeId::new("float")),
        crate::InputSetItem::new(ValueLaneKey::new("flag").unwrap(), "Flag", flag.clone())
            .with_value_type(ValueTypeId::new("bool")),
        crate::InputSetItem::new(ValueLaneKey::new("vector").unwrap(), "Vector", vector.clone())
            .with_value_type(ValueTypeId::new("vec3")),
        crate::InputSetItem::new(ValueLaneKey::new("text").unwrap(), "Text", text.clone())
            .with_value_type(ValueTypeId::new("string")),
    ])
    .unwrap();
    let layout = runtime.layout().clone();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source.clone(), RuntimeValue::Float(1.0));
    inputs.insert(flag, RuntimeValue::Bool(true));
    inputs.insert(vector, RuntimeValue::Vec3([1.0, 2.0, 3.0]));
    inputs.insert(text, RuntimeValue::String("hello".into()));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let first = runtime.materialize(&eval_ctx(1, &inputs, &registries));
    assert!(first.diagnostics.is_empty());
    assert_eq!(first.frame.slots().len(), 5);
    assert_ne!(
        first.frame.layout().channels()[0].id,
        first.frame.layout().channels()[1].id
    );
    assert_eq!(first.frame.slots()[3].value, Some(RuntimeValue::Vec3([1.0, 2.0, 3.0])));
    let initial_revision = first.frame.layout().structural_revision();
    inputs.insert(source, RuntimeValue::Float(2.0));
    let second = runtime.materialize(&eval_ctx(2, &inputs, &registries));
    assert!(Arc::ptr_eq(second.frame.layout(), &layout));
    assert_eq!(second.frame.layout().structural_revision(), initial_revision);
    assert!(second.frame.slots()[0].changed);
    assert!(second.frame.slots()[1].changed);
    assert!(!second.frame.slots()[2].changed);
}

#[test]
fn disable_rename_reorder_and_remove_keep_identity_and_report_missing_source() {
    let first = crate::InputSetItem::new(ValueLaneKey::new("first").unwrap(), "First", input_ref("first"));
    let second = crate::InputSetItem::new(ValueLaneKey::new("second").unwrap(), "Second", input_ref("second"));
    let mut runtime = InputSetRuntime::new(vec![first.clone(), second.clone()]).unwrap();
    let structural = runtime.layout().structural_revision();
    runtime
        .reconcile_items(vec![first.clone(), second.clone().with_enabled(false)])
        .unwrap();
    assert_eq!(runtime.layout().structural_revision(), structural);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(first.source.clone(), RuntimeValue::Float(1.0));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let disabled = runtime.materialize(&eval_ctx(1, &inputs, &registries));
    assert_eq!(disabled.frame.slots()[1].validity, ChannelValidity::Disabled);
    let mut renamed = second.clone();
    renamed.label = "Renamed".into();
    runtime.reconcile_items(vec![renamed.clone(), first.clone()]).unwrap();
    assert_eq!(runtime.layout().channels()[0].id.as_str(), "second");
    assert_eq!(runtime.layout().channels()[0].label, "Renamed");
    assert_eq!(runtime.layout().channels()[1].id.as_str(), "first");
    let missing = runtime.materialize(&eval_ctx(2, &inputs, &registries));
    assert_eq!(missing.diagnostics[0].code, "input_set_missing_source");
    assert_eq!(missing.frame.slots()[0].validity, ChannelValidity::MissingSource);
    runtime.reconcile_items(vec![first]).unwrap();
    assert!(
        runtime
            .layout()
            .index_of(&ValueLaneKey::new("second").unwrap())
            .is_none()
    );
}

#[test]
fn empty_input_set_is_incomplete_and_duplicate_authored_id_is_rejected() {
    let mut runtime = InputSetRuntime::new(vec![]).unwrap();
    assert_eq!(runtime.value_shape(), MappingValueShape::Incomplete);
    let inputs = RuntimeInputSnapshot::default();
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let empty = runtime.materialize(&eval_ctx(1, &inputs, &registries));
    assert_eq!(empty.diagnostics[0].code, "input_set_empty");
    assert!(empty.frame.slots().is_empty());
    let item = crate::InputSetItem::new(ValueLaneKey::new("same").unwrap(), "A", input_ref("a"));
    assert!(InputSetRuntime::new(vec![item.clone(), item]).is_err());
}

#[test]
fn authored_sources_form_one_scalar_or_ordered_typed_tuple() {
    let collection = InputSetRuntime::new(vec![
        crate::InputSetItem::new(
            ValueLaneKey::new("collection").unwrap(),
            "Collection",
            input_ref("collection"),
        )
        .with_value_type(ValueTypeId::new("value_array")),
    ])
    .unwrap();
    assert_eq!(
        collection.value_shape(),
        MappingValueShape::Single(Some(ValueTypeId::new("value_array")))
    );
    let shared = input_ref("shared");
    let flag = input_ref("flag");
    let first = crate::InputSetItem::new(ValueLaneKey::new("x").unwrap(), "X", shared.clone())
        .with_value_type(ValueTypeId::new("float"));
    let second = crate::InputSetItem::new(ValueLaneKey::new("y").unwrap(), "Y", shared)
        .with_value_type(ValueTypeId::new("float"));
    let third = crate::InputSetItem::new(ValueLaneKey::new("flag").unwrap(), "Flag", flag);
    let mut runtime = InputSetRuntime::new(vec![first.clone()]).unwrap();
    assert_eq!(
        runtime.value_shape(),
        MappingValueShape::Single(Some(ValueTypeId::new("float")))
    );
    runtime
        .reconcile_items(vec![first.clone(), second.clone(), third.clone()])
        .unwrap();
    assert_eq!(
        runtime.value_shape(),
        MappingValueShape::Tuple(vec![Some("float".into()), Some("float".into()), None])
    );
    assert!(!runtime.value_shape().is_resolved());
    runtime
        .reconcile_source_schema(|source| {
            (source == &third.source).then(|| ChannelSourceSchema {
                value_type: ValueTypeId::new("bool"),
                metadata: ChannelMetadata {
                    minimum: None,
                    maximum: None,
                    unit: Some("switch".into()),
                },
            })
        })
        .unwrap();
    assert_eq!(
        runtime.value_shape(),
        MappingValueShape::Tuple(vec![Some("float".into()), Some("float".into()), Some("bool".into())])
    );
    assert!(runtime.value_shape().is_resolved());
    runtime.reconcile_items(vec![third, second, first]).unwrap();
    assert_eq!(
        runtime.value_shape(),
        MappingValueShape::Tuple(vec![Some("bool".into()), Some("float".into()), Some("float".into())])
    );
    assert_eq!(runtime.layout().channels()[0].metadata.unit.as_deref(), Some("switch"));
    runtime
        .reconcile_items(vec![crate::InputSetItem::new(
            ValueLaneKey::new("flag").unwrap(),
            "Replacement",
            input_ref("replacement"),
        )])
        .unwrap();
    assert_eq!(runtime.value_shape(), MappingValueShape::Single(None));
    assert_eq!(runtime.layout().channels()[0].metadata.unit, None);
}

#[test]
fn explicit_source_schema_event_resolves_type_and_metadata_without_value_driven_rebuild() {
    let source = input_ref("module/fader");
    let mut runtime = InputSetRuntime::new(vec![crate::InputSetItem::new(
        ValueLaneKey::new("fader").unwrap(),
        "Fader",
        source.clone(),
    )])
    .unwrap();
    let initial = runtime.layout().structural_revision();
    runtime
        .reconcile_source_schema(|reference| {
            (reference == &source).then(|| ChannelSourceSchema {
                value_type: ValueTypeId::new("float"),
                metadata: ChannelMetadata {
                    minimum: Some(0.0),
                    maximum: Some(1.0),
                    unit: Some("normalized".into()),
                },
            })
        })
        .unwrap();
    assert_eq!(runtime.layout().structural_revision(), initial + 1);
    assert_eq!(
        runtime.layout().channels()[0].metadata.unit.as_deref(),
        Some("normalized")
    );
    let resolved = runtime.layout().structural_revision();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source.clone(), RuntimeValue::Float(0.5));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    assert!(
        runtime
            .materialize(&eval_ctx(1, &inputs, &registries))
            .diagnostics
            .is_empty()
    );
    inputs.insert(source, RuntimeValue::Bool(true));
    let wrong = runtime.materialize(&eval_ctx(2, &inputs, &registries));
    assert_eq!(wrong.diagnostics[0].code, "input_set_type_mismatch");
    assert_eq!(wrong.frame.slots()[0].validity, ChannelValidity::Invalid);
    assert_eq!(runtime.layout().structural_revision(), resolved);
}
