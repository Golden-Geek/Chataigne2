use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, ChannelDescriptor, ChannelLayout, CompileCtx, ContextAxisId, ContextKey, EvaluationCtx,
    ManagedRegionId, PipelineLoweringCtx, PrimitiveNodeKind, RuntimeInputSnapshot, RuntimeRegistries, SocketId,
    StableRef, ValueTypeId,
};
use golden_values::Value as RuntimeValue;

use super::managed_formula::{
    command_target, compile_managed_formula, endpoint_ref, eval_ctx, formula_and_instance, input_item,
    managed_item_for_primitive, output_item, region, remap_item,
};
use crate::value_set_pipeline::ValueSetPipelineRuntime;
use crate::{
    ManagedFilterAvailabilityError, RuntimeInputBinding, executable_filter_applications,
    validate_executable_filter_application,
};
use crate::{ValueLaneKey, ValueSet, ValueSetEntry};

#[test]
fn palette_only_returns_applications_the_current_managed_compiler_can_execute() {
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let channel = |id: &str, value_type: &str| {
        ChannelDescriptor::input(
            ValueLaneKey::new(id).unwrap(),
            id,
            StableRef::new(ValueTypeId::new("source"), id),
            Some(ValueTypeId::new(value_type)),
        )
    };
    let two_floats = ChannelLayout::new(vec![channel("a", "float"), channel("b", "float")]).unwrap();
    let available = executable_filter_applications(&two_floats, &ctx);
    let math_modes = available
        .iter()
        .filter(|candidate| candidate.instance.type_id.as_str() == "math")
        .map(|candidate| candidate.instance.config.get("application"))
        .collect::<Vec<_>>();
    assert_eq!(math_modes.len(), 2);
    assert!(math_modes.contains(&None));
    assert!(math_modes.contains(&Some(&RuntimeValue::String("each".into()))));

    let mixed = ChannelLayout::new(vec![channel("a", "float"), channel("b", "bool")]).unwrap();
    assert!(
        executable_filter_applications(&mixed, &ctx)
            .iter()
            .any(|candidate| candidate.instance.type_id.as_str() == "math")
    );
    let mut math = ANodeInstance::new(ANodeTypeId::new("math"), "Math");
    math.config.set("application", RuntimeValue::String("each".into()));
    math.config.set(
        "managed_selection",
        RuntimeValue::Array(vec![RuntimeValue::String("a".into())]),
    );
    assert!(validate_executable_filter_application(&math, &two_floats, &ctx).is_ok());

    let color = ChannelLayout::new(vec![channel("color", "color")]).unwrap();
    let extract = ANodeInstance::new(ANodeTypeId::new("extract_color"), "Extract Color");
    assert!(validate_executable_filter_application(&extract, &color, &ctx).is_ok());

    let unresolved = ChannelLayout::new(vec![ChannelDescriptor::input(
        ValueLaneKey::new("unknown").unwrap(),
        "Unknown",
        StableRef::new(ValueTypeId::new("source"), "unknown"),
        None,
    )])
    .unwrap();
    assert!(matches!(
        validate_executable_filter_application(&math, &unresolved, &ctx),
        Err(ManagedFilterAvailabilityError::UnresolvedInputType)
    ));
}

#[test]
fn math_apply_to_each_uses_the_graph_math_kernel_with_an_auxiliary_operand() {
    let (formula, mut instance) = formula_and_instance();
    let left = endpoint_ref("module/left");
    let right = endpoint_ref("module/right");
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode
        .config
        .set("application", RuntimeValue::String("each".into()));
    math.anode
        .input_defaults
        .insert(SocketId::new("value2"), RuntimeValue::Float(2.0));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![input_item("Left", left.clone()), input_item("Right", right.clone())],
        ),
    );
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("filters"), region("filters", vec![math]));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region(
            "outputs",
            vec![
                output_item("Left", command_target("target/left")),
                output_item("Right", command_target("target/right")),
            ],
        ),
    );
    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(left, RuntimeValue::Float(3.0));
    inputs.insert(right, RuntimeValue::Float(5.0));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let output = runtime.evaluate(&eval_ctx(1, &inputs, &registries));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(
        output.intents.iter().map(|intent| &intent.payload).collect::<Vec<_>>(),
        vec![&RuntimeValue::Float(5.0), &RuntimeValue::Float(7.0),]
    );
}

#[test]
fn math_combine_selected_uses_declared_left_fold_for_subtraction() {
    let (formula, mut instance) = formula_and_instance();
    let sources = [
        endpoint_ref("module/a"),
        endpoint_ref("module/b"),
        endpoint_ref("module/c"),
    ];
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode
        .config
        .set("application", RuntimeValue::String("combine".into()));
    math.anode
        .config
        .set("operator", RuntimeValue::String("subtract".into()));
    math.anode.config.set("num_inputs", RuntimeValue::Int(3));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            sources
                .iter()
                .enumerate()
                .map(|(index, source)| input_item(&format!("Input {index}"), source.clone()))
                .collect(),
        ),
    );
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("filters"), region("filters", vec![math]));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Result", command_target("target/result"))]),
    );
    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    for (source, value) in sources.into_iter().zip([10.0, 3.0, 2.0]) {
        inputs.insert(source, RuntimeValue::Float(value));
    }
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let output = runtime.evaluate(&eval_ctx(1, &inputs, &registries));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].payload, RuntimeValue::Float(5.0));
}

#[test]
fn backend_coefficient_edit_updates_compiled_stage_without_resetting_smooth_history() {
    let (formula, mut instance) = formula_and_instance();
    let source = endpoint_ref("module/fader");
    let remap = remap_item(0.0, 10.0, 0.0, 1.0);
    let remap_id = remap.id;
    let mut smooth = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    smooth.anode.config.set("method", RuntimeValue::String("sma".into()));
    smooth.anode.config.set("window", RuntimeValue::Int(2));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region("inputs", vec![input_item("Fader", source.clone())]),
    );
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("filters"), region("filters", vec![remap, smooth]));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Output", command_target("target/output"))]),
    );

    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Float(5.0));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let first = runtime.evaluate(&eval_ctx(1, &inputs, &registries));
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert_eq!(first.intents[0].payload, RuntimeValue::Float(0.5));

    runtime
        .update_filter_input(
            remap_id,
            &SocketId::new("out_max"),
            RuntimeInputBinding::Constant(RuntimeValue::Float(2.0)),
        )
        .unwrap();
    let second = runtime.evaluate(&eval_ctx(2, &inputs, &registries));
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(second.intents[0].payload, RuntimeValue::Float(0.75));
}

#[test]
fn auxiliary_reference_tracks_external_value_without_recompilation() {
    let (formula, mut instance) = formula_and_instance();
    let source = endpoint_ref("module/fader");
    let coefficient = StableRef::new(ValueTypeId::new("float"), "module/coefficient");
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode
        .config
        .set("application", RuntimeValue::String("each".into()));
    math.anode
        .input_defaults
        .insert(SocketId::new("value2"), RuntimeValue::Ref(coefficient.clone()));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region("inputs", vec![input_item("Fader", source.clone())]),
    );
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("filters"), region("filters", vec![math]));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Output", command_target("target/output"))]),
    );

    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Float(3.0));
    inputs.insert(coefficient.clone(), RuntimeValue::Float(2.0));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let first = runtime.evaluate(&eval_ctx(1, &inputs, &registries));
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert_eq!(first.intents[0].payload, RuntimeValue::Float(5.0));
    inputs.insert(coefficient, RuntimeValue::Float(4.0));
    let second = runtime.evaluate(&eval_ctx(2, &inputs, &registries));
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(second.intents[0].payload, RuntimeValue::Float(7.0));
}

#[test]
fn auxiliary_reference_resolves_each_channel_context_before_shared_value() {
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode
        .config
        .set("application", RuntimeValue::String("each".into()));
    let coefficient = StableRef::new(ValueTypeId::new("float"), "module/coefficient");
    math.anode
        .input_defaults
        .insert(SocketId::new("value2"), RuntimeValue::Ref(coefficient.clone()));
    let value_types = crate::alchemist::value_type_registry();
    let nodes = crate::alchemist::node_registry();
    let lowering = PipelineLoweringCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: None,
    };
    let mut runtime =
        ValueSetPipelineRuntime::compile_elementwise(vec![math], ValueTypeId::new("float"), &lowering).unwrap();
    let values = ValueSet::with_entries(
        1,
        vec![
            ValueSetEntry::new(ValueLaneKey::new("a").unwrap(), "A", RuntimeValue::Float(3.0)),
            ValueSetEntry::new(ValueLaneKey::new("b").unwrap(), "B", RuntimeValue::Float(3.0)),
        ],
    );
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(coefficient.clone(), RuntimeValue::Float(100.0));
    let axis = ContextAxisId::new("value_set_lane");
    let axes = [axis.clone()].into_iter().collect();
    inputs.insert_context(
        coefficient.clone(),
        &axes,
        ContextKey::single(axis.clone(), "a"),
        RuntimeValue::Float(2.0),
    );
    inputs.insert_context(
        coefficient,
        &axes,
        ContextKey::single(axis, "b"),
        RuntimeValue::Float(4.0),
    );
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: std::time::Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };
    let (mapped, output) = runtime.evaluate(&values, &ctx).unwrap();
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(mapped.entries[0].value, RuntimeValue::Float(5.0));
    assert_eq!(mapped.entries[1].value, RuntimeValue::Float(7.0));
}
