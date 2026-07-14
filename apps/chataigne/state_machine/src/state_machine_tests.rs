use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use golden_alchemist::{
    ANodeDeclaration, ANodeInstance, ANodeSignature, ANodeTypeId, AlchemistFormula, AlchemistGraph, AxisSet,
    CompileCtx, CompiledNodeEvaluator, CompiledNodeOperation, ContextAxisId, ContextKey, ContextValuePath,
    EvaluationCtx, ExecutionKind, FormulaContextContract, FormulaId, FormulaPropertySchema, FormulaSurface,
    InputSocketRef, LaneRuntimePool, NodeEvaluation, OutputSocketDecl, OutputSocketRef, ResolvedANodeSignature,
    RuntimeInputSnapshot, RuntimeIntent, RuntimeRegistries, SignatureCtx, StableRef, TriggerValue, TypeBindings,
    TypeConstraint, ValueTypeId, ValueTypeRegistry, primitive_node_registry,
};
use golden_statechart::Statechart;
use golden_values::Value as RuntimeValue;

use crate::{
    ChataigneStateMachine, ChataigneStateMachineRuntime, Processor, ProcessorContextProvider, ProcessorId,
    alchemist::{register_nodes, register_value_types},
};

fn constant_formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
    let mut node = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    node.config.set("value", RuntimeValue::Float(1.0));
    graph.add_node(node).unwrap();
    AlchemistFormula {
        id: FormulaId::new("constant"),
        version: 1,
        label: "Constant".into(),
        description: None,
        tags: Vec::new(),
        graph,
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

fn stateful_formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
    let mut source = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    source.config.set("value", RuntimeValue::Bool(true));
    let source = graph.add_node(source).unwrap();
    let edge = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("trigger_on_off"), "Trigger On/Off"))
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    AlchemistFormula {
        id: FormulaId::new("stateful"),
        version: 1,
        label: "Stateful".into(),
        description: None,
        tags: Vec::new(),
        graph,
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

fn context_formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
    graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("context_source"), "Context Source"))
        .unwrap();
    AlchemistFormula {
        id: FormulaId::new("context"),
        version: 1,
        label: "Context".into(),
        description: None,
        tags: Vec::new(),
        graph,
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

fn command_formula() -> AlchemistFormula {
    let mut graph = AlchemistGraph::new();
    graph
        .add_node(ANodeInstance::new(
            ANodeTypeId::new("command_emitter"),
            "Command Emitter",
        ))
        .unwrap();
    AlchemistFormula {
        id: FormulaId::new("command"),
        version: 1,
        label: "Command".into(),
        description: None,
        tags: Vec::new(),
        graph,
        properties: FormulaPropertySchema::default(),
        surface: FormulaSurface::default(),
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

fn counting_graph(type_id: &str) -> AlchemistGraph {
    let mut graph = AlchemistGraph::new();
    graph
        .add_node(ANodeInstance::new(ANodeTypeId::new(type_id), type_id))
        .unwrap();
    graph
}

#[derive(Clone, Debug)]
struct CountingTriggerDeclaration {
    type_id: &'static str,
    count: Arc<AtomicUsize>,
    fired: bool,
}

impl CountingTriggerDeclaration {
    fn new(type_id: &'static str, count: Arc<AtomicUsize>, fired: bool) -> Self {
        Self { type_id, count, fired }
    }
}

impl ANodeDeclaration for CountingTriggerDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new(self.type_id)
    }

    fn label(&self) -> &'static str {
        self.type_id
    }

    fn category(&self) -> &'static str {
        "Test"
    }

    fn execution_kind(&self) -> ExecutionKind {
        ExecutionKind::Pure
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        ANodeSignature {
            outputs: vec![OutputSocketDecl::new(
                "fired",
                "Fired",
                TypeConstraint::Exact(ValueTypeId::new("trigger")),
            )],
            ..ANodeSignature::default()
        }
    }

    fn compile_operation(
        &self,
        _instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, golden_alchemist::Diagnostic> {
        Ok(CompiledNodeOperation::Custom(Arc::new(CountingTriggerEval {
            count: Arc::clone(&self.count),
            fired: self.fired,
        })))
    }
}

#[derive(Debug)]
struct CountingTriggerEval {
    count: Arc<AtomicUsize>,
    fired: bool,
}

impl CompiledNodeEvaluator for CountingTriggerEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        self.count.fetch_add(1, Ordering::SeqCst);
        let trigger = if self.fired {
            TriggerValue::fired(
                u64::from(evaluation.exec_node.index() as u32),
                evaluation.ctx.logical_tick,
            )
        } else {
            TriggerValue::default()
        };
        Ok(vec![RuntimeValue::Trigger(trigger)])
    }
}

#[derive(Clone, Debug)]
struct ContextSourceDeclaration;

impl ANodeDeclaration for ContextSourceDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new("context_source")
    }

    fn label(&self) -> &'static str {
        "Context Source"
    }

    fn category(&self) -> &'static str {
        "Test"
    }

    fn execution_kind(&self) -> ExecutionKind {
        ExecutionKind::Pure
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        ANodeSignature {
            outputs: vec![OutputSocketDecl::new(
                "value",
                "Value",
                TypeConstraint::Exact(ValueTypeId::new("float")),
            )],
            ..ANodeSignature::default()
        }
    }

    fn context_axes(&self, _instance: &ANodeInstance, _resolved: &ResolvedANodeSignature) -> AxisSet {
        let mut axes = AxisSet::new();
        axes.insert(ContextAxisId::new("device"));
        axes
    }

    fn compile_operation(
        &self,
        _instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, golden_alchemist::Diagnostic> {
        Ok(CompiledNodeOperation::Constant(RuntimeValue::Float(1.0)))
    }
}

#[derive(Clone, Debug)]
struct CommandEmitterDeclaration;

impl ANodeDeclaration for CommandEmitterDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new("command_emitter")
    }

    fn label(&self) -> &'static str {
        "Command Emitter"
    }

    fn category(&self) -> &'static str {
        "Test"
    }

    fn execution_kind(&self) -> ExecutionKind {
        ExecutionKind::EffectEmitter
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        ANodeSignature::default()
    }

    fn context_axes(&self, _instance: &ANodeInstance, _resolved: &ResolvedANodeSignature) -> AxisSet {
        let mut axes = AxisSet::new();
        axes.insert(ContextAxisId::new("device"));
        axes
    }

    fn compile_operation(
        &self,
        _instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, golden_alchemist::Diagnostic> {
        Ok(CompiledNodeOperation::Custom(Arc::new(CommandEmitterEval)))
    }
}

#[derive(Debug)]
struct CommandEmitterEval;

impl CompiledNodeEvaluator for CommandEmitterEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        evaluation.intents.push(RuntimeIntent {
            kind: "chataigne.command".into(),
            source_node: Some(evaluation.author_node_id),
            source_socket: None,
            target: Some(StableRef::new(ValueTypeId::new("chataigne.command_target"), "target")),
            payload: RuntimeValue::Float(1.0),
            logical_tick: evaluation.ctx.logical_tick,
        });
        Ok(Vec::new())
    }
}

#[derive(Clone, Debug)]
struct TestContextProvider {
    keys: Vec<ContextKey>,
    axes: AxisSet,
}

impl TestContextProvider {
    fn with_device_count(count: usize) -> Self {
        let mut axes = AxisSet::new();
        axes.insert(ContextAxisId::new("device"));
        let keys = (0..count)
            .map(|index| ContextKey::single("device", format!("device-{index}")))
            .collect();
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

#[test]
fn two_processors_share_same_compiled_formula_arc() {
    let mut chart = Statechart::new();
    let state = chart.add_leaf(chart.root_region, "Only").unwrap();
    chart.set_initial(chart.root_region, state).unwrap();
    let mut machine = ChataigneStateMachine::new(chart);
    let formula = stateful_formula();
    let first_processor = Processor::from_formula("First Processor", &formula);
    let first_id = first_processor.id;
    let second_processor = Processor::from_formula("Second Processor", &formula);
    let second_id = second_processor.id;
    machine.add_formula(formula);
    machine.add_processor(state, first_processor).unwrap();
    machine.add_processor(state, second_processor).unwrap();

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let runtime = ChataigneStateMachineRuntime::compile(
        &machine,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    )
    .unwrap();

    let first = &runtime.processor_runtimes[&first_id];
    let second = &runtime.processor_runtimes[&second_id];
    assert!(Arc::ptr_eq(
        first.compiled.as_ref().unwrap(),
        second.compiled.as_ref().unwrap()
    ));
    match (&first.lanes, &second.lanes) {
        (LaneRuntimePool::Stateful(first_lanes), LaneRuntimePool::Stateful(second_lanes)) => {
            assert_ne!(first_lanes as *const _, second_lanes as *const _);
        }
        _ => panic!("stateful processors should own separate sparse lane pools"),
    }
}

#[test]
fn guard_evaluates_once_even_when_active_processors_have_30_lanes() {
    let mut chart = Statechart::new();
    let first = chart.add_leaf(chart.root_region, "First").unwrap();
    let second = chart.add_leaf(chart.root_region, "Second").unwrap();
    chart.set_initial(chart.root_region, first).unwrap();
    let transition = chart.add_transition(first, second, 0).unwrap();
    let mut machine = ChataigneStateMachine::new(chart);
    let formula = context_formula();
    let processor = Processor::from_formula("Context Processor", &formula);
    let processor_id = processor.id;
    machine.add_formula(formula);
    machine.add_processor(first, processor).unwrap();

    let guard_count = Arc::new(AtomicUsize::new(0));
    machine.set_transition_graphs(transition, Some(counting_graph("counting_guard")), None);

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    nodes.register(ContextSourceDeclaration).unwrap();
    nodes
        .register(CountingTriggerDeclaration::new(
            "counting_guard",
            Arc::clone(&guard_count),
            false,
        ))
        .unwrap();
    let mut runtime = ChataigneStateMachineRuntime::compile(
        &machine,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    )
    .unwrap();
    runtime.initialize(&mut machine).unwrap();
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let provider = TestContextProvider::with_device_count(30);

    let output = runtime
        .tick_with_context_provider(
            &mut machine,
            &EvaluationCtx {
                logical_tick: 1,
                delta_time: Duration::ZERO,
                events: &[],
                inputs: &inputs,
                registries: &registries,
            },
            &provider,
        )
        .unwrap();

    assert!(output.transition.is_none());
    assert_eq!(guard_count.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.execution.active_scopes, machine.chart.active.active_scopes);
    assert!(output.processor_outputs[&processor_id].debug_samples.is_empty());
}

#[test]
fn transition_effect_runs_once_after_transition() {
    let mut chart = Statechart::new();
    let first = chart.add_leaf(chart.root_region, "First").unwrap();
    let second = chart.add_leaf(chart.root_region, "Second").unwrap();
    chart.set_initial(chart.root_region, first).unwrap();
    let transition = chart.add_transition(first, second, 0).unwrap();
    let mut machine = ChataigneStateMachine::new(chart);
    let formula = context_formula();
    let processor = Processor::from_formula("Context Processor", &formula);
    let processor_id = processor.id;
    machine.add_formula(formula);
    machine.add_processor(second, processor).unwrap();

    let guard_count = Arc::new(AtomicUsize::new(0));
    let effect_count = Arc::new(AtomicUsize::new(0));
    machine.set_transition_graphs(
        transition,
        Some(counting_graph("counting_guard_true")),
        Some(counting_graph("counting_effect")),
    );

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    nodes.register(ContextSourceDeclaration).unwrap();
    nodes
        .register(CountingTriggerDeclaration::new(
            "counting_guard_true",
            Arc::clone(&guard_count),
            true,
        ))
        .unwrap();
    nodes
        .register(CountingTriggerDeclaration::new(
            "counting_effect",
            Arc::clone(&effect_count),
            true,
        ))
        .unwrap();
    let mut runtime = ChataigneStateMachineRuntime::compile(
        &machine,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    )
    .unwrap();
    runtime.initialize(&mut machine).unwrap();
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let provider = TestContextProvider::with_device_count(30);

    let output = runtime
        .tick_with_context_provider(
            &mut machine,
            &EvaluationCtx {
                logical_tick: 1,
                delta_time: Duration::ZERO,
                events: &[],
                inputs: &inputs,
                registries: &registries,
            },
            &provider,
        )
        .unwrap();

    assert_eq!(output.transition.as_ref().unwrap().transition, transition);
    assert_eq!(guard_count.load(Ordering::SeqCst), 1);
    assert_eq!(effect_count.load(Ordering::SeqCst), 1);
    assert_eq!(output.transition_outputs[&transition].debug_samples.len(), 1);
    assert!(output.processor_outputs[&processor_id].debug_samples.is_empty());
    assert_eq!(runtime.execution.active_scopes, machine.chart.active.active_scopes);
}

#[test]
fn processor_lane_command_intents_include_context_key() {
    let mut chart = Statechart::new();
    let state = chart.add_leaf(chart.root_region, "Only").unwrap();
    chart.set_initial(chart.root_region, state).unwrap();
    let mut machine = ChataigneStateMachine::new(chart);
    let formula = command_formula();
    let processor = Processor::from_formula("Command Processor", &formula);
    let processor_id = processor.id;
    machine.add_formula(formula);
    machine.add_processor(state, processor).unwrap();

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    nodes.register(CommandEmitterDeclaration).unwrap();
    let mut runtime = ChataigneStateMachineRuntime::compile(
        &machine,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    )
    .unwrap();
    runtime.initialize(&mut machine).unwrap();
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let provider = TestContextProvider::with_device_count(2);

    let output = runtime
        .tick_with_context_provider(
            &mut machine,
            &EvaluationCtx {
                logical_tick: 1,
                delta_time: Duration::ZERO,
                events: &[],
                inputs: &inputs,
                registries: &registries,
            },
            &provider,
        )
        .unwrap();

    let origins = output
        .command_intents
        .iter()
        .map(|intent| intent.origin.clone())
        .collect::<Vec<_>>();
    assert_eq!(output.intents.len(), 2);
    assert_eq!(origins.len(), 2);
    assert!(origins.contains(&crate::IntentOrigin::processor(
        processor_id,
        Some(ContextKey::single("device", "device-0")),
    )));
    assert!(origins.contains(&crate::IntentOrigin::processor(
        processor_id,
        Some(ContextKey::single("device", "device-1")),
    )));
}

#[test]
fn state_transition_updates_active_processor_matrix() {
    let mut chart = Statechart::new();
    let first = chart.add_leaf(chart.root_region, "First").unwrap();
    let second = chart.add_leaf(chart.root_region, "Second").unwrap();
    chart.set_initial(chart.root_region, first).unwrap();
    let transition = chart.add_transition(first, second, 0).unwrap();
    let mut machine = ChataigneStateMachine::new(chart);
    let formula = constant_formula();
    let first_processor = Processor::from_formula("First Processor", &formula);
    let first_id = first_processor.id;
    let second_processor = Processor::from_formula("Second Processor", &formula);
    let second_id = second_processor.id;
    machine.add_formula(formula);
    machine.add_processor(first, first_processor).unwrap();
    machine.add_processor(second, second_processor).unwrap();

    let mut guard = AlchemistGraph::new();
    let mut source = ANodeInstance::new(ANodeTypeId::new("constant"), "True");
    source.config.set("value", RuntimeValue::Bool(true));
    let source = guard.add_node(source).unwrap();
    let edge = guard
        .add_node(ANodeInstance::new(ANodeTypeId::new("trigger_on_off"), "Trigger"))
        .unwrap();
    guard
        .connect(
            OutputSocketRef::new(source, "value"),
            InputSocketRef::new(edge, "value"),
        )
        .unwrap();
    machine.set_transition_graphs(transition, Some(guard), None);

    let mut value_types = ValueTypeRegistry::with_primitives();
    register_value_types(&mut value_types).unwrap();
    let mut nodes = primitive_node_registry();
    register_nodes(&mut nodes).unwrap();
    let mut runtime = ChataigneStateMachineRuntime::compile(
        &machine,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    )
    .unwrap();
    runtime.initialize(&mut machine).unwrap();
    assert_eq!(runtime.execution.active_processors, vec![first_id]);
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };

    let output = runtime
        .tick(
            &mut machine,
            &EvaluationCtx {
                logical_tick: 1,
                delta_time: Duration::ZERO,
                events: &[],
                inputs: &inputs,
                registries: &registries,
            },
        )
        .unwrap();

    assert!(output.transition.is_some());
    assert_eq!(runtime.execution.active_processors, vec![second_id]);
    assert!(output.processor_outputs.contains_key(&second_id));
    assert!(!output.processor_outputs.contains_key(&first_id));
}
