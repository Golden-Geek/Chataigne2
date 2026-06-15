use std::sync::Arc;

use uuid::Uuid;

use golden_alchemist::{
    AlchemistFormula, AlchemistFormulaInstance, AxisSet, CompileCtx, CompiledAlchemistFormula, ContextAxisId,
    ContextKey, ContextValuePath, DebugCaptureSink, Diagnostic, DiagnosticOrigin, EvaluationCtx, EvaluationFrame,
    FormulaAnalysis, FormulaRef, FormulaSurface, LaneRuntimePool, RuntimeContextFrame, RuntimeOutput,
    RuntimePropertyFrame, RuntimeSubscription, RuntimeValue, compile_graph, evaluate_compiled_graph,
    evaluate_compiled_graph_stateless,
};
use golden_statechart::StateId;
use indexmap::IndexSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProcessorId(Uuid);

impl ProcessorId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ProcessorId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProcessorMemoryPolicy {
    #[default]
    ResetOnStateEnter,
    ResetOnProcessorEnable,
    PreserveWhileProjectOpen,
    PreserveAcrossStateReentry,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProcessorLifecyclePolicy {
    #[default]
    StateScoped,
    AlwaysActive,
    Manual,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProcessorCommandPolicy {
    #[default]
    Inherit,
    Suppress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessorLifecycleEvent {
    StateEnter(StateId),
    StateExit(StateId),
    ProcessorEnable,
    ProcessorDisable,
    ProjectStart,
    ProjectStop,
}

pub trait ProcessorContextProvider {
    fn available_axes(&self, processor_id: ProcessorId) -> AxisSet;

    fn iter_context_keys<'a>(
        &'a self,
        processor_id: ProcessorId,
        axes: &'a AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + 'a>;

    fn resolve_context_value(
        &self,
        key: &ContextKey,
        axis: &ContextAxisId,
        path: &ContextValuePath,
    ) -> Option<RuntimeValue>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessorBindingAnalysis {
    pub property_axes: AxisSet,
    pub input_axes: AxisSet,
    pub output_axes: AxisSet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessorExecutionStrategy {
    SingleStateless,
    MultiStateless,
    SingleStateful,
    MultiStatefulSparse,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessorExecutionPlan {
    pub processor_id: ProcessorId,
    pub available_axes: AxisSet,
    pub required_eval_axes: AxisSet,
    pub required_memory_axes: AxisSet,
    pub strategy: ProcessorExecutionStrategy,
}

impl ProcessorExecutionPlan {
    #[must_use]
    pub fn analyze(
        processor_id: ProcessorId,
        formula: &FormulaAnalysis,
        bindings: &ProcessorBindingAnalysis,
        available_axes: AxisSet,
    ) -> Self {
        let mut required_eval_axes = AxisSet::new();
        extend_axes(&mut required_eval_axes, &bindings.property_axes);
        extend_axes(&mut required_eval_axes, &formula.explicit_context_axes);
        extend_axes(&mut required_eval_axes, &bindings.input_axes);
        extend_axes(&mut required_eval_axes, &bindings.output_axes);
        extend_axes(&mut required_eval_axes, &formula.effect_axes);

        let mut required_memory_axes = AxisSet::new();
        if formula.has_stateful_nodes {
            extend_axes(&mut required_memory_axes, &formula.state_axes);
            extend_axes(&mut required_memory_axes, &bindings.property_axes);
            extend_axes(&mut required_memory_axes, &bindings.input_axes);
        }

        let strategy = match (formula.has_stateful_nodes, required_eval_axes.is_empty()) {
            (false, true) => ProcessorExecutionStrategy::SingleStateless,
            (false, false) => ProcessorExecutionStrategy::MultiStateless,
            (true, true) => ProcessorExecutionStrategy::SingleStateful,
            (true, false) => ProcessorExecutionStrategy::MultiStatefulSparse,
        };

        Self {
            processor_id,
            available_axes,
            required_eval_axes,
            required_memory_axes,
            strategy,
        }
    }
}

fn extend_axes(target: &mut AxisSet, source: &AxisSet) {
    target.extend(source.iter().cloned());
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultProcessorContextProvider;

impl ProcessorContextProvider for DefaultProcessorContextProvider {
    fn available_axes(&self, _processor_id: ProcessorId) -> AxisSet {
        AxisSet::new()
    }

    fn iter_context_keys<'a>(
        &'a self,
        _processor_id: ProcessorId,
        axes: &'a AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + 'a> {
        if axes.is_empty() {
            Box::new(std::iter::once(ContextKey::default_lane()))
        } else {
            Box::new(std::iter::empty())
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

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Processor {
    pub id: ProcessorId,
    pub label: String,
    pub formula_instance: AlchemistFormulaInstance,
    pub enabled: bool,
    pub lifecycle: ProcessorLifecyclePolicy,
    pub memory_policy: ProcessorMemoryPolicy,
    pub command_policy: ProcessorCommandPolicy,
}

impl Processor {
    #[must_use]
    pub fn new(label: impl Into<String>, formula_instance: AlchemistFormulaInstance) -> Self {
        Self {
            id: ProcessorId::new(),
            label: label.into(),
            formula_instance,
            enabled: true,
            lifecycle: ProcessorLifecyclePolicy::default(),
            memory_policy: ProcessorMemoryPolicy::default(),
            command_policy: ProcessorCommandPolicy::default(),
        }
    }

    #[must_use]
    pub fn from_formula(label: impl Into<String>, formula: &AlchemistFormula) -> Self {
        Self::new(label, formula.instantiate())
    }

    #[must_use]
    pub fn ui_model(&self, formula: &AlchemistFormula, diagnostics: Vec<Diagnostic>) -> ProcessorUiModel {
        ProcessorUiModel {
            id: self.id,
            label: self.label.clone(),
            formula_id: formula.id.to_string(),
            formula_label: formula.label.clone(),
            surface: formula.surface.clone(),
            diagnostics,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessorDirtyFlags {
    pub graph: bool,
    pub registry: bool,
    pub exposed: bool,
}

impl ProcessorDirtyFlags {
    #[must_use]
    pub const fn any(self) -> bool {
        self.graph || self.registry || self.exposed
    }
}

pub struct ProcessorRuntime {
    pub id: ProcessorId,
    pub compiled: Option<Arc<CompiledAlchemistFormula>>,
    pub plan: Option<ProcessorExecutionPlan>,
    pub lanes: LaneRuntimePool,
    pub properties: Option<RuntimePropertyFrame>,
    pub active: bool,
    pub dirty: ProcessorDirtyFlags,
    pub subscriptions: Vec<RuntimeSubscription>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ProcessorRuntime {
    #[must_use]
    pub fn new(id: ProcessorId) -> Self {
        Self {
            id,
            compiled: None,
            plan: None,
            lanes: LaneRuntimePool::default(),
            properties: None,
            active: false,
            dirty: ProcessorDirtyFlags {
                graph: true,
                ..ProcessorDirtyFlags::default()
            },
            subscriptions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn compile(&mut self, processor: &Processor, formula: &AlchemistFormula, ctx: &CompileCtx<'_>) -> bool {
        if let Err(error) = processor.formula_instance.require_compatible(formula) {
            self.clear_runtime();
            self.diagnostics = vec![Diagnostic::error(
                "formula_instance_incompatible",
                error.to_string(),
                DiagnosticOrigin::Graph,
            )];
            return false;
        }
        let compile_ctx = CompileCtx {
            value_types: ctx.value_types,
            nodes: ctx.nodes,
            properties: Some(&formula.properties),
        };
        let result = compile_graph(&formula.graph, &compile_ctx);
        self.diagnostics = result.diagnostics;
        let Some(compiled) = result.compiled else {
            self.clear_runtime();
            return false;
        };
        let compiled_formula = Arc::new(CompiledAlchemistFormula::new(
            FormulaRef {
                id: formula.id.clone(),
                version: formula.version,
            },
            compiled,
            self.diagnostics.clone(),
        ));
        self.compile_from_shared_formula(processor, formula, compiled_formula)
    }

    pub fn compile_from_shared_formula(
        &mut self,
        processor: &Processor,
        formula: &AlchemistFormula,
        compiled: Arc<CompiledAlchemistFormula>,
    ) -> bool {
        if let Err(error) = processor.formula_instance.require_compatible(formula) {
            self.clear_runtime();
            self.diagnostics = vec![Diagnostic::error(
                "formula_instance_incompatible",
                error.to_string(),
                DiagnosticOrigin::Graph,
            )];
            return false;
        }
        self.subscriptions = compiled.graph.subscriptions.clone();
        self.lanes = LaneRuntimePool::for_graph(&compiled.graph);
        self.properties = Some(RuntimePropertyFrame::from_defaults(&compiled.properties));
        self.diagnostics = compiled.diagnostics.clone();
        self.plan = Some(ProcessorExecutionPlan::analyze(
            processor.id,
            &compiled.analysis,
            &ProcessorBindingAnalysis::default(),
            AxisSet::new(),
        ));
        self.compiled = Some(compiled);
        self.dirty = ProcessorDirtyFlags::default();
        true
    }

    fn clear_runtime(&mut self) {
        self.compiled = None;
        self.plan = None;
        self.lanes = LaneRuntimePool::default();
        self.properties = None;
        self.subscriptions.clear();
    }

    pub fn rebuild_execution_plan(
        &mut self,
        context_provider: &dyn ProcessorContextProvider,
        bindings: &ProcessorBindingAnalysis,
    ) {
        if let Some(compiled) = &self.compiled {
            self.plan = Some(ProcessorExecutionPlan::analyze(
                self.id,
                &compiled.analysis,
                bindings,
                context_provider.available_axes(self.id),
            ));
        }
    }

    pub fn apply_lifecycle(&mut self, processor: &Processor, event: ProcessorLifecycleEvent) {
        if !processor.enabled {
            self.active = false;
            return;
        }
        self.active = match (processor.lifecycle, event) {
            (ProcessorLifecyclePolicy::AlwaysActive, ProcessorLifecycleEvent::ProjectStart) => true,
            (ProcessorLifecyclePolicy::AlwaysActive, ProcessorLifecycleEvent::ProjectStop) => false,
            (ProcessorLifecyclePolicy::StateScoped, ProcessorLifecycleEvent::StateEnter(_))
            | (ProcessorLifecyclePolicy::Manual, ProcessorLifecycleEvent::ProcessorEnable) => true,
            (ProcessorLifecyclePolicy::StateScoped, ProcessorLifecycleEvent::StateExit(_))
            | (ProcessorLifecyclePolicy::Manual, ProcessorLifecycleEvent::ProcessorDisable) => false,
            _ => self.active,
        };
        let reset = matches!(
            (processor.memory_policy, event),
            (
                ProcessorMemoryPolicy::ResetOnStateEnter,
                ProcessorLifecycleEvent::StateEnter(_)
            ) | (
                ProcessorMemoryPolicy::ResetOnProcessorEnable,
                ProcessorLifecycleEvent::ProcessorEnable
            )
        );
        if reset {
            self.lanes.clear();
        }
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        let provider = DefaultProcessorContextProvider;
        self.evaluate_with_context_provider(ctx, &provider)
            .into_iter()
            .next()
            .map(|lane| lane.output)
            .unwrap_or_default()
    }

    pub fn evaluate_with_context_provider(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
    ) -> Vec<ProcessorLaneOutput> {
        if !self.active {
            return Vec::new();
        }
        let Some(compiled) = self.compiled.as_ref().map(Arc::clone) else {
            return Vec::new();
        };
        let Some(properties) = self.properties.clone() else {
            return Vec::new();
        };
        let plan = self.plan.clone().unwrap_or_else(|| {
            ProcessorExecutionPlan::analyze(
                self.id,
                &compiled.analysis,
                &ProcessorBindingAnalysis::default(),
                context_provider.available_axes(self.id),
            )
        });
        let mut context_keys = context_provider
            .iter_context_keys(self.id, &plan.required_eval_axes)
            .collect::<IndexSet<_>>();
        if context_keys.is_empty() && plan.required_eval_axes.is_empty() {
            context_keys.insert(ContextKey::default_lane());
        }
        let memory_keys = context_keys
            .iter()
            .map(|context_key| context_key.project(&plan.required_memory_axes))
            .collect::<IndexSet<_>>();
        self.lanes.retain_keys(&memory_keys);

        context_keys
            .into_iter()
            .map(|context_key| {
                let mut debug = DebugCaptureSink::default();
                let context = RuntimeContextFrame::new(context_key.clone());
                let memory_key = context_key.project(&plan.required_memory_axes);
                let output = match self.lanes.memory_for_key(memory_key, &compiled.graph) {
                    Some(memory) => evaluate_compiled_graph(
                        &compiled.graph,
                        memory,
                        EvaluationFrame {
                            ctx,
                            properties: &properties,
                            context: &context,
                            debug: &mut debug,
                        },
                    ),
                    None => evaluate_compiled_graph_stateless(
                        &compiled.graph,
                        EvaluationFrame {
                            ctx,
                            properties: &properties,
                            context: &context,
                            debug: &mut debug,
                        },
                    ),
                };
                ProcessorLaneOutput {
                    context_key: (!context_key.is_default_lane()).then_some(context_key),
                    output,
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct ProcessorLaneOutput {
    pub context_key: Option<ContextKey>,
    pub output: RuntimeOutput,
}

#[derive(Clone, Debug)]
pub struct ProcessorUiModel {
    pub id: ProcessorId,
    pub label: String,
    pub formula_id: String,
    pub formula_label: String,
    pub surface: FormulaSurface,
    pub diagnostics: Vec<Diagnostic>,
}
