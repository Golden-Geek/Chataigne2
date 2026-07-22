use crate::testkit::TestGraph;

use std::{sync::Arc, time::Duration};

use chataigne_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistFormula, AxisSet, CompileCtx, ContextAxisId, ContextKey, ContextValuePath,
    EvaluationCtx, FormulaContextContract, FormulaId, FormulaPropertyDecl, FormulaPropertyId, FormulaPropertySchema,
    FormulaSurface, InputSocketRef, ManagedItemId, ManagedItemInstance, ManagedItemUiState, ManagedRegionDefinition,
    ManagedRegionId, ManagedRegionKind, OutputPreviewStatus, OutputSocketRef, RuntimeInputSnapshot, RuntimeOutput,
    RuntimeRegistries, StableRef, SurfaceItem, SurfaceItemId, SurfaceItemKind, SurfaceSection, SurfaceSectionId,
    SurfaceSource, ValueTypeId, ValueTypeRegistry, primitive_node_registry,
};
use chataigne_condition::{ConditionDefinition, ConditionKind, TypedComparator};
use golden_values::Value as RuntimeValue;

use crate::{
    DefaultProcessorContextProvider, Processor, ProcessorBindingAnalysis, ProcessorContextProvider,
    ProcessorDebugCapture, ProcessorExecutionStrategy, ProcessorId, ProcessorLifecycleEvent, ProcessorRuntime,
};

fn formula() -> AlchemistFormula {
    let mut graph = TestGraph::new();
    let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    constant.config.set("value", RuntimeValue::Float(1.0));
    graph.add_node(constant).unwrap();
    formula_with_graph(graph)
}

fn stateful_formula() -> AlchemistFormula {
    let mut graph = TestGraph::new();
    let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    constant.config.set("value", RuntimeValue::Bool(true));
    let source = graph.add_node(constant).unwrap();
    let edge = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("trigger_on_off"), "Trigger On/Off"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    formula_with_graph(graph)
}

fn property_formula(property_id: &str, default_value: RuntimeValue) -> AlchemistFormula {
    let mut graph = TestGraph::new();
    let mut property = ANodeInstance::new(ANodeTypeId::new("property"), "Property");
    property.config.set(
        "property_id",
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), property_id)),
    );
    graph.add_node(property).unwrap();
    let mut formula = formula_with_graph(graph);
    formula.properties.insert(FormulaPropertyDecl {
        id: FormulaPropertyId::new(property_id),
        label: "Amount".into(),
        description: None,
        value_type: ValueTypeId::new("float"),
        default_value,
        ui: chataigne_alchemist::PropertyUiHints::default(),
    });
    formula
}

fn large_stateless_formula(node_count: usize) -> AlchemistFormula {
    let mut graph = TestGraph::new();
    for index in 0..node_count {
        let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), format!("Constant {index}"));
        constant.config.set("value", RuntimeValue::Float(index as f64));
        graph.add_node(constant).unwrap();
    }
    formula_with_graph(graph)
}

fn formula_with_graph(graph: TestGraph) -> AlchemistFormula {
    AlchemistFormula {
        id: FormulaId::new("test"),
        version: 1,
        label: "Test".into(),
        description: None,
        tags: Vec::new(),
        graph: graph.to_document().unwrap(),
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

#[derive(Clone, Debug)]
struct TestContextProvider {
    keys: Vec<ContextKey>,
    axes: AxisSet,
}

impl TestContextProvider {
    fn new(keys: Vec<ContextKey>) -> Self {
        let mut axes = AxisSet::new();
        axes.insert(ContextAxisId::new("device"));
        Self { keys, axes }
    }
}

fn device_context_keys(count: usize) -> Vec<ContextKey> {
    (0..count)
        .map(|index| ContextKey::single("device", format!("lane-{index}")))
        .collect()
}

impl ProcessorContextProvider for TestContextProvider {
    fn available_axes(&self, _processor_id: ProcessorId) -> AxisSet {
        self.axes.clone()
    }

    fn iter_context_keys<'a>(
        &'a self,
        _processor_id: ProcessorId,
        axes: &'a AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + 'a> {
        if axes.is_empty() {
            Box::new(std::iter::once(ContextKey::default_lane()))
        } else {
            Box::new(self.keys.clone().into_iter())
        }
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

fn compile_active_runtime(formula: &AlchemistFormula) -> (Processor, ProcessorRuntime) {
    let processor = Processor::from_formula("Processor", formula);
    let mut runtime = ProcessorRuntime::new(processor.id);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    assert!(runtime.compile(
        &processor,
        formula,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: Some(&formula.properties),
        }
    ));
    runtime.apply_lifecycle(
        &processor,
        ProcessorLifecycleEvent::StateEnter(chataigne_state_machine_model::StateId::new()),
    );
    (processor, runtime)
}

fn evaluation_ctx<'a>(
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

fn trigger_fired(output: &RuntimeOutput) -> bool {
    output
        .debug_samples
        .iter()
        .any(|sample| matches!(&sample.value, RuntimeValue::Trigger(trigger) if trigger.fired))
}

fn first_float(output: &RuntimeOutput) -> Option<f64> {
    output.debug_samples.iter().find_map(|sample| match sample.value {
        RuntimeValue::Float(value) => Some(value),
        _ => None,
    })
}

fn capture_all() -> ProcessorDebugCapture {
    ProcessorDebugCapture::All { history_len: 64 }
}

fn evaluate_default_lane_with_capture(
    runtime: &mut ProcessorRuntime,
    processor: &Processor,
    ctx: &EvaluationCtx<'_>,
) -> RuntimeOutput {
    let provider = DefaultProcessorContextProvider;
    let mut lanes =
        runtime.evaluate_processor_with_context_provider_and_capture(processor, ctx, &provider, &capture_all());
    assert_eq!(lanes.len(), 1);
    lanes.remove(0).output
}

#[test]
fn compiled_condition_program_gates_processor_without_walking_authoring_nodes() {
    let formula = formula();
    let source = StableRef::new(ValueTypeId::new("float"), "module/input");
    let mut condition = ConditionDefinition::input_value("threshold", source.clone(), RuntimeValue::Float(0.5));
    let ConditionKind::InputValue(input) = &mut condition.kind else {
        unreachable!();
    };
    input.comparator = TypedComparator::Greater;
    let mut processor = Processor::from_formula("Processor", &formula);
    processor.condition = Some(condition);
    let mut runtime = ProcessorRuntime::new(processor.id);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    assert!(runtime.compile(
        &processor,
        &formula,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: Some(&formula.properties),
        }
    ));
    runtime.apply_lifecycle(
        &processor,
        ProcessorLifecycleEvent::StateEnter(chataigne_state_machine_model::StateId::new()),
    );
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let mut inputs = RuntimeInputSnapshot::default();
    inputs.insert(source.clone(), RuntimeValue::Float(0.25));
    let provider = DefaultProcessorContextProvider;
    assert!(
        runtime
            .evaluate_processor_with_context_provider(&processor, &evaluation_ctx(1, &inputs, &registries), &provider,)
            .is_empty()
    );

    inputs.insert(source, RuntimeValue::Float(0.75));
    assert!(
        !runtime
            .evaluate_processor_with_context_provider(&processor, &evaluation_ctx(2, &inputs, &registries), &provider,)
            .is_empty()
    );
}

#[test]
fn processor_compiles_and_evaluates_only_while_active() {
    let formula = formula();
    let processor = Processor::from_formula("Processor", &formula);
    let mut runtime = ProcessorRuntime::new(processor.id);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    assert!(runtime.compile(
        &processor,
        &formula,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: Some(&formula.properties),
        }
    ));
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = EvaluationCtx {
        logical_tick: 1,
        delta_time: Duration::ZERO,
        events: &[],
        inputs: &inputs,
        registries: &registries,
    };

    assert!(runtime.evaluate_processor(&processor, &ctx).debug_samples.is_empty());
    runtime.apply_lifecycle(
        &processor,
        ProcessorLifecycleEvent::StateEnter(chataigne_state_machine_model::StateId::new()),
    );
    assert!(runtime.evaluate_processor(&processor, &ctx).debug_samples.is_empty());
    assert_eq!(
        evaluate_default_lane_with_capture(&mut runtime, &processor, &ctx)
            .debug_samples
            .len(),
        1
    );
}

#[test]
fn override_change_rebuilds_property_frame_not_formula() {
    let formula = property_formula("amount", RuntimeValue::Float(1.0));
    let (mut processor, mut runtime) = compile_active_runtime(&formula);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let first_ctx = evaluation_ctx(1, &inputs, &registries);
    let compiled = Arc::clone(runtime.compiled.as_ref().unwrap());

    let first = evaluate_default_lane_with_capture(&mut runtime, &processor, &first_ctx);
    assert_eq!(first_float(&first), Some(1.0));

    processor
        .formula_instance
        .overrides
        .values
        .insert(SurfaceItemId::new("amount"), RuntimeValue::Float(7.5));
    let second_ctx = evaluation_ctx(2, &inputs, &registries);
    let second = evaluate_default_lane_with_capture(&mut runtime, &processor, &second_ctx);

    assert_eq!(first_float(&second), Some(7.5));
    assert!(Arc::ptr_eq(&compiled, runtime.compiled.as_ref().unwrap()));
    assert_eq!(runtime.lanes.memory_count(), 1);
}

#[test]
fn processor_under_multiplex_without_context_reference_runs_one_lane() {
    let formula = formula();
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = evaluation_ctx(1, &inputs, &registries);
    let provider = TestContextProvider::new(vec![
        ContextKey::single("device", "a"),
        ContextKey::single("device", "b"),
    ]);

    let outputs = runtime.evaluate_processor_with_context_provider(&processor, &ctx, &provider);

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].context_key, None);
    assert_eq!(runtime.lanes.memory_count(), 1);
    assert_eq!(
        runtime.plan.as_ref().unwrap().strategy,
        ProcessorExecutionStrategy::SingleStateless
    );
    assert!(
        provider
            .available_axes(processor.id)
            .contains(&ContextAxisId::new("device"))
    );
}

#[test]
fn stateless_processor_bound_to_context_runs_lanes_with_input_process_cache() {
    let formula = formula();
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = evaluation_ctx(1, &inputs, &registries);
    let provider = TestContextProvider::new(vec![
        ContextKey::single("device", "a"),
        ContextKey::single("device", "b"),
    ]);
    let axes = provider.available_axes(processor.id);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            input_axes: axes,
            ..ProcessorBindingAnalysis::default()
        },
    );

    let outputs = runtime.evaluate_processor_with_context_provider(&processor, &ctx, &provider);

    assert_eq!(outputs.len(), 2);
    assert!(outputs.iter().all(|lane| lane.context_key.is_some()));
    assert_eq!(runtime.lanes.memory_count(), 2);
    let plan = runtime.plan.as_ref().unwrap();
    assert_eq!(plan.strategy, ProcessorExecutionStrategy::MultiStateless);
    assert!(plan.required_memory_axes.contains(&ContextAxisId::new("device")));
}

#[test]
fn stateful_lanes_have_independent_memory_preserved_by_stable_key() {
    let formula = stateful_formula();
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let a = ContextKey::single("device", "a");
    let b = ContextKey::single("device", "b");
    let provider = TestContextProvider::new(vec![a.clone(), b.clone()]);
    let axes = provider.available_axes(processor.id);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            input_axes: axes.clone(),
            ..ProcessorBindingAnalysis::default()
        },
    );
    let first_ctx = evaluation_ctx(1, &inputs, &registries);

    let first =
        runtime.evaluate_processor_with_context_provider_and_capture(&processor, &first_ctx, &provider, &capture_all());

    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|lane| trigger_fired(&lane.output)));
    assert_eq!(runtime.lanes.memory_count(), 2);
    let plan = runtime.plan.as_ref().unwrap();
    assert_eq!(plan.strategy, ProcessorExecutionStrategy::MultiStatefulSparse);
    assert_eq!(plan.required_memory_axes, axes);

    let reordered_provider = TestContextProvider::new(vec![b.clone(), a.clone()]);
    let second_ctx = evaluation_ctx(2, &inputs, &registries);
    let second = runtime.evaluate_processor_with_context_provider_and_capture(
        &processor,
        &second_ctx,
        &reordered_provider,
        &capture_all(),
    );

    assert_eq!(
        second.iter().map(|lane| lane.context_key.as_ref()).collect::<Vec<_>>(),
        vec![Some(&b), Some(&a)]
    );
    assert!(second.iter().all(|lane| !trigger_fired(&lane.output)));
    assert_eq!(runtime.lanes.memory_count(), 2);
}

#[test]
fn removed_context_item_evicts_lane_memory() {
    let formula = stateful_formula();
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let a = ContextKey::single("device", "a");
    let b = ContextKey::single("device", "b");
    let provider = TestContextProvider::new(vec![a.clone(), b.clone()]);
    let axes = provider.available_axes(processor.id);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            input_axes: axes,
            ..ProcessorBindingAnalysis::default()
        },
    );
    let first_ctx = evaluation_ctx(1, &inputs, &registries);

    runtime.evaluate_processor_with_context_provider_and_capture(&processor, &first_ctx, &provider, &capture_all());
    assert_eq!(runtime.lanes.memory_count(), 2);

    let b_only_provider = TestContextProvider::new(vec![b.clone()]);
    let second_ctx = evaluation_ctx(2, &inputs, &registries);
    let second = runtime.evaluate_processor_with_context_provider_and_capture(
        &processor,
        &second_ctx,
        &b_only_provider,
        &capture_all(),
    );
    assert_eq!(second.len(), 1);
    assert!(!trigger_fired(&second[0].output));
    assert_eq!(runtime.lanes.memory_count(), 1);

    let restored_provider = TestContextProvider::new(vec![a.clone(), b.clone()]);
    let third_ctx = evaluation_ctx(3, &inputs, &registries);
    let third = runtime.evaluate_processor_with_context_provider_and_capture(
        &processor,
        &third_ctx,
        &restored_provider,
        &capture_all(),
    );
    let a_lane = third
        .iter()
        .find(|lane| lane.context_key.as_ref() == Some(&a))
        .expect("restored context item should be evaluated");
    let b_lane = third
        .iter()
        .find(|lane| lane.context_key.as_ref() == Some(&b))
        .expect("preserved context item should be evaluated");

    assert!(trigger_fired(&a_lane.output));
    assert!(!trigger_fired(&b_lane.output));
    assert_eq!(runtime.lanes.memory_count(), 2);
}

#[test]
fn processor_preview_shows_override_resolved_values() {
    let formula = property_formula("amount", RuntimeValue::Float(1.0));
    let (mut processor, mut runtime) = compile_active_runtime(&formula);
    processor
        .formula_instance
        .overrides
        .values
        .insert(SurfaceItemId::new("amount"), RuntimeValue::Float(7.5));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = evaluation_ctx(5, &inputs, &registries);
    let provider = DefaultProcessorContextProvider;

    let samples = runtime.evaluate_processor_preview_with_context_provider(
        &processor,
        &ctx,
        &provider,
        &ProcessorDebugCapture::ProcessorLane {
            context_key: None,
            history_len: 4,
        },
    );

    assert_eq!(samples.len(), 1);
    assert_eq!(&samples[0].formula_id, &formula.id);
    assert_eq!(samples[0].processor_id, Some(processor.id));
    assert!(samples[0].context_key.is_none());
    assert_eq!(&samples[0].value, &RuntimeValue::Float(7.5));
    assert_eq!(samples[0].status, OutputPreviewStatus::Live);
}

#[test]
fn multiplexed_processor_preview_filters_to_selected_lane() {
    let formula = formula();
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = evaluation_ctx(1, &inputs, &registries);
    let selected = ContextKey::single("device", "b");
    let provider = TestContextProvider::new(vec![
        ContextKey::single("device", "a"),
        selected.clone(),
        ContextKey::single("device", "c"),
    ]);
    let axes = provider.available_axes(processor.id);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            output_axes: axes,
            ..ProcessorBindingAnalysis::default()
        },
    );

    let samples = runtime.evaluate_processor_preview_with_context_provider(
        &processor,
        &ctx,
        &provider,
        &ProcessorDebugCapture::ProcessorLane {
            context_key: Some(selected.clone()),
            history_len: 8,
        },
    );

    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].processor_id, Some(processor.id));
    assert_eq!(samples[0].context_key.as_ref(), Some(&selected));
}

#[test]
fn changing_selected_lane_changes_preview_samples_only() {
    let formula = formula();
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let compiled = Arc::clone(runtime.compiled.as_ref().unwrap());
    let original_override_count = processor.formula_instance.overrides.values.len();
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let provider = TestContextProvider::new(vec![
        ContextKey::single("device", "a"),
        ContextKey::single("device", "b"),
    ]);
    let axes = provider.available_axes(processor.id);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            output_axes: axes,
            ..ProcessorBindingAnalysis::default()
        },
    );
    let a = ContextKey::single("device", "a");
    let b = ContextKey::single("device", "b");

    let first = runtime.evaluate_processor_preview_with_context_provider(
        &processor,
        &evaluation_ctx(1, &inputs, &registries),
        &provider,
        &ProcessorDebugCapture::ProcessorLane {
            context_key: Some(a.clone()),
            history_len: 4,
        },
    );
    let second = runtime.evaluate_processor_preview_with_context_provider(
        &processor,
        &evaluation_ctx(2, &inputs, &registries),
        &provider,
        &ProcessorDebugCapture::ProcessorLane {
            context_key: Some(b.clone()),
            history_len: 4,
        },
    );

    assert_eq!(first[0].context_key.as_ref(), Some(&a));
    assert_eq!(second[0].context_key.as_ref(), Some(&b));
    assert!(Arc::ptr_eq(&compiled, runtime.compiled.as_ref().unwrap()));
    assert_eq!(
        processor.formula_instance.overrides.values.len(),
        original_override_count
    );
    assert_eq!(runtime.lanes.memory_count(), 1);
}

#[test]
fn large_graph_preview_does_not_capture_all_lanes() {
    let formula = formula();
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let selected = ContextKey::single("device", "lane-7");
    let provider = TestContextProvider::new(
        (0..10)
            .map(|index| ContextKey::single("device", format!("lane-{index}")))
            .collect(),
    );
    let axes = provider.available_axes(processor.id);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            output_axes: axes,
            ..ProcessorBindingAnalysis::default()
        },
    );

    let samples = runtime.evaluate_processor_preview_with_context_provider(
        &processor,
        &evaluation_ctx(1, &inputs, &registries),
        &provider,
        &ProcessorDebugCapture::ProcessorLane {
            context_key: Some(selected.clone()),
            history_len: 128,
        },
    );

    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].context_key.as_ref(), Some(&selected));
}

#[test]
fn ten_thousand_stateless_processors_share_compile_and_allocate_one_process_cache() {
    let formula = formula();
    let (_first_processor, first_runtime) = compile_active_runtime(&formula);
    let compiled = Arc::clone(first_runtime.compiled.as_ref().unwrap());
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = evaluation_ctx(1, &inputs, &registries);

    for index in 0..10_000 {
        let processor = Processor::from_formula(format!("Processor {index}"), &formula);
        let mut runtime = ProcessorRuntime::new(processor.id);
        assert!(runtime.compile_from_shared_formula(&processor, &formula, Arc::clone(&compiled)));
        runtime.apply_lifecycle(
            &processor,
            ProcessorLifecycleEvent::StateEnter(chataigne_state_machine_model::StateId::new()),
        );

        let output = runtime.evaluate_processor(&processor, &ctx);

        assert!(Arc::ptr_eq(runtime.compiled.as_ref().unwrap(), &compiled));
        assert!(output.debug_samples.is_empty());
        assert_eq!(runtime.lanes.memory_count(), 1);
    }
}

#[test]
fn thousand_stateful_processors_allocate_sparse_lanes_only() {
    let formula = stateful_formula();
    let (_first_processor, first_runtime) = compile_active_runtime(&formula);
    let compiled = Arc::clone(first_runtime.compiled.as_ref().unwrap());
    let provider = TestContextProvider::new(device_context_keys(3));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = evaluation_ctx(1, &inputs, &registries);

    for index in 0..1_000 {
        let processor = Processor::from_formula(format!("Processor {index}"), &formula);
        let mut runtime = ProcessorRuntime::new(processor.id);
        assert!(runtime.compile_from_shared_formula(&processor, &formula, Arc::clone(&compiled)));
        runtime.apply_lifecycle(
            &processor,
            ProcessorLifecycleEvent::StateEnter(chataigne_state_machine_model::StateId::new()),
        );
        runtime.rebuild_execution_plan(
            &provider,
            &ProcessorBindingAnalysis {
                input_axes: provider.available_axes(processor.id),
                ..ProcessorBindingAnalysis::default()
            },
        );

        let lanes = runtime.evaluate_processor_with_context_provider(&processor, &ctx, &provider);

        assert_eq!(lanes.len(), 3);
        assert!(lanes.iter().all(|lane| lane.output.debug_samples.is_empty()));
        assert_eq!(runtime.lanes.memory_count(), 3);
    }
}

#[test]
fn processor_scale_p50_l1_shares_one_compiled_kernel() {
    let formula = formula();
    let (_first_processor, first_runtime) = compile_active_runtime(&formula);
    let compiled = Arc::clone(first_runtime.compiled.as_ref().unwrap());
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = evaluation_ctx(1, &inputs, &registries);

    for index in 0..50 {
        let processor = Processor::from_formula(format!("P50-L1 {index}"), &formula);
        let mut runtime = ProcessorRuntime::new(processor.id);
        assert!(runtime.compile_from_shared_formula(&processor, &formula, Arc::clone(&compiled)));
        runtime.apply_lifecycle(
            &processor,
            ProcessorLifecycleEvent::StateEnter(chataigne_state_machine_model::StateId::new()),
        );
        let output = runtime.evaluate_processor(&processor, &ctx);
        assert!(Arc::ptr_eq(runtime.compiled.as_ref().unwrap(), &compiled));
        assert!(output.debug_samples.is_empty());
        assert_eq!(runtime.lanes.memory_count(), 1);
    }
}

#[test]
fn processor_scale_p5_l127_preserves_sparse_lane_state() {
    let formula = stateful_formula();
    let (_first_processor, first_runtime) = compile_active_runtime(&formula);
    let compiled = Arc::clone(first_runtime.compiled.as_ref().unwrap());
    let provider = TestContextProvider::new(device_context_keys(127));
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let ctx = evaluation_ctx(1, &inputs, &registries);

    for index in 0..5 {
        let processor = Processor::from_formula(format!("P5-L127 {index}"), &formula);
        let mut runtime = ProcessorRuntime::new(processor.id);
        assert!(runtime.compile_from_shared_formula(&processor, &formula, Arc::clone(&compiled)));
        runtime.apply_lifecycle(
            &processor,
            ProcessorLifecycleEvent::StateEnter(chataigne_state_machine_model::StateId::new()),
        );
        runtime.rebuild_execution_plan(
            &provider,
            &ProcessorBindingAnalysis {
                input_axes: provider.available_axes(processor.id),
                ..ProcessorBindingAnalysis::default()
            },
        );

        let lanes = runtime.evaluate_processor_with_context_provider(&processor, &ctx, &provider);
        assert_eq!(lanes.len(), 127);
        assert_eq!(runtime.lanes.memory_count(), 127);
        assert!(lanes.iter().all(|lane| lane.output.debug_samples.is_empty()));
    }
}

#[test]
fn selected_lane_preview_in_large_graph_captures_only_selected_lane() {
    let formula = large_stateless_formula(128);
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let selected = ContextKey::single("device", "lane-777");
    let provider = TestContextProvider::new(device_context_keys(1_000));
    let axes = provider.available_axes(processor.id);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            output_axes: axes,
            ..ProcessorBindingAnalysis::default()
        },
    );
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();

    let samples = runtime.evaluate_processor_preview_with_context_provider(
        &processor,
        &evaluation_ctx(1, &inputs, &registries),
        &provider,
        &ProcessorDebugCapture::ProcessorLane {
            context_key: Some(selected.clone()),
            history_len: 128,
        },
    );

    assert_eq!(samples.len(), 128);
    assert!(
        samples
            .iter()
            .all(|sample| sample.context_key.as_ref() == Some(&selected))
    );
    assert_eq!(runtime.lanes.memory_count(), 1);
}

#[test]
fn processor_preview_capture_off_when_editor_not_visible() {
    let formula = formula();
    let (processor, mut runtime) = compile_active_runtime(&formula);
    let value_types = ValueTypeRegistry::with_primitives();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let inputs = RuntimeInputSnapshot::default();
    let provider = TestContextProvider::new(vec![
        ContextKey::single("device", "a"),
        ContextKey::single("device", "b"),
    ]);
    let axes = provider.available_axes(processor.id);
    runtime.rebuild_execution_plan(
        &provider,
        &ProcessorBindingAnalysis {
            output_axes: axes,
            ..ProcessorBindingAnalysis::default()
        },
    );

    let lanes = runtime.evaluate_processor_with_context_provider_and_capture(
        &processor,
        &evaluation_ctx(1, &inputs, &registries),
        &provider,
        &ProcessorDebugCapture::Off,
    );

    assert_eq!(lanes.len(), 2);
    assert!(lanes.iter().all(|lane| lane.output.debug_samples.is_empty()));
}

#[test]
fn formula_surface_is_present_in_ui_model() {
    let mut formula = formula();
    formula.surface.sections.push(SurfaceSection {
        id: SurfaceSectionId::new("commands"),
        label: "Commands".into(),
        items: vec![SurfaceItem {
            id: SurfaceItemId::new("run"),
            label: "Run".into(),
            description: None,
            path: Vec::new(),
            kind: SurfaceItemKind::Command,
            value_type: None,
            ui: chataigne_alchemist::ParamUiHints::default(),
            bindings: Vec::new(),
        }],
        source: SurfaceSource::Formula,
    });

    let processor = Processor::from_formula("Processor", &formula);
    let ui = processor.ui_model(&formula, Vec::new());
    assert_eq!(ui.formula_id, "test");
    assert_eq!(ui.formula_label, "Test");
    assert_eq!(ui.surface.sections[0].items.len(), 1);
}

#[test]
fn managed_regions_are_present_in_ui_model() {
    let mut formula = formula();
    formula.surface.managed_regions.push(ManagedRegionDefinition {
        id: ManagedRegionId::new("filters"),
        kind: ManagedRegionKind::FilterPipeline,
        label: "Filters".into(),
        input_socket: None,
        output_socket: None,
        accepted_roles: vec![SurfaceItemKind::Filter],
    });

    let mut processor = Processor::from_formula("Processor", &formula);
    let mut remap = ANodeInstance::new(ANodeTypeId::new("remap"), "Remap");
    remap.enabled = false;
    processor
        .formula_instance
        .managed_regions
        .regions
        .get_mut(&ManagedRegionId::new("filters"))
        .unwrap()
        .items
        .push(ManagedItemInstance {
            id: ManagedItemId::new(),
            anode: remap,
            enabled: true,
            ui_state: ManagedItemUiState { collapsed: true },
        });

    let ui = processor.ui_model(&formula, Vec::new());

    assert!(ui.active);
    assert_eq!(ui.surface.managed_regions.len(), 1);
    assert_eq!(ui.surface.managed_regions[0].label, "Filters");
    let region = ui
        .managed_region_instances
        .regions
        .get(&ManagedRegionId::new("filters"))
        .unwrap();
    assert_eq!(region.items.len(), 1);
    assert_eq!(region.items[0].anode.label, "Remap");
    assert!(!region.items[0].anode.enabled);
    assert!(region.items[0].enabled);
    assert!(region.items[0].ui_state.collapsed);
}
