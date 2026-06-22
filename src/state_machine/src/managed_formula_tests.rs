use std::time::Duration;

use golden_alchemist::{
    ANodeDeclaration, ANodeInstance, AlchemistFormula, AlchemistFormulaInstance, AlchemistGraph, CompileCtx,
    EvaluationCtx, FormulaContextContract, FormulaId, FormulaPropertySchema, FormulaRef, FormulaSurface, ManagedItemId,
    ManagedItemInstance, ManagedItemUiState, ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance,
    ManagedRegionKind, PrimitiveNodeDeclaration, PrimitiveNodeKind, RuntimeInputSnapshot, RuntimeRegistries,
    RuntimeValue, SocketId, StableRef, SurfaceItemKind, TriggerValue, ValueTypeId, ValueTypeRegistry,
};
use golden_statechart::StateId;

use crate::alchemist::node_registry;
use crate::{
    INPUT_SOURCE_FIELD, ManagedFormulaRuntime, OUTPUT_TARGET_FIELD, Processor, ProcessorLifecycleEvent,
    ProcessorRuntime,
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
fn processor_runtime_evaluates_managed_mapping_sidecar() {
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
    let processor = Processor::new("Mapping", instance);
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
fn action_trigger_produces_command_intent() {
    let (formula, mut instance) = action_formula_and_instance();
    let source = endpoint_ref("module/trigger");
    let target = command_target("target/action");
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
        panic!("Action command should carry trigger payload");
    };
    assert!(trigger.fired);
    assert_eq!(trigger.edge_id, 7);
}

#[test]
fn action_condition_gate_blocks_command() {
    let (formula, mut instance) = action_formula_and_instance();
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
            vec![output_item("Command", command_target("target/action"))],
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
fn action_condition_gate_passes_command() {
    let (formula, mut instance) = action_formula_and_instance();
    let source = endpoint_ref("module/trigger");
    let target = command_target("target/action");
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
fn processor_runtime_evaluates_managed_action_sidecar() {
    let (formula, mut instance) = action_formula_and_instance();
    let source = endpoint_ref("module/trigger");
    let target = command_target("target/action");
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("trigger"),
        region("trigger", vec![input_item("Trigger", source.clone())]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("commands"),
        region("commands", vec![output_item("Command", target.clone())]),
    );
    let processor = Processor::new("Action", instance);
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

    let output = runtime.evaluate_processor(&processor, &ctx);

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].target.as_ref(), Some(&target));
}

fn formula_and_instance() -> (AlchemistFormula, AlchemistFormulaInstance) {
    let formula = AlchemistFormula {
        id: FormulaId::new("test.mapping"),
        version: 1,
        label: "Test Mapping".into(),
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

fn action_formula_and_instance() -> (AlchemistFormula, AlchemistFormulaInstance) {
    let formula = AlchemistFormula {
        id: FormulaId::new("test.action"),
        version: 1,
        label: "Test Action".into(),
        description: None,
        tags: Vec::new(),
        graph: AlchemistGraph::new(),
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface {
            sections: Vec::new(),
            managed_regions: vec![
                ManagedRegionDefinition {
                    id: ManagedRegionId::new("trigger"),
                    kind: ManagedRegionKind::ActionTrigger,
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
                    kind: ManagedRegionKind::ActionCommands,
                    label: "Commands".into(),
                    input_socket: None,
                    output_socket: None,
                    accepted_roles: vec![SurfaceItemKind::Action],
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
