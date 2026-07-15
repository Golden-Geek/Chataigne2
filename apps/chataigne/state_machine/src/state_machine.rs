use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};

use chataigne_alchemist::{
    AlchemistFormula, AlchemistGraphDocument, AlchemistRuntime, CompileCtx, CompiledAlchemistFormula, EvaluationCtx,
    FormulaCompileKey, FormulaId, FormulaRef, RuntimeEvent, RuntimeInputSnapshot, RuntimeIntent, RuntimeOutput,
    compile_graph,
};
use golden_statechart::{LifecycleEvent, StateId, Statechart, TransitionId, TransitionOutcome};

use crate::{
    CommandIntent, CommandPolicy, DefaultProcessorContextProvider, IntentOrigin, Processor, ProcessorCommandPolicy,
    ProcessorContextProvider, ProcessorGroup, ProcessorGroupId, ProcessorId, ProcessorLaneOutput,
    ProcessorLifecycleEvent, ProcessorManager, ProcessorManagerError, ProcessorManagerId, ProcessorRuntime,
};

#[derive(Clone, Debug)]
pub struct ChataigneTransition {
    pub transition_id: TransitionId,
    pub guard_graph: Option<AlchemistGraphDocument>,
    pub effect_graph: Option<AlchemistGraphDocument>,
}

#[derive(Clone, Debug)]
pub struct ChataigneStateMachine {
    pub chart: Statechart,
    pub formulas: IndexMap<FormulaId, AlchemistFormula>,
    pub processor_managers: IndexMap<StateId, ProcessorManager>,
    pub transitions: IndexMap<TransitionId, ChataigneTransition>,
}

impl ChataigneStateMachine {
    #[must_use]
    pub fn new(chart: Statechart) -> Self {
        Self {
            chart,
            formulas: IndexMap::new(),
            processor_managers: IndexMap::new(),
            transitions: IndexMap::new(),
        }
    }

    pub fn add_formula(&mut self, formula: AlchemistFormula) -> Option<AlchemistFormula> {
        self.formulas.insert(formula.id.clone(), formula)
    }

    pub fn processor_manager_mut(&mut self, state: StateId) -> &mut ProcessorManager {
        self.processor_managers.entry(state).or_default()
    }

    pub fn add_processor(
        &mut self,
        state: StateId,
        processor: Processor,
    ) -> Result<ProcessorId, ProcessorManagerError> {
        self.processor_manager_mut(state).add_processor(processor)
    }

    pub fn add_processor_group(
        &mut self,
        state: StateId,
        group: ProcessorGroup,
    ) -> Result<ProcessorGroupId, ProcessorManagerError> {
        self.processor_manager_mut(state).add_group(group)
    }

    #[must_use]
    pub fn processor(&self, id: ProcessorId) -> Option<&Processor> {
        self.processor_managers
            .values()
            .find_map(|manager| manager.processor(id))
    }

    pub fn processors(&self) -> impl Iterator<Item = &Processor> {
        self.processor_managers.values().flat_map(ProcessorManager::processors)
    }

    pub fn set_transition_graphs(
        &mut self,
        transition_id: TransitionId,
        guard_graph: Option<AlchemistGraphDocument>,
        effect_graph: Option<AlchemistGraphDocument>,
    ) {
        self.transitions.insert(
            transition_id,
            ChataigneTransition {
                transition_id,
                guard_graph,
                effect_graph,
            },
        );
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeExecutionMatrix {
    pub active_scopes: IndexSet<StateId>,
    pub active_managers: Vec<ProcessorManagerId>,
    pub active_processors: Vec<ProcessorId>,
    pub processors_by_manager: IndexMap<ProcessorManagerId, Vec<ProcessorId>>,
    pub dirty_processors: IndexSet<ProcessorId>,
}

#[derive(Clone, Debug, Default)]
pub struct StateMachineTickOutput {
    pub transition: Option<TransitionOutcome>,
    pub intents: Vec<RuntimeIntent>,
    pub command_intents: Vec<CommandIntent>,
    pub transition_outputs: IndexMap<TransitionId, RuntimeOutput>,
    pub processor_outputs: IndexMap<ProcessorId, RuntimeOutput>,
}

pub struct GlobalCompiledGraphRuntime {
    runtime: AlchemistRuntime,
}

impl std::fmt::Debug for GlobalCompiledGraphRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GlobalCompiledGraphRuntime")
            .finish_non_exhaustive()
    }
}

impl GlobalCompiledGraphRuntime {
    fn new(runtime: AlchemistRuntime) -> Self {
        Self { runtime }
    }

    fn evaluate(&mut self, ctx: &EvaluationCtx<'_>, global: &GlobalStateMachineContextFrame<'_>) -> RuntimeOutput {
        debug_assert_eq!(global.logical_tick, ctx.logical_tick);
        debug_assert!(std::ptr::eq(global.inputs, ctx.inputs));
        debug_assert!(std::ptr::eq(global.events, ctx.events));
        self.runtime.evaluate(ctx)
    }

    fn evaluate_with_debug_capture(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        global: &GlobalStateMachineContextFrame<'_>,
    ) -> RuntimeOutput {
        debug_assert_eq!(global.logical_tick, ctx.logical_tick);
        debug_assert!(std::ptr::eq(global.inputs, ctx.inputs));
        debug_assert!(std::ptr::eq(global.events, ctx.events));
        self.runtime
            .evaluate_with_capture_mode(ctx, chataigne_alchemist::DebugCaptureMode::All { history_len: 1 })
    }

    fn fired_trigger_in_last_evaluation(&self) -> bool {
        self.runtime.memory.last_executed_nodes().iter().any(|exec_id| {
            self.runtime.compiled.exec_nodes[exec_id.index()]
                .outputs
                .iter()
                .any(|slot| {
                    matches!(
                        self.runtime.memory.value(*slot),
                        Some(golden_values::Value::Trigger(trigger)) if trigger.fired
                    )
                })
        })
    }
}

#[derive(Debug)]
pub struct StateMachineTransitionRuntime {
    pub transition_id: TransitionId,
    pub guard: Option<GlobalCompiledGraphRuntime>,
    pub effect: Option<GlobalCompiledGraphRuntime>,
}

impl StateMachineTransitionRuntime {
    fn guarded_transition(&self) -> Option<TransitionId> {
        self.guard.as_ref().map(|_| self.transition_id)
    }

    fn evaluate_guard(&mut self, ctx: &EvaluationCtx<'_>, global: &GlobalStateMachineContextFrame<'_>) -> bool {
        let Some(guard) = self.guard.as_mut() else {
            return false;
        };
        guard.evaluate(ctx, global);
        guard.fired_trigger_in_last_evaluation()
    }

    fn evaluate_effect(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        global: &GlobalStateMachineContextFrame<'_>,
    ) -> Option<RuntimeOutput> {
        self.effect
            .as_mut()
            .map(|effect| effect.evaluate_with_debug_capture(ctx, global))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalStateMachineContextFrame<'a> {
    pub logical_tick: u64,
    pub active_scopes: &'a IndexSet<StateId>,
    pub inputs: &'a RuntimeInputSnapshot,
    pub events: &'a [RuntimeEvent],
}

impl<'a> GlobalStateMachineContextFrame<'a> {
    fn from_tick(ctx: &EvaluationCtx<'a>, active_scopes: &'a IndexSet<StateId>) -> Self {
        Self {
            logical_tick: ctx.logical_tick,
            active_scopes,
            inputs: ctx.inputs,
            events: ctx.events,
        }
    }
}

pub struct ChataigneStateMachineRuntime {
    pub processor_runtimes: IndexMap<ProcessorId, ProcessorRuntime>,
    transition_runtimes: IndexMap<TransitionId, StateMachineTransitionRuntime>,
    pub execution: RuntimeExecutionMatrix,
}

impl ChataigneStateMachineRuntime {
    pub fn compile(machine: &ChataigneStateMachine, ctx: &CompileCtx<'_>) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let mut compiled_formulas = IndexMap::<FormulaCompileKey, Arc<CompiledAlchemistFormula>>::new();
        let mut processor_runtimes = IndexMap::new();
        for processor in machine.processors() {
            let Some(formula) = machine.formulas.get(&processor.formula_instance.formula_ref.id) else {
                errors.push(format!(
                    "processor `{}` references missing formula `{}`",
                    processor.label, processor.formula_instance.formula_ref.id
                ));
                continue;
            };
            let key = FormulaCompileKey::from_formula(formula, 0, 0);
            let compiled_formula = if let Some(compiled) = compiled_formulas.get(&key) {
                Arc::clone(compiled)
            } else {
                let formula_ctx = CompileCtx {
                    value_types: ctx.value_types,
                    nodes: ctx.nodes,
                    properties: Some(&formula.properties),
                };
                let result = compile_graph(&formula.graph, &formula_ctx);
                let diagnostics = result.diagnostics;
                let Some(compiled) = result.compiled else {
                    errors.extend(diagnostics.into_iter().map(|diagnostic| diagnostic.message));
                    continue;
                };
                let compiled = Arc::new(CompiledAlchemistFormula::new(
                    FormulaRef {
                        id: formula.id.clone(),
                        version: formula.version,
                    },
                    compiled,
                    diagnostics,
                ));
                compiled_formulas.insert(key, Arc::clone(&compiled));
                compiled
            };
            let mut runtime = ProcessorRuntime::new(processor.id);
            if !runtime.compile_from_shared_formula_with_compile_ctx(processor, formula, compiled_formula, ctx) {
                errors.extend(runtime.diagnostics.iter().map(|diagnostic| diagnostic.message.clone()));
            }
            processor_runtimes.insert(processor.id, runtime);
        }
        let mut transition_runtimes = IndexMap::new();
        for transition in machine.transitions.values() {
            let guard = transition
                .guard_graph
                .as_ref()
                .and_then(|graph| compile_global_graph_runtime(graph, ctx, &mut errors));
            let effect = transition
                .effect_graph
                .as_ref()
                .and_then(|graph| compile_global_graph_runtime(graph, ctx, &mut errors));
            if guard.is_some() || effect.is_some() {
                transition_runtimes.insert(
                    transition.transition_id,
                    StateMachineTransitionRuntime {
                        transition_id: transition.transition_id,
                        guard,
                        effect,
                    },
                );
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Self {
            processor_runtimes,
            transition_runtimes,
            execution: RuntimeExecutionMatrix::default(),
        })
    }

    pub fn initialize(
        &mut self,
        machine: &mut ChataigneStateMachine,
    ) -> Result<(), golden_statechart::StatechartError> {
        let lifecycle = machine.chart.initialize()?;
        self.apply_lifecycle(machine, &lifecycle);
        self.rebuild_execution_matrix(machine);
        Ok(())
    }

    pub fn tick(
        &mut self,
        machine: &mut ChataigneStateMachine,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<StateMachineTickOutput, golden_statechart::StatechartError> {
        let provider = DefaultProcessorContextProvider;
        self.tick_with_context_provider(machine, ctx, &provider)
    }

    pub fn tick_with_context_provider(
        &mut self,
        machine: &mut ChataigneStateMachine,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
    ) -> Result<StateMachineTickOutput, golden_statechart::StatechartError> {
        let mut fired_guards = IndexSet::new();
        let guard_context = GlobalStateMachineContextFrame::from_tick(ctx, &machine.chart.active().active_scopes);
        for runtime in self.transition_runtimes.values_mut() {
            if runtime.evaluate_guard(ctx, &guard_context) {
                fired_guards.insert(runtime.transition_id);
            }
        }
        let guarded: IndexSet<TransitionId> = self
            .transition_runtimes
            .values()
            .filter_map(StateMachineTransitionRuntime::guarded_transition)
            .collect();
        let transition = machine
            .chart
            .step(|candidate| !guarded.contains(&candidate.id) || fired_guards.contains(&candidate.id))?;
        let mut transition_output = None;
        if let Some(outcome) = &transition {
            self.apply_lifecycle(machine, &outcome.lifecycle);
            self.rebuild_execution_matrix(machine);
            let effect_context = GlobalStateMachineContextFrame::from_tick(ctx, &machine.chart.active().active_scopes);
            transition_output = self
                .transition_runtimes
                .get_mut(&outcome.transition)
                .and_then(|runtime| runtime.evaluate_effect(ctx, &effect_context))
                .map(|output| (outcome.transition, output));
        }

        let mut result = StateMachineTickOutput {
            transition,
            ..StateMachineTickOutput::default()
        };
        if let Some((transition_id, output)) = transition_output {
            result.intents.extend(output.intents.iter().cloned());
            extend_command_intents(
                &mut result.command_intents,
                &output.intents,
                IntentOrigin::Transition { transition_id },
                CommandPolicy::LastWriterWins,
            );
            result.transition_outputs.insert(transition_id, output);
        }
        for processor_id in self.execution.active_processors.clone() {
            let Some(processor) = machine.processor(processor_id) else {
                continue;
            };
            let lanes = self.processor_runtimes[&processor_id].evaluate_processor_with_context_provider(
                processor,
                ctx,
                context_provider,
            );
            if let Some(policy) = processor_command_policy(processor) {
                for lane in &lanes {
                    extend_command_intents(
                        &mut result.command_intents,
                        &lane.output.intents,
                        IntentOrigin::processor(processor_id, lane.context_key.clone()),
                        policy.clone(),
                    );
                }
            }
            let output = merge_processor_lane_outputs(lanes);
            result.intents.extend(output.intents.iter().cloned());
            result.processor_outputs.insert(processor_id, output);
        }
        Ok(result)
    }

    fn apply_lifecycle(&mut self, machine: &ChataigneStateMachine, events: &[LifecycleEvent]) {
        for event in events {
            let (state, lifecycle) = match event {
                LifecycleEvent::Enter(state) => (*state, ProcessorLifecycleEvent::StateEnter(*state)),
                LifecycleEvent::Exit(state) => (*state, ProcessorLifecycleEvent::StateExit(*state)),
            };
            if let Some(manager) = machine.processor_managers.get(&state) {
                for processor in manager.processors() {
                    let Some(runtime) = self.processor_runtimes.get_mut(&processor.id) else {
                        continue;
                    };
                    runtime.apply_lifecycle(processor, lifecycle);
                }
            }
        }
    }

    fn rebuild_execution_matrix(&mut self, machine: &ChataigneStateMachine) {
        self.execution.active_scopes = machine.chart.active().active_scopes.clone();
        self.execution.active_managers.clear();
        self.execution.active_processors.clear();
        self.execution.processors_by_manager.clear();
        for state in &self.execution.active_scopes {
            if let Some(manager) = machine.processor_managers.get(state) {
                let processors = manager.active_processor_ids();
                self.execution.active_managers.push(manager.id);
                self.execution
                    .processors_by_manager
                    .insert(manager.id, processors.clone());
                self.execution.active_processors.extend(processors);
            }
        }
    }
}

fn compile_global_graph_runtime(
    graph: &AlchemistGraphDocument,
    ctx: &CompileCtx<'_>,
    errors: &mut Vec<String>,
) -> Option<GlobalCompiledGraphRuntime> {
    let global_ctx = CompileCtx {
        value_types: ctx.value_types,
        nodes: ctx.nodes,
        properties: None,
    };
    let result = compile_graph(graph, &global_ctx);
    if let Some(compiled) = result.compiled {
        Some(GlobalCompiledGraphRuntime::new(AlchemistRuntime::new(compiled)))
    } else {
        errors.extend(result.diagnostics.into_iter().map(|diagnostic| diagnostic.message));
        None
    }
}

fn merge_processor_lane_outputs(lanes: Vec<ProcessorLaneOutput>) -> RuntimeOutput {
    let mut output = RuntimeOutput::default();
    for lane in lanes {
        output.intents.extend(lane.output.intents);
        output.diagnostics.extend(lane.output.diagnostics);
        output.debug_samples.extend(lane.output.debug_samples);
    }
    output
}

fn extend_command_intents(
    target: &mut Vec<CommandIntent>,
    intents: &[RuntimeIntent],
    origin: IntentOrigin,
    policy: CommandPolicy,
) {
    target.extend(
        intents
            .iter()
            .cloned()
            .filter_map(|intent| CommandIntent::from_runtime_with_policy(intent, origin.clone(), 0, policy.clone())),
    );
}

fn processor_command_policy(processor: &Processor) -> Option<CommandPolicy> {
    match processor.command_policy {
        ProcessorCommandPolicy::Inherit => Some(CommandPolicy::LastWriterWins),
        ProcessorCommandPolicy::Suppress => None,
    }
}
