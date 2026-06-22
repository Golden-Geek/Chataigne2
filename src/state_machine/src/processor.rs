use std::{fmt, sync::Arc};

use uuid::Uuid;

use golden_alchemist::{
    ANodeId, AlchemistFormula, AlchemistFormulaInstance, AxisSet, CompileCtx, CompiledAlchemistFormula, ContextAxisId,
    ContextKey, ContextValuePath, DebugCaptureMode, DebugCaptureSink, DebugValueSample, Diagnostic, DiagnosticOrigin,
    EvaluationCtx, EvaluationFrame, ExecNodeId, FormulaAnalysis, FormulaId, FormulaPropertyId, FormulaRef,
    FormulaSurface, LaneRuntimePool, ManagedRegionInstances, OutputPreviewStatus, RuntimeContextFrame,
    RuntimeDiagnostic, RuntimeOutput, RuntimePropertyFrame, RuntimePropertyFrameError, RuntimeSubscription,
    RuntimeValue, SocketId, ValueTypeId, compile_graph, evaluate_compiled_graph, evaluate_compiled_graph_stateless,
};
use golden_statechart::StateId;
use indexmap::{IndexMap, IndexSet};

use crate::ManagedFormulaRuntime;

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
        if formula.has_input_gated_nodes {
            extend_axes(&mut required_memory_axes, &bindings.property_axes);
            extend_axes(&mut required_memory_axes, &formula.explicit_context_axes);
            extend_axes(&mut required_memory_axes, &bindings.input_axes);
            extend_axes(&mut required_memory_axes, &formula.effect_axes);
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
        self.ui_model_with_formula_source(formula, diagnostics, ProcessorFormulaUiState::default())
    }

    #[must_use]
    pub fn ui_model_with_formula_source(
        &self,
        formula: &AlchemistFormula,
        diagnostics: Vec<Diagnostic>,
        formula_source: ProcessorFormulaUiState,
    ) -> ProcessorUiModel {
        ProcessorUiModel {
            id: self.id,
            label: self.label.clone(),
            active: self.enabled,
            formula_id: formula.id.to_string(),
            formula_label: formula.label.clone(),
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
    pub managed_formula: Option<ManagedFormulaRuntime>,
    pub plan: Option<ProcessorExecutionPlan>,
    pub lanes: LaneRuntimePool,
    pub active: bool,
    pub dirty: ProcessorDirtyFlags,
    pub subscriptions: Vec<RuntimeSubscription>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessorDebugCapture {
    Off,
    All {
        history_len: usize,
    },
    ProcessorLane {
        context_key: Option<ContextKey>,
        history_len: usize,
    },
    SelectedNodes {
        context_key: Option<ContextKey>,
        nodes: IndexSet<ANodeId>,
        history_len: usize,
    },
}

impl Default for ProcessorDebugCapture {
    fn default() -> Self {
        Self::Off
    }
}

impl ProcessorDebugCapture {
    fn debug_capture_mode(&self, formula_id: &FormulaId) -> DebugCaptureMode {
        match self {
            Self::Off => DebugCaptureMode::Off,
            Self::All { history_len } => DebugCaptureMode::All {
                history_len: *history_len,
            },
            Self::ProcessorLane {
                context_key,
                history_len,
            } => DebugCaptureMode::ProcessorLane {
                formula_id: formula_id.clone(),
                context_key: context_key.clone(),
                history_len: *history_len,
            },
            Self::SelectedNodes {
                context_key,
                nodes,
                history_len,
            } => DebugCaptureMode::SelectedNodes {
                formula_id: Some(formula_id.clone()),
                context_key: context_key.clone(),
                nodes: nodes.clone(),
                history_len: *history_len,
            },
        }
    }
}

impl ProcessorRuntime {
    #[must_use]
    pub fn new(id: ProcessorId) -> Self {
        Self {
            id,
            compiled: None,
            managed_formula: None,
            plan: None,
            lanes: LaneRuntimePool::default(),
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
        let managed_formula = match ManagedFormulaRuntime::compile(formula, &processor.formula_instance, ctx) {
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
        let managed_formula = match ManagedFormulaRuntime::compile(formula, &processor.formula_instance, ctx) {
            Ok(managed_formula) => managed_formula,
            Err(error) => {
                self.clear_runtime();
                self.diagnostics = vec![error.into_diagnostic()];
                return false;
            }
        };
        self.compile_from_shared_formula_with_lane_policy(processor, formula, compiled, managed_formula, false)
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
        self.subscriptions = compiled.graph.subscriptions.clone();
        if !(preserve_compatible_lanes && self.lanes.is_compatible_with_graph(&compiled.graph)) {
            self.lanes = LaneRuntimePool::for_graph(&compiled.graph);
        }
        self.diagnostics = compiled.diagnostics.clone();
        self.plan = Some(ProcessorExecutionPlan::analyze(
            processor.id,
            &compiled.analysis,
            &ProcessorBindingAnalysis::default(),
            AxisSet::new(),
        ));
        self.compiled = Some(compiled);
        self.managed_formula = managed_formula;
        self.dirty = ProcessorDirtyFlags::default();
        true
    }

    fn clear_runtime(&mut self) {
        self.compiled = None;
        self.managed_formula = None;
        self.plan = None;
        self.lanes = LaneRuntimePool::default();
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
        self.evaluate_processor_with_context_provider_and_capture_mode(processor, ctx, context_provider, capture, true)
    }

    pub fn evaluate_processor_with_context_provider_and_send_capture(
        &mut self,
        processor: &Processor,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
        capture: &ProcessorDebugCapture,
    ) -> Vec<ProcessorLaneOutput> {
        self.evaluate_processor_with_context_provider_and_capture_mode(processor, ctx, context_provider, capture, false)
    }

    fn evaluate_processor_with_context_provider_and_capture_mode(
        &mut self,
        processor: &Processor,
        ctx: &EvaluationCtx<'_>,
        context_provider: &dyn ProcessorContextProvider,
        capture: &ProcessorDebugCapture,
        capture_unchanged_outputs: bool,
    ) -> Vec<ProcessorLaneOutput> {
        if !self.active {
            return Vec::new();
        }
        if let Some(managed_formula) = self.managed_formula.as_mut() {
            return vec![ProcessorLaneOutput {
                context_key: None,
                output: managed_formula.evaluate(ctx),
            }];
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
                let mut debug = DebugCaptureSink::new(capture.debug_capture_mode(&compiled.formula_ref.id));
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
                            force_process_unchanged_inputs: capture_unchanged_outputs,
                            capture_unchanged_outputs,
                        },
                    ),
                    None => evaluate_compiled_graph_stateless(
                        &compiled.graph,
                        EvaluationFrame {
                            ctx,
                            properties: &properties,
                            context: &context,
                            debug: &mut debug,
                            force_process_unchanged_inputs: capture_unchanged_outputs,
                            capture_unchanged_outputs,
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

    fn resolve_property_frame(
        &self,
        processor: &Processor,
        compiled: &CompiledAlchemistFormula,
        _context_key: &ContextKey,
        _context_provider: &dyn ProcessorContextProvider,
    ) -> Result<RuntimePropertyFrame, RuntimePropertyFrameError> {
        let mut overrides = IndexMap::new();
        for (surface_item, value) in &processor.formula_instance.overrides.values {
            let property_id = FormulaPropertyId::new(surface_item.as_str());
            if compiled.properties.get(&property_id).is_some() {
                overrides.insert(property_id, value.clone());
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

#[derive(Clone, Debug, PartialEq)]
pub struct ANodeOutputPreviewSample {
    pub formula_id: FormulaId,
    pub processor_id: Option<ProcessorId>,
    pub context_key: Option<ContextKey>,
    pub author_node_id: ANodeId,
    pub exec_node: ExecNodeId,
    pub output_socket: SocketId,
    pub value_type: ValueTypeId,
    pub value: RuntimeValue,
    pub logical_tick: u64,
    pub status: OutputPreviewStatus,
}

impl ANodeOutputPreviewSample {
    fn from_debug_sample(
        processor_id: Option<ProcessorId>,
        fallback_formula_id: &FormulaId,
        sample: DebugValueSample,
    ) -> Self {
        Self {
            formula_id: sample.formula_id.unwrap_or_else(|| fallback_formula_id.clone()),
            processor_id,
            context_key: sample.context_key,
            author_node_id: sample.author_node_id,
            exec_node: sample.exec_node,
            output_socket: sample.output_socket,
            value_type: sample.value_type,
            value: sample.value,
            logical_tick: sample.logical_tick,
            status: sample.status,
        }
    }
}

pub fn processor_output_preview_samples(
    processor_id: ProcessorId,
    formula_id: &FormulaId,
    lanes: Vec<ProcessorLaneOutput>,
) -> Vec<ANodeOutputPreviewSample> {
    lanes
        .into_iter()
        .flat_map(|lane| {
            lane.output
                .debug_samples
                .into_iter()
                .map(move |sample| ANodeOutputPreviewSample::from_debug_sample(Some(processor_id), formula_id, sample))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ProcessorUiModel {
    pub id: ProcessorId,
    pub label: String,
    pub active: bool,
    pub formula_id: String,
    pub formula_label: String,
    pub surface: FormulaSurface,
    pub managed_region_instances: ManagedRegionInstances,
    pub diagnostics: Vec<Diagnostic>,
    pub formula_source: ProcessorFormulaUiState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessorFormulaSourceKind {
    Project,
    Builtin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessorFormulaUiState {
    pub source_kind: ProcessorFormulaSourceKind,
    pub open_readonly_from_processor: bool,
    pub can_duplicate_to_library: bool,
}

impl ProcessorFormulaUiState {
    #[must_use]
    pub fn project() -> Self {
        Self {
            source_kind: ProcessorFormulaSourceKind::Project,
            open_readonly_from_processor: false,
            can_duplicate_to_library: false,
        }
    }

    #[must_use]
    pub fn builtin(open_readonly_from_processor: bool, can_duplicate_to_library: bool) -> Self {
        Self {
            source_kind: ProcessorFormulaSourceKind::Builtin,
            open_readonly_from_processor,
            can_duplicate_to_library,
        }
    }
}

impl Default for ProcessorFormulaUiState {
    fn default() -> Self {
        Self::project()
    }
}
