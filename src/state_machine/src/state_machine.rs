use indexmap::{IndexMap, IndexSet};

use golden_alchemist::{
    AlchemistFormula, AlchemistGraph, AlchemistRuntime, CompileCtx, EvaluationCtx, FormulaId, RuntimeIntent,
    RuntimeOutput, compile_graph,
};
use golden_statechart::{LifecycleEvent, StateId, Statechart, TransitionId, TransitionOutcome};

use crate::{
    Processor, ProcessorGroup, ProcessorGroupId, ProcessorId, ProcessorLifecycleEvent, ProcessorManager,
    ProcessorManagerError, ProcessorManagerId, ProcessorRuntime,
};

#[derive(Clone, Debug)]
pub struct ChataigneTransition {
    pub transition_id: TransitionId,
    pub guard_graph: Option<AlchemistGraph>,
    pub effect_graph: Option<AlchemistGraph>,
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
        guard_graph: Option<AlchemistGraph>,
        effect_graph: Option<AlchemistGraph>,
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
    pub processor_outputs: IndexMap<ProcessorId, RuntimeOutput>,
}

pub struct ChataigneStateMachineRuntime {
    pub processor_runtimes: IndexMap<ProcessorId, ProcessorRuntime>,
    guard_runtimes: IndexMap<TransitionId, AlchemistRuntime>,
    pub execution: RuntimeExecutionMatrix,
}

impl ChataigneStateMachineRuntime {
    pub fn compile(machine: &ChataigneStateMachine, ctx: &CompileCtx<'_>) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let mut processor_runtimes = IndexMap::new();
        for processor in machine.processors() {
            let Some(formula) = machine.formulas.get(&processor.formula_instance.formula_ref.id) else {
                errors.push(format!(
                    "processor `{}` references missing formula `{}`",
                    processor.label, processor.formula_instance.formula_ref.id
                ));
                continue;
            };
            let mut runtime = ProcessorRuntime::new(processor.id);
            if !runtime.compile(processor, formula, ctx) {
                errors.extend(runtime.diagnostics.iter().map(|diagnostic| diagnostic.message.clone()));
            }
            processor_runtimes.insert(processor.id, runtime);
        }
        let mut guard_runtimes = IndexMap::new();
        for transition in machine.transitions.values() {
            if let Some(graph) = &transition.guard_graph {
                let result = compile_graph(graph, ctx);
                if let Some(compiled) = result.compiled {
                    guard_runtimes.insert(transition.transition_id, AlchemistRuntime::new(compiled));
                } else {
                    errors.extend(result.diagnostics.into_iter().map(|diagnostic| diagnostic.message));
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Self {
            processor_runtimes,
            guard_runtimes,
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
        let mut fired_guards = IndexSet::new();
        for (transition, runtime) in &mut self.guard_runtimes {
            let output = runtime.evaluate(ctx);
            if output.debug_samples.iter().any(|sample| {
                matches!(
                    sample.value,
                    golden_alchemist::RuntimeValue::Trigger(trigger) if trigger.fired
                )
            }) {
                fired_guards.insert(*transition);
            }
        }
        let guarded: IndexSet<TransitionId> = machine
            .transitions
            .values()
            .filter(|transition| transition.guard_graph.is_some())
            .map(|transition| transition.transition_id)
            .collect();
        let transition = machine
            .chart
            .step(|candidate| !guarded.contains(&candidate.id) || fired_guards.contains(&candidate.id))?;
        if let Some(outcome) = &transition {
            self.apply_lifecycle(machine, &outcome.lifecycle);
            self.rebuild_execution_matrix(machine);
        }

        let mut result = StateMachineTickOutput {
            transition,
            ..StateMachineTickOutput::default()
        };
        for processor_id in self.execution.active_processors.clone() {
            let output = self.processor_runtimes[&processor_id].evaluate(ctx);
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
        self.execution.active_scopes = machine.chart.active.active_scopes.clone();
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
