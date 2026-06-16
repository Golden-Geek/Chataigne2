use std::{sync::Arc, time::Duration};

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistGraph, AxisSet, CompileCtx, ContextAxisId, ContextKey,
    ContextValuePath, EvaluationCtx, FormulaContextContract, FormulaId, FormulaPropertyDecl, FormulaPropertyId,
    FormulaPropertySchema, FormulaSurface, InputSocketRef, OutputPreviewStatus, OutputSocketRef, RuntimeInputSnapshot,
    RuntimeOutput, RuntimeRegistries, RuntimeValue, SurfaceItem, SurfaceItemId, SurfaceItemKind, SurfaceSection,
    SurfaceSectionId, SurfaceSource, ValueTypeId, ValueTypeRegistry, primitive_node_registry,
};

use crate::{
    DefaultProcessorContextProvider, Processor, ProcessorBindingAnalysis, ProcessorContextProvider,
    ProcessorDebugCapture, ProcessorExecutionStrategy, ProcessorId, ProcessorLifecycleEvent, ProcessorRuntime,
};

fn formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
    let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    constant.config.set("value", RuntimeValue::Float(1.0));
    graph.add_node(constant).unwrap();
    formula_with_graph(graph)
}

fn stateful_formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
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
    let mut graph = AlchemistGraph::new();
    let mut property = ANodeInstance::new(ANodeTypeId::new("property"), "Property");
    property
        .config
        .set("property_id", RuntimeValue::String(property_id.into()));
    graph.add_node(property).unwrap();
    let mut formula = formula_with_graph(graph);
    formula.properties.insert(FormulaPropertyDecl {
        id: FormulaPropertyId::new(property_id),
        label: "Amount".into(),
        description: None,
        value_type: ValueTypeId::new("float"),
        default_value,
        ui: golden_alchemist::PropertyUiHints::default(),
    });
    formula
}

fn formula_with_graph(graph: AlchemistGraph) -> AlchemistFormula {
    AlchemistFormula {
        id: FormulaId::new("test"),
        version: 1,
        label: "Test".into(),
        description: None,
        tags: Vec::new(),
        graph,
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
        ProcessorLifecycleEvent::StateEnter(golden_statechart::StateId::new()),
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
        ProcessorLifecycleEvent::StateEnter(golden_statechart::StateId::new()),
    );
    assert_eq!(runtime.evaluate_processor(&processor, &ctx).debug_samples.len(), 1);
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

    let first = runtime.evaluate_processor(&processor, &first_ctx);
    assert_eq!(first_float(&first), Some(1.0));

    processor
        .formula_instance
        .overrides
        .values
        .insert(SurfaceItemId::new("amount"), RuntimeValue::Float(7.5));
    let second_ctx = evaluation_ctx(2, &inputs, &registries);
    let second = runtime.evaluate_processor(&processor, &second_ctx);

    assert_eq!(first_float(&second), Some(7.5));
    assert!(Arc::ptr_eq(&compiled, runtime.compiled.as_ref().unwrap()));
    assert_eq!(runtime.lanes.memory_count(), 0);
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
    assert_eq!(runtime.lanes.memory_count(), 0);
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
fn stateless_processor_bound_to_context_runs_lanes_without_memory() {
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
    assert_eq!(runtime.lanes.memory_count(), 0);
    let plan = runtime.plan.as_ref().unwrap();
    assert_eq!(plan.strategy, ProcessorExecutionStrategy::MultiStateless);
    assert!(plan.required_memory_axes.is_empty());
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

    let first = runtime.evaluate_processor_with_context_provider(&processor, &first_ctx, &provider);

    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|lane| trigger_fired(&lane.output)));
    assert_eq!(runtime.lanes.memory_count(), 2);
    let plan = runtime.plan.as_ref().unwrap();
    assert_eq!(plan.strategy, ProcessorExecutionStrategy::MultiStatefulSparse);
    assert_eq!(plan.required_memory_axes, axes);

    let reordered_provider = TestContextProvider::new(vec![b.clone(), a.clone()]);
    let second_ctx = evaluation_ctx(2, &inputs, &registries);
    let second = runtime.evaluate_processor_with_context_provider(&processor, &second_ctx, &reordered_provider);

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

    runtime.evaluate_processor_with_context_provider(&processor, &first_ctx, &provider);
    assert_eq!(runtime.lanes.memory_count(), 2);

    let b_only_provider = TestContextProvider::new(vec![b.clone()]);
    let second_ctx = evaluation_ctx(2, &inputs, &registries);
    let second = runtime.evaluate_processor_with_context_provider(&processor, &second_ctx, &b_only_provider);
    assert_eq!(second.len(), 1);
    assert!(!trigger_fired(&second[0].output));
    assert_eq!(runtime.lanes.memory_count(), 1);

    let restored_provider = TestContextProvider::new(vec![a.clone(), b.clone()]);
    let third_ctx = evaluation_ctx(3, &inputs, &registries);
    let third = runtime.evaluate_processor_with_context_provider(&processor, &third_ctx, &restored_provider);
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
    assert_eq!(runtime.lanes.memory_count(), 0);
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
            history_len: 2,
        },
    );

    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].context_key.as_ref(), Some(&selected));
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
        id: SurfaceSectionId::new("actions"),
        label: "Actions".into(),
        items: vec![SurfaceItem {
            id: SurfaceItemId::new("run"),
            label: "Run".into(),
            description: None,
            path: Vec::new(),
            kind: SurfaceItemKind::Action,
            value_type: None,
            ui: golden_alchemist::ParamUiHints::default(),
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
