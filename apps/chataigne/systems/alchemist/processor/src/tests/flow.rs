use chataigne_alchemist::{
    AxisSet, CompileCtx, ContextAxisId, ContextKey, ContextValuePath, ManagedFilterValueMode, ManagedItemId,
    ManagedRegionId, PrimitiveNodeKind, RuntimeInputSnapshot, RuntimeRegistries, SocketId, ValueTypeRegistry,
};
use golden_values::Value as RuntimeValue;

use super::managed_formula::{
    command_target, endpoint_ref, eval_ctx, formula_and_instance, input_item, managed_item_for_primitive, output_item,
    region, registries, remap_item,
};
use crate::{
    ChannelSourceSchema, ManagedFormulaRuntime, Processor, ProcessorBindingAnalysis, ProcessorContextProvider,
    ProcessorId, ProcessorLifecycleEvent, ProcessorRuntime, RuntimeInputBinding,
};
use chataigne_state_machine_model::StateId;

fn gate_document(
    mode: &str,
    default: Option<f64>,
) -> (
    chataigne_alchemist::AlchemistFormula,
    chataigne_alchemist::AlchemistFormulaInstance,
    ManagedItemId,
) {
    let (mut formula, mut instance) = formula_and_instance();
    formula
        .surface
        .managed_regions
        .iter_mut()
        .find(|region| region.id == ManagedRegionId::new("filters"))
        .unwrap()
        .filter_value_mode = ManagedFilterValueMode::Tuple;
    let mut gate = managed_item_for_primitive(PrimitiveNodeKind::ConditionGate);
    gate.anode.config.set("mode", RuntimeValue::String(mode.into()));
    gate.anode
        .input_defaults
        .insert(SocketId::new("condition"), RuntimeValue::Bool(false));
    if let Some(default) = default {
        gate.anode
            .input_defaults
            .insert(SocketId::new("default_value"), RuntimeValue::Float(default));
    }
    let gate_id = gate.id;
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region("inputs", vec![input_item("Input", endpoint_ref("module/value"))]),
    );
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("filters"), region("filters", vec![gate]));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Output", command_target("target/command"))]),
    );
    (formula, instance, gate_id)
}

fn mapping_gate(mode: &str, default: Option<f64>) -> (ManagedFormulaRuntime, ManagedItemId, ValueTypeRegistry) {
    let (formula, instance, gate_id) = gate_document(mode, default);
    let (value_types, nodes) = registries();
    let ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &ctx)
        .unwrap()
        .unwrap();
    runtime
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    (runtime, gate_id, value_types)
}

fn evaluate(
    runtime: &mut ManagedFormulaRuntime,
    value_types: &ValueTypeRegistry,
    tick: u64,
) -> chataigne_alchemist::RuntimeOutput {
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(endpoint_ref("module/value"), RuntimeValue::Float(5.0));
    let registries = RuntimeRegistries { value_types };
    runtime.evaluate(&eval_ctx(tick, &inputs, &registries))
}

#[test]
fn closed_mapping_gate_sends_no_default_and_reopens_for_unchanged_source() {
    let (mut runtime, gate, value_types) = mapping_gate("pass_when_true", Some(7.0));
    let blocked = evaluate(&mut runtime, &value_types, 1);
    assert!(blocked.diagnostics.is_empty(), "{:?}", blocked.diagnostics);
    assert!(blocked.intents.is_empty());

    runtime
        .update_filter_input(
            gate,
            &SocketId::new("condition"),
            RuntimeInputBinding::Constant(RuntimeValue::Bool(true)),
        )
        .unwrap();
    let opened = evaluate(&mut runtime, &value_types, 2);
    assert!(opened.diagnostics.is_empty(), "{:?}", opened.diagnostics);
    assert_eq!(opened.intents.len(), 1);
    assert_eq!(opened.intents[0].payload, RuntimeValue::Float(5.0));
}

#[test]
fn output_default_is_delivered_only_when_selected() {
    let (mut runtime, _, value_types) = mapping_gate("output_default", Some(7.0));
    let output = evaluate(&mut runtime, &value_types, 1);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.intents.len(), 1);
    assert_eq!(output.intents[0].payload, RuntimeValue::Float(7.0));
}

#[test]
fn suppressed_temporal_stage_sleeps_until_gate_control_reopens() {
    let (formula, mut instance, gate) = gate_document("pass_when_true", None);
    instance
        .managed_regions
        .regions
        .get_mut(&ManagedRegionId::new("filters"))
        .unwrap()
        .items
        .push(managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter));
    let (value_types, nodes) = registries();
    let ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &ctx)
        .unwrap()
        .unwrap();
    runtime
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    assert!(runtime.needs_continuous_evaluation());
    assert!(evaluate(&mut runtime, &value_types, 1).intents.is_empty());
    assert!(!runtime.needs_continuous_evaluation());
    runtime
        .update_filter_input(
            gate,
            &SocketId::new("condition"),
            RuntimeInputBinding::Constant(RuntimeValue::Bool(true)),
        )
        .unwrap();
    assert_eq!(evaluate(&mut runtime, &value_types, 2).intents.len(), 1);
    assert!(runtime.needs_continuous_evaluation());
}

#[test]
fn hold_last_starts_empty_then_uses_last_passed_value() {
    let (mut runtime, gate, value_types) = mapping_gate("hold_last", None);
    assert!(evaluate(&mut runtime, &value_types, 1).intents.is_empty());
    runtime
        .update_filter_input(
            gate,
            &SocketId::new("condition"),
            RuntimeInputBinding::Constant(RuntimeValue::Bool(true)),
        )
        .unwrap();
    assert_eq!(
        evaluate(&mut runtime, &value_types, 2).intents[0].payload,
        RuntimeValue::Float(5.0)
    );
    runtime
        .update_filter_input(
            gate,
            &SocketId::new("condition"),
            RuntimeInputBinding::Constant(RuntimeValue::Bool(false)),
        )
        .unwrap();
    let held = evaluate(&mut runtime, &value_types, 3);
    assert!(held.diagnostics.is_empty(), "{:?}", held.diagnostics);
    assert_eq!(held.intents[0].payload, RuntimeValue::Float(5.0));
}

#[test]
fn processor_lifecycle_reset_clears_managed_hold_history() {
    let (formula, instance, gate) = gate_document("hold_last", None);
    let processor = Processor::new("Hold Mapping", instance);
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ProcessorRuntime::new(processor.id);
    assert!(runtime.compile(&processor, &formula, &compile_ctx));
    runtime
        .managed_formula
        .as_mut()
        .unwrap()
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    assert!(!runtime.needs_continuous_evaluation());
    runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::StateEnter(StateId::new()));
    runtime
        .managed_formula
        .as_mut()
        .unwrap()
        .update_filter_input(
            gate,
            &SocketId::new("condition"),
            RuntimeInputBinding::Constant(RuntimeValue::Bool(true)),
        )
        .unwrap();
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(endpoint_ref("module/value"), RuntimeValue::Float(5.0));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    assert_eq!(
        runtime
            .evaluate_processor(&processor, &eval_ctx(1, &inputs, &registries))
            .intents
            .len(),
        1
    );
    runtime
        .managed_formula
        .as_mut()
        .unwrap()
        .update_filter_input(
            gate,
            &SocketId::new("condition"),
            RuntimeInputBinding::Constant(RuntimeValue::Bool(false)),
        )
        .unwrap();
    assert_eq!(
        runtime
            .evaluate_processor(&processor, &eval_ctx(2, &inputs, &registries))
            .intents
            .len(),
        1
    );
    runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::StateEnter(StateId::new()));
    let reset = runtime.evaluate_processor(&processor, &eval_ctx(3, &inputs, &registries));
    assert!(reset.diagnostics.is_empty(), "{:?}", reset.diagnostics);
    assert!(reset.intents.is_empty());
}

struct TwoContexts {
    keys: Vec<ContextKey>,
}

impl ProcessorContextProvider for TwoContexts {
    fn available_axes(&self, _processor: ProcessorId) -> AxisSet {
        [ContextAxisId::new("device")].into_iter().collect()
    }

    fn iter_context_keys<'a>(
        &'a self,
        _processor: ProcessorId,
        _axes: &'a AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + 'a> {
        Box::new(self.keys.iter().cloned())
    }

    fn resolve_context_value(
        &self,
        _key: &ContextKey,
        _axis: &ContextAxisId,
        _path: &ContextValuePath,
    ) -> Option<RuntimeValue> {
        None
    }
}

#[test]
fn managed_smoothing_histories_remain_isolated_when_context_order_changes() {
    let (mut formula, mut instance) = formula_and_instance();
    formula
        .surface
        .managed_regions
        .iter_mut()
        .find(|region| region.id == ManagedRegionId::new("filters"))
        .unwrap()
        .filter_value_mode = ManagedFilterValueMode::Tuple;
    let mut smooth = managed_item_for_primitive(PrimitiveNodeKind::SmoothFilter);
    smooth.anode.config.set("method", RuntimeValue::String("sma".into()));
    smooth.anode.config.set("window", RuntimeValue::Int(2));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region("inputs", vec![input_item("Input", endpoint_ref("module/value"))]),
    );
    instance
        .managed_regions
        .regions
        .insert(ManagedRegionId::new("filters"), region("filters", vec![smooth]));
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Output", command_target("target/command"))]),
    );
    let processor = Processor::new("Context Mapping", instance);
    let (value_types, nodes) = registries();
    let compile_ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ProcessorRuntime::new(processor.id);
    assert!(runtime.compile(&processor, &formula, &compile_ctx));
    runtime
        .managed_formula
        .as_mut()
        .unwrap()
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    assert!(runtime.needs_continuous_evaluation());
    runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::StateEnter(StateId::new()));
    let axes: AxisSet = [ContextAxisId::new("device")].into_iter().collect();
    let a = ContextKey::single("device", "a");
    let b = ContextKey::single("device", "b");
    let provider = TwoContexts {
        keys: vec![a.clone(), b.clone()],
    };
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            input_axes: axes.clone(),
            ..Default::default()
        },
    );
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let mut first_inputs = RuntimeInputSnapshot::default();
    first_inputs.insert_context(endpoint_ref("module/value"), &axes, a.clone(), RuntimeValue::Float(2.0));
    first_inputs.insert_context(
        endpoint_ref("module/value"),
        &axes,
        b.clone(),
        RuntimeValue::Float(10.0),
    );
    let first = runtime.evaluate_processor_with_context_provider(
        &processor,
        &eval_ctx(1, &first_inputs, &registries),
        &provider,
    );
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].output.intents[0].payload, RuntimeValue::Float(2.0));
    assert_eq!(first[1].output.intents[0].payload, RuntimeValue::Float(10.0));

    let reordered = TwoContexts {
        keys: vec![b.clone(), a.clone()],
    };
    let mut second_inputs = RuntimeInputSnapshot::default();
    second_inputs.insert_context(
        endpoint_ref("module/value"),
        &axes,
        b.clone(),
        RuntimeValue::Float(30.0),
    );
    second_inputs.insert_context(endpoint_ref("module/value"), &axes, a.clone(), RuntimeValue::Float(6.0));
    let second = runtime.evaluate_processor_with_context_provider(
        &processor,
        &eval_ctx(2, &second_inputs, &registries),
        &reordered,
    );
    assert_eq!(
        second.iter().map(|lane| lane.context_key.as_ref()).collect::<Vec<_>>(),
        vec![Some(&b), Some(&a)]
    );
    assert_eq!(second[0].output.intents[0].payload, RuntimeValue::Float(20.0));
    assert_eq!(second[1].output.intents[0].payload, RuntimeValue::Float(4.0));

    let only_a = TwoContexts { keys: vec![a.clone()] };
    let mut third_inputs = RuntimeInputSnapshot::default();
    third_inputs.insert_context(endpoint_ref("module/value"), &axes, a.clone(), RuntimeValue::Float(8.0));
    let third =
        runtime.evaluate_processor_with_context_provider(&processor, &eval_ctx(3, &third_inputs, &registries), &only_a);
    assert_eq!(third.len(), 1);
    assert_eq!(third[0].output.intents[0].payload, RuntimeValue::Float(7.0));

    let mut fourth_inputs = RuntimeInputSnapshot::default();
    fourth_inputs.insert_context(
        endpoint_ref("module/value"),
        &axes,
        a.clone(),
        RuntimeValue::Float(10.0),
    );
    fourth_inputs.insert_context(
        endpoint_ref("module/value"),
        &axes,
        b.clone(),
        RuntimeValue::Float(40.0),
    );
    let fourth = runtime.evaluate_processor_with_context_provider(
        &processor,
        &eval_ctx(4, &fourth_inputs, &registries),
        &provider,
    );
    assert_eq!(fourth.len(), 2);
    assert_eq!(fourth[0].output.intents[0].payload, RuntimeValue::Float(9.0));
    assert_eq!(fourth[1].output.intents[0].payload, RuntimeValue::Float(40.0));

    let mut fifth_inputs = RuntimeInputSnapshot::default();
    fifth_inputs.insert_context(endpoint_ref("module/value"), &axes, a, RuntimeValue::Float(12.0));
    let fifth = runtime.evaluate_processor_with_context_provider(
        &processor,
        &eval_ctx(5, &fifth_inputs, &registries),
        &provider,
    );
    assert_eq!(fifth[0].output.intents[0].payload, RuntimeValue::Float(11.0));
    assert!(
        runtime.needs_continuous_evaluation(),
        "one valid context still has temporal work"
    );
    let missing = RuntimeInputSnapshot::default();
    runtime.evaluate_processor_with_context_provider(&processor, &eval_ctx(6, &missing, &registries), &provider);
    assert!(
        !runtime.needs_continuous_evaluation(),
        "missing inputs leave no due temporal work"
    );
}

#[test]
fn invalid_structural_revision_cannot_dispatch_the_previous_stage_chain() {
    let (mut formula, mut instance) = formula_and_instance();
    formula
        .surface
        .managed_regions
        .iter_mut()
        .find(|region| region.id == ManagedRegionId::new("filters"))
        .unwrap()
        .filter_value_mode = ManagedFilterValueMode::Tuple;
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("inputs"),
        region("inputs", vec![input_item("Input", endpoint_ref("module/value"))]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("filters"),
        region("filters", vec![remap_item(0.0, 10.0, 0.0, 1.0)]),
    );
    instance.managed_regions.regions.insert(
        ManagedRegionId::new("outputs"),
        region("outputs", vec![output_item("Output", command_target("target/command"))]),
    );
    let (value_types, nodes) = registries();
    let ctx = CompileCtx {
        value_types: &value_types,
        nodes: &nodes,
        properties: Some(&formula.properties),
    };
    let mut runtime = ManagedFormulaRuntime::compile(&formula, &instance, &ctx)
        .unwrap()
        .unwrap();
    runtime
        .reconcile_input_source_schema(|_| {
            Some(ChannelSourceSchema {
                value_type: "float".into(),
                metadata: Default::default(),
            })
        })
        .unwrap();
    assert_eq!(evaluate(&mut runtime, &value_types, 1).intents.len(), 1);

    assert!(
        runtime
            .reconcile_input_source_schema(|_| Some(ChannelSourceSchema {
                value_type: "bool".into(),
                metadata: Default::default(),
            }))
            .is_err()
    );
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(endpoint_ref("module/value"), RuntimeValue::Bool(true));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let after = runtime.evaluate(&eval_ctx(2, &inputs, &registries));
    assert!(after.intents.is_empty());
    assert!(
        after
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("bind a source schema"))
    );
}
