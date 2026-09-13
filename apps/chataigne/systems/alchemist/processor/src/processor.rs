use std::{fmt, sync::Arc};

use uuid::Uuid;

use chataigne_alchemist::{
    AlchemistFormula, AlchemistFormulaInstance, AlchemistMemory, AxisSet, CompileCtx, CompiledAlchemistFormula,
    ContextAxisId, ContextKey, ContextValuePath, DebugCaptureMode, DebugCaptureSink, Diagnostic, DiagnosticOrigin,
    EvaluationCtx, EvaluationFrame, ExecNodeId, FormulaCompileKey, FormulaPropertyId, FormulaRef, LaneRuntimePool,
    RuntimeContextFrame, RuntimeDiagnostic, RuntimeInputSnapshot, RuntimeOutput, RuntimePropertyFrame,
    RuntimePropertyFrameError, RuntimeSubscription, SurfaceItemId, compile_graph, evaluate_compiled_graph,
    evaluate_compiled_graph_fresh_reusing,
};
use chataigne_condition::{
    CompiledConditionProgram, ConditionDefinition, ConditionEvaluationFrame, ConditionInputProvider, ConditionRuntime,
    compile_condition,
};
use chataigne_state_machine_model::StateId;
use golden_values::{StableRef, Value as RuntimeValue};
use indexmap::{IndexMap, IndexSet};

use crate::{ManagedFormulaRuntime, kernel_profile::profile_kernel};

mod plan;
mod presentation;

pub use plan::{
    DefaultProcessorContextProvider, ProcessorBindingAnalysis, ProcessorExecutionPlan, ProcessorExecutionStrategy,
};
pub use presentation::{
    ANodeOutputPreviewSample, ProcessorDebugCapture, ProcessorFormulaSourceKind, ProcessorFormulaUiState,
    ProcessorUiModel, processor_output_preview_samples, processor_output_preview_samples_from_lanes,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProcessorId(Uuid);

impl ProcessorId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for ProcessorId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProcessorId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
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

    /// Iterates each available lane exactly once in stable axis order.
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

    fn resolve_condition_node_value(
        &self,
        _provider: &str,
        _node: &StableRef,
        _key: &ContextKey,
    ) -> Option<RuntimeValue> {
        None
    }

    fn evaluate_script_condition(&self, _script: &str, _key: &ContextKey) -> Result<bool, String> {
        Err("script condition provider is unavailable".to_owned())
    }
}

struct LaneConditionInputs<'a> {
    snapshot: &'a RuntimeInputSnapshot,
    context_provider: &'a dyn ProcessorContextProvider,
    context_key: &'a ContextKey,
}

impl ConditionInputProvider for LaneConditionInputs<'_> {
    fn input_value(&self, input: &StableRef) -> Option<RuntimeValue> {
        self.snapshot
            .get_context(input, self.context_key)
            .or_else(|| self.snapshot.get(input))
            .cloned()
    }

    fn input_node_value(&self, provider: &str, node: &StableRef) -> Option<RuntimeValue> {
        self.context_provider
            .resolve_condition_node_value(provider, node, self.context_key)
    }

    fn script_condition(&self, script: &str) -> Result<bool, String> {
        self.context_provider
            .evaluate_script_condition(script, self.context_key)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Processor {
    pub id: ProcessorId,
    pub label: String,
    pub formula_instance: AlchemistFormulaInstance,
    pub context_property_bindings: IndexMap<SurfaceItemId, ProcessorContextPropertyBinding>,
    pub condition: Option<ConditionDefinition>,
    pub enabled: bool,
    pub lifecycle: ProcessorLifecyclePolicy,
    pub memory_policy: ProcessorMemoryPolicy,
    pub command_policy: ProcessorCommandPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProcessorContextPropertyBinding {
    pub axis: ContextAxisId,
    pub path: ContextValuePath,
}

impl Processor {
    #[must_use]
    pub fn new(label: impl Into<String>, formula_instance: AlchemistFormulaInstance) -> Self {
        Self {
            id: ProcessorId::new(),
            label: label.into(),
            formula_instance,
            context_property_bindings: IndexMap::new(),
            condition: None,
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
        self.ui_model_with_formula_source(formula, diagnostics, ProcessorFormulaUiState::default(), None)
    }

    #[must_use]
    pub fn ui_model_with_formula_source(
        &self,
        formula: &AlchemistFormula,
        diagnostics: Vec<Diagnostic>,
        formula_source: ProcessorFormulaUiState,
        formula_source_key: Option<String>,
    ) -> ProcessorUiModel {
        ProcessorUiModel {
            id: self.id,
            label: self.label.clone(),
            active: self.enabled,
            formula_id: formula.id.to_string(),
            formula_label: formula.label.clone(),
            formula_source_key,
            surface: formula.surface.clone(),
            managed_region_instances: self.formula_instance.managed_regions.clone(),
            diagnostics,
            formula_source,
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
    compiled_formula_key: Option<FormulaCompileKey>,
    pub managed_formula: Option<ManagedFormulaRuntime>,
    pub plan: Option<ProcessorExecutionPlan>,
    pub compiled_condition: Option<Arc<CompiledConditionProgram>>,
    pub condition_runtimes: IndexMap<ContextKey, ConditionRuntime>,
    pub lanes: LaneRuntimePool,
    managed_context_keys: IndexSet<ContextKey>,
    managed_context_revision: u64,
    stateless_scratch: Option<AlchemistMemory>,
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
            compiled_formula_key: None,
            managed_formula: None,
            plan: None,
            compiled_condition: None,
            condition_runtimes: IndexMap::new(),
            lanes: LaneRuntimePool::default(),
            managed_context_keys: IndexSet::new(),
            managed_context_revision: 0,
            stateless_scratch: None,
            active: false,
            dirty: ProcessorDirtyFlags {
                graph: true,
                ..ProcessorDirtyFlags::default()
            },
            subscriptions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[must_use]
    pub fn needs_continuous_evaluation(&self) -> bool {
        self.compiled
            .as_ref()
            .is_some_and(|compiled| compiled.analysis.has_always_process_nodes)
            || self
                .managed_formula
                .as_ref()
                .is_some_and(ManagedFormulaRuntime::needs_continuous_evaluation)
    }

    #[must_use]
    pub fn managed_context_revision(&self) -> u64 {
        self.managed_context_revision
    }

    #[must_use]
    pub fn has_managed_context_key(&self, key: &ContextKey) -> bool {
        self.managed_context_keys.contains(key)
    }

    #[cfg(test)]
    pub(crate) fn stateless_scratch_address(&self) -> Option<usize> {
        self.stateless_scratch
            .as_ref()
            .map(|memory| std::ptr::from_ref(memory).addr())
    }

    pub fn compile(&mut self, processor: &Processor, formula: &AlchemistFormula, ctx: &CompileCtx<'_>) -> bool {
        self.compile_with_lane_policy(processor, formula, ctx, false)
    }

    pub fn compile_preserving_compatible_lanes(
        &mut self,
        processor: &Processor,
        formula: &AlchemistFormula,
        ctx: &CompileCtx<'_>,
    ) -> bool {
        self.compile_with_lane_policy(processor, formula, ctx, true)
    }

    fn compile_with_lane_policy(
        &mut self,
        processor: &Processor,
        formula: &AlchemistFormula,
        ctx: &CompileCtx<'_>,
        preserve_compatible_lanes: bool,
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
        let managed_formula = match ManagedFormulaRuntime::compile_with_shared_graph(
            formula,
            &processor.formula_instance,
            ctx,
            Arc::clone(&compiled_formula.graph),
        ) {
            Ok(managed_formula) => managed_formula,
            Err(error) => {
                self.clear_runtime();
                self.diagnostics = vec![error.into_diagnostic()];
                return false;
            }
        };
        self.compile_from_shared_formula_with_lane_policy(
            processor,
            formula,
            compiled_formula,
            managed_formula,
            preserve_compatible_lanes,
        )
    }

    pub fn compile_from_shared_formula(
        &mut self,
        processor: &Processor,
        formula: &AlchemistFormula,
        compiled: Arc<CompiledAlchemistFormula>,
    ) -> bool {
        self.compile_from_shared_formula_with_lane_policy(processor, formula, compiled, None, false)
    }

    pub fn compile_from_shared_formula_with_compile_ctx(
        &mut self,
        processor: &Processor,
        formula: &AlchemistFormula,
        compiled: Arc<CompiledAlchemistFormula>,
        ctx: &CompileCtx<'_>,
    ) -> bool {
        let managed_formula = match ManagedFormulaRuntime::compile_with_shared_graph(
            formula,
            &processor.formula_instance,
            ctx,
            Arc::clone(&compiled.graph),
        ) {
            Ok(managed_formula) => managed_formula,
            Err(error) => {
                self.clear_runtime();
                self.diagnostics = vec![error.into_diagnostic()];
                return false;
            }
        };
        self.compile_from_shared_formula_with_lane_policy(processor, formula, compiled, managed_formula, false)
    }

    pub fn compile_from_shared_formula_with_compile_ctx_preserving_compatible_lanes(
        &mut self,
        processor: &Processor,
        formula: &AlchemistFormula,
        compiled: Arc<CompiledAlchemistFormula>,
        ctx: &CompileCtx<'_>,
    ) -> bool {
        let managed_formula = match ManagedFormulaRuntime::compile_with_shared_graph(
            formula,
            &processor.formula_instance,
            ctx,
            Arc::clone(&compiled.graph),
        ) {
            Ok(managed_formula) => managed_formula,
            Err(error) => {
                self.clear_runtime();
                self.diagnostics = vec![error.into_diagnostic()];
                return false;
            }
        };
        self.compile_from_shared_formula_with_lane_policy(processor, formula, compiled, managed_formula, true)
    }

    pub fn compile_from_shared_formula_preserving_compatible_lanes(
        &mut self,
        processor: &Processor,
        formula: &AlchemistFormula,
        compiled: Arc<CompiledAlchemistFormula>,
    ) -> bool {
        self.compile_from_shared_formula_with_lane_policy(processor, formula, compiled, None, true)
    }

    fn compile_from_shared_formula_with_lane_policy(
        &mut self,
        processor: &Processor,
        formula: &AlchemistFormula,
        compiled: Arc<CompiledAlchemistFormula>,
        managed_formula: Option<ManagedFormulaRuntime>,
        preserve_compatible_lanes: bool,
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
        if !self.compile_condition(processor, preserve_compatible_lanes) {
            return false;
        }
        self.subscriptions = compiled.graph.subscriptions.clone();
        let compile_key = FormulaCompileKey::from_formula(formula, 0, 0);
        let same_executable_graph = self.compiled_formula_key.as_ref() == Some(&compile_key);
        if !(preserve_compatible_lanes && same_executable_graph && self.lanes.is_compatible_with_graph(&compiled.graph))
        {
            self.lanes = LaneRuntimePool::for_graph(&compiled.graph);
        }
        if self.lanes.is_stateless() {
            self.stateless_scratch
                .get_or_insert_with(|| AlchemistMemory::for_graph(&compiled.graph));
        } else {
            self.stateless_scratch = None;
        }
        self.diagnostics = compiled.diagnostics.clone();
        self.plan = Some(ProcessorExecutionPlan::analyze(
            processor.id,
            &compiled.analysis,
            &ProcessorBindingAnalysis::default(),
            AxisSet::new(),
        ));
        self.compiled = Some(compiled);
        self.compiled_formula_key = Some(compile_key);
        self.managed_formula = managed_formula;
        self.dirty = ProcessorDirtyFlags::default();
        true
    }

    fn clear_runtime(&mut self) {
        self.compiled = None;
        self.compiled_formula_key = None;
        self.managed_formula = None;
        self.plan = None;
        self.compiled_condition = None;
        self.condition_runtimes.clear();
        self.lanes = LaneRuntimePool::default();
        if !self.managed_context_keys.is_empty() {
            self.managed_context_revision = self.managed_context_revision.wrapping_add(1);
        }
        self.managed_context_keys.clear();
        self.stateless_scratch = None;
        self.subscriptions.clear();
    }

    pub fn invalidate(&mut self, diagnostic: Diagnostic) {
        self.clear_runtime();
        self.diagnostics = vec![diagnostic];
    }

    fn compile_condition(&mut self, processor: &Processor, preserve_compatible_state: bool) -> bool {
        let Some(definition) = &processor.condition else {
            self.compiled_condition = None;
            self.condition_runtimes.clear();
            return true;
        };
        let program = match compile_condition(definition) {
            Ok(program) => Arc::new(program),
            Err(diagnostics) => {
                self.clear_runtime();
                self.diagnostics = diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        Diagnostic::error("condition_compile", diagnostic.message, DiagnosticOrigin::Graph)
                    })
                    .collect();
                return false;
            }
        };
        if preserve_compatible_state {
            for runtime in self.condition_runtimes.values_mut() {
                *runtime = runtime.migrate(&program);
            }
        } else {
            self.condition_runtimes.clear();
        }
        self.compiled_condition = Some(program);
        true
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
            if let Some(managed) = self.managed_formula.as_mut() {
                managed.reset_memory();
            }
        }
    }

    pub fn evaluate_processor(&mut self, processor: &Processor, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        let provider = DefaultProcessorContextProvider;
        merge_lane_outputs(self.evaluate_processor_with_context_provider(processor, ctx, &provider))
    }

    pub fn evaluate_processor_with_context_provider(
        &mut self,
        processor: &Processor,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
    ) -> Vec<ProcessorLaneOutput> {
        self.evaluate_processor_with_context_provider_and_capture(
            processor,
            ctx,
            context_provider,
            &ProcessorDebugCapture::default(),
        )
    }

    pub fn evaluate_processor_preview_with_context_provider(
        &mut self,
        processor: &Processor,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
        capture: &ProcessorDebugCapture,
    ) -> Vec<ANodeOutputPreviewSample> {
        let formula_id = self.compiled.as_ref().map(|compiled| compiled.formula_ref.id.clone());
        let lanes =
            self.evaluate_processor_with_context_provider_and_capture(processor, ctx, context_provider, capture);
        let Some(formula_id) = formula_id else {
            return Vec::new();
        };
        processor_output_preview_samples(processor.id, &formula_id, lanes)
    }

    pub fn evaluate_processor_with_context_provider_and_capture(
        &mut self,
        processor: &Processor,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
        capture: &ProcessorDebugCapture,
    ) -> Vec<ProcessorLaneOutput> {
        self.evaluate_processor_with_context_provider_and_capture_mode(
            processor,
            ctx,
            context_provider,
            capture,
            true,
            true,
        )
    }

    pub fn evaluate_processor_with_context_provider_and_runtime_capture(
        &mut self,
        processor: &Processor,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
        capture: &ProcessorDebugCapture,
    ) -> Vec<ProcessorLaneOutput> {
        self.evaluate_processor_with_context_provider_and_capture_mode(
            processor,
            ctx,
            context_provider,
            capture,
            false,
            true,
        )
    }

    /// Evaluates normal runtime work while capturing only outputs whose values changed.
    ///
    /// This is intended for retained live-preview streams: the caller keeps the last sample for
    /// each output, so recapturing unchanged values would only allocate duplicate debug data.
    pub fn evaluate_processor_with_context_provider_and_runtime_delta_capture(
        &mut self,
        processor: &Processor,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
        capture: &ProcessorDebugCapture,
    ) -> Vec<ProcessorLaneOutput> {
        self.evaluate_processor_with_context_provider_and_capture_mode(
            processor,
            ctx,
            context_provider,
            capture,
            false,
            false,
        )
    }

    fn evaluate_processor_with_context_provider_and_capture_mode(
        &mut self,
        processor: &Processor,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
        capture: &ProcessorDebugCapture,
        force_process_unchanged_inputs: bool,
        capture_unchanged_outputs: bool,
    ) -> Vec<ProcessorLaneOutput> {
        if !self.active {
            return Vec::new();
        }
        if self.managed_formula.is_some() {
            let axes = self
                .plan
                .as_ref()
                .map_or_else(AxisSet::new, |plan| plan.required_eval_axes.clone());
            let mut context_keys = context_provider.iter_context_keys(self.id, &axes).collect::<Vec<_>>();
            if context_keys.is_empty() && axes.is_empty() {
                context_keys.push(ContextKey::default_lane());
            }
            if context_keys.len() != self.managed_context_keys.len()
                || context_keys.iter().any(|key| !self.managed_context_keys.contains(key))
            {
                self.managed_context_keys = context_keys.iter().cloned().collect();
                self.managed_context_revision = self.managed_context_revision.wrapping_add(1);
                if let Some(managed) = self.managed_formula.as_mut() {
                    managed.retain_context_keys(&self.managed_context_keys);
                }
                self.condition_runtimes
                    .retain(|key, _| self.managed_context_keys.contains(key));
            }
            let graph_backed = self
                .managed_formula
                .as_ref()
                .is_some_and(ManagedFormulaRuntime::uses_authored_graph);
            let compiled = self.compiled.as_ref().map(Arc::clone);
            let mut lanes = Vec::with_capacity(context_keys.len());
            for context_key in context_keys {
                if !self.condition_passes(ctx, context_provider, &context_key) {
                    continue;
                }
                let capture_mode = compiled.as_ref().map_or(DebugCaptureMode::Off, |compiled| {
                    capture.debug_capture_mode(&compiled.formula_ref.id, &context_key)
                });
                let properties = if graph_backed {
                    match compiled.as_ref().map(|compiled| {
                        self.resolve_property_frame(processor, compiled, &context_key, context_provider)
                    }) {
                        Some(Ok(properties)) => Some(properties),
                        Some(Err(error)) => {
                            lanes.push(ProcessorLaneOutput {
                                context_key: (!context_key.is_default_lane()).then_some(context_key),
                                output: property_frame_error_output(error),
                            });
                            continue;
                        }
                        None => None,
                    }
                } else {
                    None
                };
                let mut output = self
                    .managed_formula
                    .as_mut()
                    .expect("managed formula presence was checked before condition evaluation")
                    .evaluate_with_context_frame(ctx, &context_key, properties.as_ref(), capture_mode.clone());
                if !graph_backed
                    && !matches!(capture_mode, DebugCaptureMode::Off)
                    && let Some(compiled) = &compiled
                    && let Ok(properties) =
                        self.resolve_property_frame(processor, compiled, &context_key, context_provider)
                {
                    let context = RuntimeContextFrame::new(context_key.clone());
                    let mut debug = DebugCaptureSink::new(capture_mode);
                    let frame = EvaluationFrame {
                        ctx,
                        properties: &properties,
                        context: &context,
                        debug: Some(&mut debug),
                        force_process_unchanged_inputs,
                        capture_unchanged_outputs,
                    };
                    let preview = match self.lanes.memory_for_key(context_key.clone(), &compiled.graph) {
                        Some(memory) => profile_kernel(|| evaluate_compiled_graph(&compiled.graph, memory, frame)),
                        None => {
                            let scratch = self
                                .stateless_scratch
                                .get_or_insert_with(|| AlchemistMemory::for_graph(&compiled.graph));
                            profile_kernel(|| evaluate_compiled_graph_fresh_reusing(&compiled.graph, scratch, frame))
                        }
                    };
                    output.debug_samples = preview.debug_samples;
                }
                lanes.push(ProcessorLaneOutput {
                    context_key: (!context_key.is_default_lane()).then_some(context_key),
                    output,
                });
            }
            return lanes;
        }
        let Some(compiled) = self.compiled.as_ref().map(Arc::clone) else {
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
            .collect::<Vec<_>>();
        if context_keys.is_empty() && plan.required_eval_axes.is_empty() {
            context_keys.push(ContextKey::default_lane());
        }
        let stateless = self.lanes.is_stateless();
        if !stateless {
            let memory_keys = context_keys
                .iter()
                .map(|context_key| context_key.project(&plan.required_memory_axes))
                .collect::<IndexSet<_>>();
            self.lanes.retain_keys(&memory_keys);
        }

        if self.compiled_condition.is_some() {
            context_keys.retain(|context_key| self.condition_passes(ctx, context_provider, context_key));
            let active_condition_keys = context_keys.iter().cloned().collect::<IndexSet<_>>();
            self.condition_runtimes
                .retain(|context_key, _| active_condition_keys.contains(context_key));
        } else {
            self.condition_runtimes.clear();
        }

        context_keys
            .into_iter()
            .map(|context_key| {
                let mut debug =
                    DebugCaptureSink::new(capture.debug_capture_mode(&compiled.formula_ref.id, &context_key));
                let context = RuntimeContextFrame::new(context_key.clone());
                let properties = match self.resolve_property_frame(processor, &compiled, &context_key, context_provider)
                {
                    Ok(properties) => properties,
                    Err(error) => {
                        return ProcessorLaneOutput {
                            context_key: (!context_key.is_default_lane()).then_some(context_key),
                            output: property_frame_error_output(error),
                        };
                    }
                };
                let frame = EvaluationFrame {
                    ctx,
                    properties: &properties,
                    context: &context,
                    debug: Some(&mut debug),
                    force_process_unchanged_inputs,
                    capture_unchanged_outputs,
                };
                let output = if stateless {
                    let scratch = self
                        .stateless_scratch
                        .get_or_insert_with(|| AlchemistMemory::for_graph(&compiled.graph));
                    profile_kernel(|| evaluate_compiled_graph_fresh_reusing(&compiled.graph, scratch, frame))
                } else {
                    let memory_key = context_key.project(&plan.required_memory_axes);
                    let memory = self
                        .lanes
                        .memory_for_key(memory_key, &compiled.graph)
                        .expect("stateful lane pools materialize memory for every active key");
                    profile_kernel(|| evaluate_compiled_graph(&compiled.graph, memory, frame))
                };
                ProcessorLaneOutput {
                    context_key: (!context_key.is_default_lane()).then_some(context_key),
                    output,
                }
            })
            .collect()
    }

    fn condition_passes(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
        context_key: &ContextKey,
    ) -> bool {
        let Some(program) = self.compiled_condition.as_ref().map(Arc::clone) else {
            return true;
        };
        let inputs = LaneConditionInputs {
            snapshot: ctx.inputs,
            context_provider,
            context_key,
        };
        let runtime = self
            .condition_runtimes
            .entry(context_key.clone())
            .or_insert_with(|| ConditionRuntime::new(&program));
        match runtime.evaluate_value(
            &program,
            &ConditionEvaluationFrame {
                logical_tick: ctx.logical_tick,
                delta_time: ctx.delta_time,
                inputs: &inputs,
            },
        ) {
            Ok(value) => value,
            Err(error) => {
                let message = error.to_string();
                if !self.diagnostics.iter().any(|diagnostic| diagnostic.message == message) {
                    self.diagnostics.push(Diagnostic::error(
                        "condition_evaluation",
                        message,
                        DiagnosticOrigin::Runtime,
                    ));
                }
                false
            }
        }
    }

    fn resolve_property_frame(
        &self,
        processor: &Processor,
        compiled: &CompiledAlchemistFormula,
        context_key: &ContextKey,
        context_provider: &dyn ProcessorContextProvider,
    ) -> Result<RuntimePropertyFrame, RuntimePropertyFrameError> {
        let mut overrides = IndexMap::new();
        for (surface_item, binding) in &processor.context_property_bindings {
            let property_id = FormulaPropertyId::new(surface_item.as_str());
            if compiled.properties.get(&property_id).is_some()
                && let Some(value) = context_provider.resolve_context_value(context_key, &binding.axis, &binding.path)
            {
                overrides.insert(property_id, value);
            }
        }
        for (surface_item, value) in &processor.formula_instance.overrides.values {
            let property_id = FormulaPropertyId::new(surface_item.as_str());
            if compiled.properties.get(&property_id).is_some() {
                overrides.entry(property_id).or_insert_with(|| value.clone());
            }
        }
        RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides)
    }
}

fn merge_lane_outputs(lanes: Vec<ProcessorLaneOutput>) -> RuntimeOutput {
    let mut output = RuntimeOutput::default();
    for lane in lanes {
        output.intents.extend(lane.output.intents);
        output.diagnostics.extend(lane.output.diagnostics);
        output.debug_samples.extend(lane.output.debug_samples);
    }
    output
}

fn property_frame_error_output(error: RuntimePropertyFrameError) -> RuntimeOutput {
    RuntimeOutput {
        diagnostics: vec![RuntimeDiagnostic {
            exec_node: ExecNodeId::new(0),
            message: error.to_string(),
        }],
        ..RuntimeOutput::default()
    }
}

#[derive(Clone, Debug)]
pub struct ProcessorLaneOutput {
    pub context_key: Option<ContextKey>,
    pub output: RuntimeOutput,
}
