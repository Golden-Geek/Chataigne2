use std::{collections::HashMap, fmt, sync::Arc};

use uuid::Uuid;

use golden_alchemist::{
    ANodeId, AlchemistFormula, AlchemistFormulaInstance, AlchemistMemory, AxisSet, CompileCtx,
    CompiledAlchemistFormula, ContextAxisId, ContextItemId, ContextKey, ContextKeyPart, ContextValuePath,
    DebugCaptureMode, DebugCaptureSink, DebugValueSample, Diagnostic, DiagnosticOrigin, EvaluationCtx, EvaluationFrame,
    ExecNodeId, FormulaAnalysis, FormulaId, FormulaPropertyId, FormulaRef, FormulaSurface, LaneRuntimePool,
    ManagedRegionInstances, OutputPreviewStatus, RuntimeContextFrame, RuntimeDiagnostic, RuntimeOutput,
    RuntimePropertyFrame, RuntimePropertyFrameError, RuntimeSubscription, RuntimeValue, SocketId, SurfaceItemId,
    ValueTypeId, compile_graph, evaluate_compiled_graph, evaluate_compiled_graph_stateless,
};
use golden_statechart::StateId;
use indexmap::{IndexMap, IndexSet};
use rayon::prelude::*;

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

pub trait ProcessorContextProvider: Sync {
    fn available_axes(&self, processor_id: ProcessorId) -> AxisSet;

    fn context_axis_items(
        &self,
        processor_id: ProcessorId,
        axes: &AxisSet,
    ) -> Result<Vec<(ContextAxisId, Vec<ContextItemId>)>, ProcessorMultiplexError>;

    fn lane_count(
        &self,
        processor_id: ProcessorId,
        axes: &AxisSet,
        limits: ProcessorMultiplexLimits,
    ) -> Result<usize, ProcessorMultiplexError> {
        let axis_items = self.context_axis_items(processor_id, axes)?;
        let axis_lengths = axis_items
            .iter()
            .map(|(axis, items)| (axis.clone(), items.len()))
            .collect::<Vec<_>>();
        checked_context_cardinality(&axis_lengths, limits)
    }

    fn iter_context_keys(
        &self,
        processor_id: ProcessorId,
        axes: &AxisSet,
        limits: ProcessorMultiplexLimits,
    ) -> Result<ContextKeyProduct, ProcessorMultiplexError> {
        let axis_items = self.context_axis_items(processor_id, axes)?;
        let axis_lengths = axis_items
            .iter()
            .map(|(axis, items)| (axis.clone(), items.len()))
            .collect::<Vec<_>>();
        checked_context_cardinality(&axis_lengths, limits)?;
        Ok(ContextKeyProduct::new(axis_items))
    }

    fn resolve_context_value(
        &self,
        key: &ContextKey,
        axis: &ContextAxisId,
        path: &ContextValuePath,
    ) -> Option<RuntimeValue>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessorMultiplexLimits {
    pub max_items_per_axis: usize,
    pub max_lanes_per_processor: usize,
    pub max_total_active_lanes: usize,
}

impl Default for ProcessorMultiplexLimits {
    fn default() -> Self {
        Self {
            max_items_per_axis: 4_096,
            max_lanes_per_processor: 16_384,
            max_total_active_lanes: 65_536,
        }
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProcessorMultiplexError {
    #[error("processor {processor_id} has no context runtime")]
    MissingProcessor { processor_id: ProcessorId },
    #[error("processor {processor_id} does not provide context axis {axis:?}")]
    MissingAxis {
        processor_id: ProcessorId,
        axis: ContextAxisId,
    },
    #[error("context axis {axis:?} contains {items} items, exceeding the per-axis budget of {limit}")]
    AxisBudgetExceeded {
        axis: ContextAxisId,
        items: usize,
        limit: usize,
    },
    #[error("context lane cardinality overflowed while multiplying axis {axis:?}")]
    CardinalityOverflow { axis: ContextAxisId },
    #[error("context lane cardinality {lanes} exceeds the per-processor budget of {limit}")]
    LaneBudgetExceeded { lanes: usize, limit: usize },
    #[error("active processor lane cardinality {lanes} exceeds the runtime budget of {limit}")]
    RuntimeLaneBudgetExceeded { lanes: usize, limit: usize },
    #[error("active processor lane cardinality overflowed the platform size")]
    RuntimeCardinalityOverflow,
}

pub fn checked_context_cardinality(
    axis_lengths: &[(ContextAxisId, usize)],
    limits: ProcessorMultiplexLimits,
) -> Result<usize, ProcessorMultiplexError> {
    let mut cardinality = 1usize;
    for (axis, item_count) in axis_lengths {
        if *item_count > limits.max_items_per_axis {
            return Err(ProcessorMultiplexError::AxisBudgetExceeded {
                axis: axis.clone(),
                items: *item_count,
                limit: limits.max_items_per_axis,
            });
        }
        cardinality = cardinality
            .checked_mul(*item_count)
            .ok_or_else(|| ProcessorMultiplexError::CardinalityOverflow { axis: axis.clone() })?;
        if cardinality > limits.max_lanes_per_processor {
            return Err(ProcessorMultiplexError::LaneBudgetExceeded {
                lanes: cardinality,
                limit: limits.max_lanes_per_processor,
            });
        }
    }
    Ok(cardinality)
}

pub struct ContextKeyProduct {
    axes: Vec<(ContextAxisId, Vec<ContextItemId>)>,
    indexes: Vec<usize>,
    exhausted: bool,
}

impl ContextKeyProduct {
    fn new(axes: Vec<(ContextAxisId, Vec<ContextItemId>)>) -> Self {
        let exhausted = axes.iter().any(|(_, items)| items.is_empty());
        let indexes = vec![0; axes.len()];
        Self {
            axes,
            indexes,
            exhausted,
        }
    }
}

impl Iterator for ContextKeyProduct {
    type Item = ContextKey;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }
        let key = ContextKey::new(
            self.axes
                .iter()
                .zip(&self.indexes)
                .map(|((axis, items), index)| ContextKeyPart::new(axis.clone(), items[*index].clone())),
        );
        if self.axes.is_empty() {
            self.exhausted = true;
            return Some(key);
        }
        for axis_index in (0..self.axes.len()).rev() {
            self.indexes[axis_index] += 1;
            if self.indexes[axis_index] < self.axes[axis_index].1.len() {
                return Some(key);
            }
            self.indexes[axis_index] = 0;
        }
        self.exhausted = true;
        Some(key)
    }
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

    fn context_axis_items(
        &self,
        processor_id: ProcessorId,
        axes: &AxisSet,
    ) -> Result<Vec<(ContextAxisId, Vec<ContextItemId>)>, ProcessorMultiplexError> {
        if axes.is_empty() {
            Ok(Vec::new())
        } else {
            Err(ProcessorMultiplexError::MissingAxis {
                processor_id,
                axis: axes.first().cloned().expect("non-empty axis set"),
            })
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
    pub context_property_bindings: IndexMap<SurfaceItemId, ProcessorContextPropertyBinding>,
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
    pub managed_formula: Option<ManagedFormulaRuntime>,
    pub plan: Option<ProcessorExecutionPlan>,
    pub lanes: LaneRuntimePool,
    pub active: bool,
    pub dirty: ProcessorDirtyFlags,
    pub subscriptions: Vec<RuntimeSubscription>,
    pub diagnostics: Vec<Diagnostic>,
    pub multiplex_limits: ProcessorMultiplexLimits,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProcessorDebugCapture {
    #[default]
    Off,
    All {
        history_len: usize,
    },
    ProcessorLane {
        context_key: Option<ContextKey>,
        history_len: usize,
    },
    ProcessorLanes {
        context_keys: IndexSet<Option<ContextKey>>,
        history_len: usize,
    },
    SelectedNodes {
        context_key: Option<ContextKey>,
        nodes: IndexSet<ANodeId>,
        history_len: usize,
    },
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
            Self::ProcessorLanes {
                context_keys,
                history_len,
            } => DebugCaptureMode::ProcessorLanes {
                formula_id: formula_id.clone(),
                context_keys: context_keys.clone(),
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
            multiplex_limits: ProcessorMultiplexLimits::default(),
        }
    }

    pub fn set_multiplex_limits(&mut self, limits: ProcessorMultiplexLimits) {
        self.multiplex_limits = limits;
    }

    pub fn planned_lane_count(
        &self,
        context_provider: &dyn ProcessorContextProvider,
    ) -> Result<usize, ProcessorMultiplexError> {
        if !self.active || self.compiled.is_none() {
            return Ok(0);
        }
        if self.managed_formula.is_some() {
            return Ok(1);
        }
        let compiled = self.compiled.as_ref().expect("compiled runtime checked above");
        let plan = self.plan.clone().unwrap_or_else(|| {
            ProcessorExecutionPlan::analyze(
                self.id,
                &compiled.analysis,
                &ProcessorBindingAnalysis::default(),
                context_provider.available_axes(self.id),
            )
        });
        context_provider.lane_count(self.id, &plan.required_eval_axes, self.multiplex_limits)
    }

    pub fn multiplex_diagnostic_output(&self, error: ProcessorMultiplexError) -> RuntimeOutput {
        if let Some(compiled) = self.compiled.as_ref() {
            multiplex_error_lane(compiled, error).output
        } else {
            RuntimeOutput {
                diagnostics: vec![RuntimeDiagnostic {
                    exec_node: ExecNodeId::new(0),
                    message: error.to_string(),
                }],
                ..RuntimeOutput::default()
            }
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

    pub fn compile_from_shared_formula_with_compile_ctx_preserving_compatible_lanes(
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
        processor_output_preview_samples(processor.id, &formula_id, &lanes)
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
        if let Some(managed_formula) = self.managed_formula.as_mut() {
            let mut output = managed_formula.evaluate(ctx);
            let Some(compiled) = self.compiled.as_ref().map(Arc::clone) else {
                return vec![ProcessorLaneOutput {
                    context_key: None,
                    output,
                }];
            };
            let capture_mode = capture.debug_capture_mode(&compiled.formula_ref.id);
            if !matches!(capture_mode, DebugCaptureMode::Off) {
                let context_key = ContextKey::default_lane();
                let context = RuntimeContextFrame::new(context_key.clone());
                if let Ok(properties) =
                    resolve_processor_property_frame(processor, &compiled, &context_key, context_provider)
                {
                    let mut debug = DebugCaptureSink::new(capture_mode);
                    let preview = match self.lanes.memory_for_key(context_key, &compiled.graph) {
                        Some(memory) => evaluate_compiled_graph(
                            &compiled.graph,
                            memory,
                            EvaluationFrame {
                                ctx,
                                properties: &properties,
                                context: &context,
                                debug: &mut debug,
                                force_process_unchanged_inputs,
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
                                force_process_unchanged_inputs,
                                capture_unchanged_outputs,
                            },
                        ),
                    };
                    output.debug_samples = preview.debug_samples;
                }
            }
            return vec![ProcessorLaneOutput {
                context_key: None,
                output,
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
        let lane_count = match context_provider.lane_count(self.id, &plan.required_eval_axes, self.multiplex_limits) {
            Ok(lane_count) => lane_count,
            Err(error) => return vec![multiplex_error_lane(&compiled, error)],
        };
        let context_keys =
            match context_provider.iter_context_keys(self.id, &plan.required_eval_axes, self.multiplex_limits) {
                Ok(context_keys) => context_keys,
                Err(error) => return vec![multiplex_error_lane(&compiled, error)],
            };
        let mut bounded_context_keys = IndexSet::with_capacity(lane_count);
        bounded_context_keys.extend(context_keys);
        let mut context_keys = bounded_context_keys;
        debug_assert!(context_keys.len() <= lane_count);
        if context_keys.is_empty() && plan.required_eval_axes.is_empty() {
            context_keys.insert(ContextKey::default_lane());
        }
        let memory_keys = context_keys
            .iter()
            .map(|context_key| context_key.project(&plan.required_memory_axes))
            .collect::<IndexSet<_>>();
        self.lanes.retain_keys(&memory_keys);

        let context_keys = context_keys.into_iter().collect::<Vec<_>>();
        if self.lanes.is_stateless() && context_keys.len() >= 32 && matches!(capture, ProcessorDebugCapture::Off) {
            return context_keys
                .into_par_iter()
                .map(|context_key| {
                    evaluate_stateless_processor_lane(
                        processor,
                        &compiled,
                        context_key,
                        ctx,
                        context_provider,
                        capture,
                        force_process_unchanged_inputs,
                        capture_unchanged_outputs,
                    )
                })
                .collect();
        }

        let one_memory_per_lane = context_keys
            .iter()
            .all(|context_key| context_key.project(&plan.required_memory_axes) == *context_key);
        if one_memory_per_lane && context_keys.len() >= 32 && !self.lanes.is_stateless() {
            for context_key in &context_keys {
                let _ = self.lanes.memory_for_key(context_key.clone(), &compiled.graph);
            }
            let lane_order = context_keys
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, key)| (key, index))
                .collect::<HashMap<_, _>>();
            let lanes = self
                .lanes
                .stateful_memories_mut()
                .expect("stateful lane pool was checked above");
            let mut outputs = lanes
                .par_iter_mut()
                .map(|(context_key, memory)| {
                    let index = lane_order[context_key];
                    let output = evaluate_stateful_processor_lane(
                        processor,
                        &compiled,
                        context_key.clone(),
                        memory,
                        ctx,
                        context_provider,
                        capture,
                        force_process_unchanged_inputs,
                        capture_unchanged_outputs,
                    );
                    (index, output)
                })
                .collect::<Vec<_>>();
            outputs.sort_unstable_by_key(|(index, _)| *index);
            return outputs.into_iter().map(|(_, output)| output).collect();
        }

        context_keys
            .into_iter()
            .map(|context_key| {
                let mut debug = DebugCaptureSink::new(capture.debug_capture_mode(&compiled.formula_ref.id));
                let context = RuntimeContextFrame::new(context_key.clone());
                let properties =
                    match resolve_processor_property_frame(processor, &compiled, &context_key, context_provider) {
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
                            force_process_unchanged_inputs,
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
                            force_process_unchanged_inputs,
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
}

fn resolve_processor_property_frame(
    processor: &Processor,
    compiled: &CompiledAlchemistFormula,
    context_key: &ContextKey,
    context_provider: &dyn ProcessorContextProvider,
) -> Result<RuntimePropertyFrame, RuntimePropertyFrameError> {
    let mut overrides = IndexMap::new();
    for (surface_item, value) in &processor.formula_instance.overrides.values {
        let property_id = FormulaPropertyId::new(surface_item.as_str());
        if compiled.properties.get(&property_id).is_some() {
            let value = processor
                .context_property_bindings
                .get(surface_item)
                .and_then(|binding| context_provider.resolve_context_value(context_key, &binding.axis, &binding.path))
                .unwrap_or_else(|| value.clone());
            overrides.insert(property_id, value);
        }
    }
    RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_stateless_processor_lane(
    processor: &Processor,
    compiled: &CompiledAlchemistFormula,
    context_key: ContextKey,
    ctx: &EvaluationCtx<'_>,
    context_provider: &dyn ProcessorContextProvider,
    capture: &ProcessorDebugCapture,
    force_process_unchanged_inputs: bool,
    capture_unchanged_outputs: bool,
) -> ProcessorLaneOutput {
    let mut debug = DebugCaptureSink::new(capture.debug_capture_mode(&compiled.formula_ref.id));
    let context = RuntimeContextFrame::new(context_key.clone());
    let properties = match resolve_processor_property_frame(processor, compiled, &context_key, context_provider) {
        Ok(properties) => properties,
        Err(error) => {
            return ProcessorLaneOutput {
                context_key: (!context_key.is_default_lane()).then_some(context_key),
                output: property_frame_error_output(error),
            };
        }
    };
    let output = evaluate_compiled_graph_stateless(
        &compiled.graph,
        EvaluationFrame {
            ctx,
            properties: &properties,
            context: &context,
            debug: &mut debug,
            force_process_unchanged_inputs,
            capture_unchanged_outputs,
        },
    );
    ProcessorLaneOutput {
        context_key: (!context_key.is_default_lane()).then_some(context_key),
        output,
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_stateful_processor_lane(
    processor: &Processor,
    compiled: &CompiledAlchemistFormula,
    context_key: ContextKey,
    memory: &mut AlchemistMemory,
    ctx: &EvaluationCtx<'_>,
    context_provider: &dyn ProcessorContextProvider,
    capture: &ProcessorDebugCapture,
    force_process_unchanged_inputs: bool,
    capture_unchanged_outputs: bool,
) -> ProcessorLaneOutput {
    let mut debug = DebugCaptureSink::new(capture.debug_capture_mode(&compiled.formula_ref.id));
    let context = RuntimeContextFrame::new(context_key.clone());
    let properties = match resolve_processor_property_frame(processor, compiled, &context_key, context_provider) {
        Ok(properties) => properties,
        Err(error) => {
            return ProcessorLaneOutput {
                context_key: (!context_key.is_default_lane()).then_some(context_key),
                output: property_frame_error_output(error),
            };
        }
    };
    let output = evaluate_compiled_graph(
        &compiled.graph,
        memory,
        EvaluationFrame {
            ctx,
            properties: &properties,
            context: &context,
            debug: &mut debug,
            force_process_unchanged_inputs,
            capture_unchanged_outputs,
        },
    );
    ProcessorLaneOutput {
        context_key: (!context_key.is_default_lane()).then_some(context_key),
        output,
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

fn multiplex_error_lane(compiled: &CompiledAlchemistFormula, error: ProcessorMultiplexError) -> ProcessorLaneOutput {
    let exec_node = compiled
        .graph
        .topo_order
        .first()
        .copied()
        .unwrap_or_else(|| ExecNodeId::new(0));
    ProcessorLaneOutput {
        context_key: None,
        output: RuntimeOutput {
            diagnostics: vec![RuntimeDiagnostic {
                exec_node,
                message: error.to_string(),
            }],
            ..RuntimeOutput::default()
        },
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
    lanes: &[ProcessorLaneOutput],
) -> Vec<ANodeOutputPreviewSample> {
    lanes
        .iter()
        .flat_map(|lane| {
            lane.output
                .debug_samples
                .iter()
                .cloned()
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
    pub formula_source_key: Option<String>,
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
