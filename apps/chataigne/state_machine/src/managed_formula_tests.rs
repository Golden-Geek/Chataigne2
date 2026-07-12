use std::time::Duration;

use golden_alchemist::{
    ANodeDeclaration, ANodeId, ANodeInstance, AlchemistFormula, AlchemistFormulaInstance, AlchemistGraph,
    AlchemistRuntime, CompileCtx, DebugCaptureMode, EvaluationCtx, FormulaContextContract, FormulaId,
    FormulaPropertySchema, FormulaRef, FormulaSurface, InputSocketRef, ManagedItemId, ManagedItemInstance,
    ManagedItemUiState, ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind,
    OutputSocketRef, PrimitiveNodeDeclaration, PrimitiveNodeKind, RuntimeInputSnapshot, RuntimeRegistries,
    RuntimeValue, SocketId, StableRef, SurfaceItemKind, TriggerValue, ValueTypeId, ValueTypeRegistry, compile_graph,
};
use golden_statechart::StateId;

use crate::alchemist::node_registry;
use crate::{
    DefaultProcessorContextProvider, INPUT_SOURCE_FIELD, ManagedFormulaRuntime, OUTPUT_TARGET_FIELD, Processor,
    ProcessorDebugCapture, ProcessorLifecycleEvent, ProcessorRuntime,
};

#[test]
fn managed_formula_maps_inputs_to_outputs_without_filters() {
    let (formula, mut instance) = formula_and_instance();
    let left = endpoint_ref("module/left");
    let right = endpoint_ref("module/right");
    let out_left = command_target("target/left");
    let out_right = command_target("target/right");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![input_item("Left", left.clone()), input_item("Right", right.clone())],
        ),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region(
            "outputs",
            vec![
                output_item("Left", out_left.clone()),
                output_item("Right", out_right.clone()),
            ],
        ),
    );

    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(left, RuntimeValue::Float(0.25));
    inputs.insert(right, RuntimeValue::Float(0.75));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(10, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 2);
    assert_eq!(output.intents[0].target.as_ref(), Some(&out_left));
    assert_eq!(output.intents[0].payload, RuntimeValue::Float(0.25));
    assert_eq!(output.intents[1].target.as_ref(), Some(&out_right));
    assert_eq!(output.intents[1].payload, RuntimeValue::Float(0.75));
}

#[test]
fn managed_formula_runs_elementwise_filter_pipeline_before_outputs() {
    let (formula, mut instance) = formula_and_instance();
    let left = endpoint_ref("module/left");
    let right = endpoint_ref("module/right");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![input_item("Left", left.clone()), input_item("Right", right.clone())],
        ),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![remap_item(0.0, 10.0, 0.0, 2.0), clamp_item(0.0, 1.0)]),
    );
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

    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(left, RuntimeValue::Float(2.5));
    inputs.insert(right, RuntimeValue::Float(7.5));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(11, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents[0].payload, RuntimeValue::Float(0.5));
    assert_eq!(output.intents[1].payload, RuntimeValue::Float(1.0));
}

#[test]
fn managed_formula_runtime_filter_errors_use_specific_diagnostic_prefix() {
    let (formula, mut instance) = formula_and_instance();
    let left = endpoint_ref("module/left");
    let right = endpoint_ref("module/right");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![input_item("Left", left.clone()), input_item("Right", right.clone())],
        ),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![remap_item(0.0, 1.0, 0.0, 1.0)]),
    );
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
    inputs.insert(left, RuntimeValue::Float(0.25));
    inputs.insert(right, RuntimeValue::Bool(true));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(19, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert_eq!(output.diagnostics.len(), 1);
    assert!(
        output.diagnostics[0]
            .message
            .starts_with("managed_formula_mixed_valueset_types:")
    );
    assert!(output.intents.is_empty());
}

#[test]
fn manager_filter_chain_matches_direct_anode_result() {
    let (formula, mut instance) = formula_and_instance();
    let source = endpoint_ref("module/value");
    let target = command_target("target/value");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region("inputs", vec![input_item("Value", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![remap_item(0.0, 10.0, 0.0, 2.0), clamp_item(0.0, 1.0)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Value", target.clone())]),
    );

    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Float(7.5));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(12, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
    assert_eq!(output.intents[0].payload, direct_remap_clamp_result(7.5));
}

#[test]
fn managed_formula_aggregates_valueset_to_single_output() {
    let (formula, mut instance) = formula_and_instance();
    let x = endpoint_ref("module/x");
    let y = endpoint_ref("module/y");
    let z = endpoint_ref("module/z");
    let target = command_target("target/sum");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![
                input_item("X", x.clone()),
                input_item("Y", y.clone()),
                input_item("Z", z.clone()),
            ],
        ),
    );
    let mut math = managed_item_for_primitive(PrimitiveNodeKind::Math);
    math.anode.config.set("num_inputs", RuntimeValue::Int(3));
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("filters"), region("filters", vec![math]));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Sum", target.clone())]),
    );

    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(x, RuntimeValue::Float(1.0));
    inputs.insert(y, RuntimeValue::Float(2.0));
    inputs.insert(z, RuntimeValue::Float(3.0));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(12, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
    assert_eq!(output.intents[0].payload, RuntimeValue::Float(6.0));
}

#[test]
fn managed_formula_projects_three_lanes_to_vec3_output() {
    let (formula, mut instance) = formula_and_instance();
    let x = endpoint_ref("module/x");
    let y = endpoint_ref("module/y");
    let z = endpoint_ref("module/z");
    let target = command_target("target/position");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region(
            "inputs",
            vec![
                input_item("X", x.clone()),
                input_item("Y", y.clone()),
                input_item("Z", z.clone()),
            ],
        ),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![managed_item_for_primitive(PrimitiveNodeKind::PackVec3)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Position", target.clone())]),
    );

    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &compile_ctx)
        .unwrap()
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(x, RuntimeValue::Float(1.0));
    inputs.insert(y, RuntimeValue::Float(2.0));
    inputs.insert(z, RuntimeValue::Float(3.0));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(13, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
    assert_eq!(output.intents[0].payload, RuntimeValue::Vec3([1.0, 2.0, 3.0]));
}

#[test]
fn processor_runtime_evaluates_managed_value_pipeline_sidecar() {
    let (formula, mut instance) = formula_and_instance();
    let source = endpoint_ref("module/fader");
    let target = command_target("target/fader");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region("inputs", vec![input_item("Fader", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![remap_item(0.0, 1.0, 0.0, 100.0)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Fader", target.clone())]),
    );
    let processor = Processor::new("Value Pipeline", instance);
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ProcessorRuntime::new(processor.id);

    assert!(runtime.compile(&processor, &formula, &compile_ctx));
    runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::StateEnter(StateId::new()));

    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Float(0.5));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(14, &inputs, &registries);

    let output = runtime.evaluate_processor(&processor, &ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
    assert_eq!(output.intents[0].payload, RuntimeValue::Float(50.0));
}

#[test]
fn trigger_input_produces_command_intent() {
    let (formula, mut instance) = trigger_pipeline_formula_and_instance();
    let source = endpoint_ref("module/trigger");
    let target = command_target("target/command");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("trigger"),
        region("trigger", vec![input_item("Trigger", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands"),
        region("commands", vec![output_item("Command", target.clone())]),
    );

    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Trigger(TriggerValue::fired(7, 15)));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(15, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
    let RuntimeValue::Trigger(trigger) = output.intents[0].payload else {
        panic!("Command should carry trigger payload");
    };
    assert!(trigger.fired);
    assert_eq!(trigger.edge_id, 7);
}

#[test]
fn trigger_pipeline_allows_multiple_command_regions() {
    let (mut formula, mut instance) = trigger_pipeline_formula_and_instance();
    formula.surface.managed_regions.push(ManagedRegionDefinition {
        id: ManagedRegionId::new("commands_false"),
        kind: ManagedRegionKind::CommandSet,
        label: "On False".into(),
        input_socket: None,
        output_socket: None,
        accepted_roles: vec![SurfaceItemKind::Command],
    });

    let source = endpoint_ref("module/trigger");
    let target_true = command_target("target/on_true");
    let target_false = command_target("target/on_false");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("trigger"),
        region("trigger", vec![input_item("Trigger", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands"),
        region("commands", vec![output_item("On True", target_true.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands_false"),
        region("commands_false", vec![output_item("On False", target_false.clone())]),
    );

    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Trigger(TriggerValue::fired(8, 16)));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(16, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 2);
    assert!(
        output
            .intents
            .iter()
            .any(|intent| intent.target.as_ref() == Some(&target_true))
    );
    assert!(
        output
            .intents
            .iter()
            .any(|intent| intent.target.as_ref() == Some(&target_false))
    );
}

#[test]
fn trigger_condition_gate_blocks_command() {
    let (formula, mut instance) = trigger_pipeline_formula_and_instance();
    let source = endpoint_ref("module/trigger");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("trigger"),
        region("trigger", vec![input_item("Trigger", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("pipeline"),
        region("pipeline", vec![condition_gate_item(false)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands"),
        region(
            "commands",
            vec![output_item("Command", command_target("target/command"))],
        ),
    );

    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Trigger(TriggerValue::fired(8, 16)));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(16, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert!(output.intents.is_empty());
}

#[test]
fn trigger_condition_gate_passes_command() {
    let (formula, mut instance) = trigger_pipeline_formula_and_instance();
    let source = endpoint_ref("module/trigger");
    let target = command_target("target/command");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("trigger"),
        region("trigger", vec![input_item("Trigger", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("pipeline"),
        region("pipeline", vec![condition_gate_item(true)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands"),
        region("commands", vec![output_item("Command", target.clone())]),
    );

    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Trigger(TriggerValue::fired(9, 17)));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(17, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
}

#[test]
fn manager_condition_gate_matches_direct_anode_result() {
    let (formula, mut instance) = trigger_pipeline_formula_and_instance();
    let source = endpoint_ref("module/trigger");
    let target = command_target("target/command");
    let trigger = TriggerValue::fired(10, 18);
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("trigger"),
        region("trigger", vec![input_item("Trigger", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("pipeline"),
        region("pipeline", vec![condition_gate_item(true)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands"),
        region("commands", vec![output_item("Command", target.clone())]),
    );

    let mut runtime = compile_managed_formula(&formula, &instance);
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Trigger(trigger));
    let value_types = crate::alchemist::value_type_registry();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(18, &inputs, &registries);

    let output = runtime.evaluate(&ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
    assert_eq!(output.intents[0].payload, direct_condition_gate_result(trigger, true));
}

#[test]
fn processor_runtime_evaluates_managed_trigger_pipeline_sidecar() {
    let (mut formula, mut instance) = trigger_pipeline_formula_and_instance();
    let mut constant = primitive_anode(PrimitiveNodeKind::Constant);
    constant.config.set("value", RuntimeValue::Bool(true));
    let constant_id = formula.graph.add_node(constant).unwrap();
    let source = endpoint_ref("module/trigger");
    let target = command_target("target/command");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("trigger"),
        region("trigger", vec![input_item("Trigger", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands"),
        region("commands", vec![output_item("Command", target.clone())]),
    );
    let processor = Processor::new("Trigger Pipeline", instance);
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ProcessorRuntime::new(processor.id);

    assert!(runtime.compile(&processor, &formula, &compile_ctx));
    runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::StateEnter(StateId::new()));

    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source, RuntimeValue::Trigger(TriggerValue::fired(10, 18)));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(18, &inputs, &registries);

    let provider = DefaultProcessorContextProvider;
    let mut lanes = runtime.evaluate_processor_with_context_provider_and_runtime_capture(
        &processor,
        &ctx,
        &provider,
        &ProcessorDebugCapture::All { history_len: 64 },
    );
    assert_eq!(lanes.len(), 1);
    let output = lanes.remove(0).output;

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
    assert!(
        output
            .debug_samples
            .iter()
            .any(|sample| sample.author_node_id == constant_id),
        "managed sidecar evaluation should retain authored-graph activity samples"
    );
}

#[test]
fn managed_formula_missing_region_diagnostic_uses_specific_code() {
    let (mut formula, _) = formula_and_instance();
    formula
        .surface
        .managed_regions
        .retain(|region| region.kind != ManagedRegionKind::OutputSet);
    let mut instance = formula.instantiate();
    instance.formula_ref = FormulaRef {
        id: formula.id.clone(),
        version: formula.version,
    };

    let diagnostic = compile_error_diagnostic(&formula, &instance);

    assert_eq!(diagnostic.code, "managed_formula_missing_region");
    assert!(diagnostic.message.contains("OutputSet"));
}

#[test]
fn managed_formula_missing_command_target_uses_specific_code() {
    let (formula, mut instance) = trigger_pipeline_formula_and_instance();
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands"),
        region(
            "commands",
            vec![ManagedItemInstance {
                id: ManagedItemId::new(),
                anode: ANodeInstance::new(golden_alchemist::ANodeTypeId::new("managed_output"), "Command"),
                enabled: true,
                ui_state: ManagedItemUiState::default(),
            }],
        ),
    );

    let diagnostic = compile_error_diagnostic(&formula, &instance);

    assert_eq!(diagnostic.code, "managed_formula_missing_command_target");
    assert!(diagnostic.message.contains(OUTPUT_TARGET_FIELD));
}

fn formula_and_instance() -> (AlchemistFormula, AlchemistFormulaInstance) {
    let formula = AlchemistFormula {
        id: FormulaId::new("test.value_pipeline"),
        version: 1,
        label: "Test Value Pipeline".into(),
        description: None,
        tags: Vec::new(),
        graph: AlchemistGraph::new(),
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface {
            sections: Vec::new(),
            managed_regions: vec![
                ManagedRegionDefinition {
                    id: ManagedRegionId::new("inputs"),
                    kind: ManagedRegionKind::InputSet,
                    label: "Inputs".into(),
                    input_socket: None,
                    output_socket: None,
                    accepted_roles: vec![SurfaceItemKind::Input],
                },
                ManagedRegionDefinition {
                    id: ManagedRegionId::new("filters"),
                    kind: ManagedRegionKind::FilterPipeline,
                    label: "Filters".into(),
                    input_socket: None,
                    output_socket: None,
                    accepted_roles: vec![SurfaceItemKind::Filter],
                },
                ManagedRegionDefinition {
                    id: ManagedRegionId::new("outputs"),
                    kind: ManagedRegionKind::OutputSet,
                    label: "Outputs".into(),
                    input_socket: None,
                    output_socket: None,
                    accepted_roles: vec![SurfaceItemKind::Output],
                },
            ],
        },
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    };
    let mut instance = formula.instantiate();
    instance.formula_ref = FormulaRef {
        id: formula.id.clone(),
        version: formula.version,
    };
    (formula, instance)
}

fn trigger_pipeline_formula_and_instance() -> (AlchemistFormula, AlchemistFormulaInstance) {
    let formula = AlchemistFormula {
        id: FormulaId::new("test.trigger_pipeline"),
        version: 1,
        label: "Test Trigger Pipeline".into(),
        description: None,
        tags: Vec::new(),
        graph: AlchemistGraph::new(),
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface {
            sections: Vec::new(),
            managed_regions: vec![
                ManagedRegionDefinition {
                    id: ManagedRegionId::new("trigger"),
                    kind: ManagedRegionKind::TriggerInput,
                    label: "Trigger".into(),
                    input_socket: None,
                    output_socket: None,
                    accepted_roles: vec![SurfaceItemKind::Input],
                },
                ManagedRegionDefinition {
                    id: ManagedRegionId::new("pipeline"),
                    kind: ManagedRegionKind::FilterPipeline,
                    label: "Pipeline".into(),
                    input_socket: None,
                    output_socket: None,
                    accepted_roles: vec![SurfaceItemKind::Filter],
                },
                ManagedRegionDefinition {
                    id: ManagedRegionId::new("commands"),
                    kind: ManagedRegionKind::CommandSet,
                    label: "Commands".into(),
                    input_socket: None,
                    output_socket: None,
                    accepted_roles: vec![SurfaceItemKind::Command],
                },
            ],
        },
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    };
    let mut instance = formula.instantiate();
    instance.formula_ref = FormulaRef {
        id: formula.id.clone(),
        version: formula.version,
    };
    (formula, instance)
}

fn compile_managed_formula(formula: &AlchemistFormula, instance: &AlchemistFormulaInstance) -> ManagedFormulaRuntime {
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    ManagedFormulaRuntime::compile(formula, instance, &compile_ctx)
        .unwrap()
        .unwrap()
}

fn compile_error_diagnostic(
    formula: &AlchemistFormula,
    instance: &AlchemistFormulaInstance,
) -> golden_alchemist::Diagnostic {
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    match ManagedFormulaRuntime::compile(formula, instance, &compile_ctx) {
        Ok(_) => panic!("managed formula should fail"),
        Err(error) => error.into_diagnostic(),
    }
}

fn region(id: &str, items: Vec<ManagedItemInstance>) -> ManagedRegionInstance {
    ManagedRegionInstance {
        region_id: ManagedRegionId::new(id),
        items,
    }
}

fn input_item(label: &str, source: StableRef) -> ManagedItemInstance {
    let mut anode = ANodeInstance::new(golden_alchemist::ANodeTypeId::new("managed_input"), label);
    anode.config.set(INPUT_SOURCE_FIELD, RuntimeValue::Ref(source));
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode,
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    }
}

fn output_item(label: &str, target: StableRef) -> ManagedItemInstance {
    let mut anode = ANodeInstance::new(golden_alchemist::ANodeTypeId::new("managed_output"), label);
    anode.config.set(OUTPUT_TARGET_FIELD, RuntimeValue::Ref(target));
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode,
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    }
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

fn condition_gate_item(condition: bool) -> ManagedItemInstance {
    let mut item = managed_item_for_primitive(PrimitiveNodeKind::ConditionGate);
    item.anode
        .config
        .set("mode", RuntimeValue::String("block_trigger".into()));
    item.anode
        .input_defaults
        .insert(SocketId::new("condition"), RuntimeValue::Bool(condition));
    item
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

fn direct_remap_clamp_result(value: f64) -> RuntimeValue {
    let mut graph = AlchemistGraph::new();
    let mut source = ANodeInstance::new(golden_alchemist::ANodeTypeId::new("constant"), "Value");
    source.config.set("value", RuntimeValue::Float(value));
    let mut remap = primitive_anode(PrimitiveNodeKind::Remap);
    remap
        .input_defaults
        .insert(SocketId::new("in_min"), RuntimeValue::Float(0.0));
    remap
        .input_defaults
        .insert(SocketId::new("in_max"), RuntimeValue::Float(10.0));
    remap
        .input_defaults
        .insert(SocketId::new("out_min"), RuntimeValue::Float(0.0));
    remap
        .input_defaults
        .insert(SocketId::new("out_max"), RuntimeValue::Float(2.0));
    let mut clamp = primitive_anode(PrimitiveNodeKind::Clamp);
    clamp
        .input_defaults
        .insert(SocketId::new("minimum"), RuntimeValue::Float(0.0));
    clamp
        .input_defaults
        .insert(SocketId::new("maximum"), RuntimeValue::Float(1.0));

    let source = graph.add_node(source).unwrap();
    let remap = graph.add_node(remap).unwrap();
    let clamp = graph.add_node(clamp).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(remap, "value"),
        )
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(remap, "result"),
            InputSocketRef::new(clamp, "value"),
        )
        .unwrap();

    evaluate_direct_output(graph, clamp, "result")
}

fn direct_condition_gate_result(trigger: TriggerValue, condition: bool) -> RuntimeValue {
    let mut graph = AlchemistGraph::new();
    let mut source = ANodeInstance::new(golden_alchemist::ANodeTypeId::new("constant"), "Trigger");
    source.config.set("value", RuntimeValue::Trigger(trigger));
    let mut gate = primitive_anode(PrimitiveNodeKind::ConditionGate);
    gate.config.set("mode", RuntimeValue::String("block_trigger".into()));
    gate.input_defaults
        .insert(SocketId::new("condition"), RuntimeValue::Bool(condition));
    gate.input_defaults.insert(
        SocketId::new("default_value"),
        RuntimeValue::Trigger(TriggerValue::default()),
    );

    let source = graph.add_node(source).unwrap();
    let gate = graph.add_node(gate).unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(gate, "value"),
        )
        .unwrap();

    evaluate_direct_output(graph, gate, "value")
}

fn primitive_anode(kind: PrimitiveNodeKind) -> ANodeInstance {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    ANodeInstance::new(declaration.type_id(), declaration.label())
}

fn evaluate_direct_output(graph: AlchemistGraph, output_node: ANodeId, socket: &str) -> RuntimeValue {
    let (value_types, nodes) = registries();
    let compiled = compile_graph(
        &graph,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let mut runtime = AlchemistRuntime::new(compiled.compiled.unwrap());
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let ctx = eval_ctx(1, &inputs, &registries);
    let output = runtime.evaluate_with_capture_mode(&ctx, DebugCaptureMode::All { history_len: 64 });
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    output
        .debug_samples
        .into_iter()
        .find(|sample| sample.author_node_id == output_node && sample.output_socket.as_str() == socket)
        .unwrap_or_else(|| panic!("missing direct `{socket}` output sample"))
        .value
}

fn endpoint_ref(id: &str) -> StableRef {
    StableRef::new(ValueTypeId::new("chataigne.module_endpoint"), id)
}

fn command_target(id: &str) -> StableRef {
    StableRef::new(ValueTypeId::new("chataigne.command_target"), id)
}

fn registries() -> (ValueTypeRegistry, golden_alchemist::ANodeRegistry) {
    (crate::alchemist::value_type_registry(), node_registry())
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
