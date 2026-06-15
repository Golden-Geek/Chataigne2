use std::sync::Arc;

use uuid::Uuid;

use golden_alchemist::{
    AlchemistFormula, AlchemistFormulaInstance, AlchemistMemory, CompileCtx, CompiledAlchemistFormula,
    DebugCaptureSink, Diagnostic, DiagnosticOrigin, EvaluationCtx, EvaluationFrame, FormulaRef, FormulaSurface,
    RuntimeContextFrame, RuntimeOutput, RuntimePropertyFrame, RuntimeSubscription, compile_graph,
    evaluate_compiled_graph,
};
use golden_statechart::StateId;

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
    pub memory: Option<AlchemistMemory>,
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
            memory: None,
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
        self.memory = Some(AlchemistMemory::for_graph(&compiled.graph));
        self.properties = Some(RuntimePropertyFrame::from_defaults(&compiled.properties));
        self.diagnostics = compiled.diagnostics.clone();
        self.compiled = Some(compiled);
        self.dirty = ProcessorDirtyFlags::default();
        true
    }

    fn clear_runtime(&mut self) {
        self.compiled = None;
        self.memory = None;
        self.properties = None;
        self.subscriptions.clear();
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
        if reset && let Some(compiled) = &self.compiled {
            self.memory = Some(AlchemistMemory::for_graph(&compiled.graph));
        }
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        if !self.active {
            return RuntimeOutput::default();
        }
        let (Some(compiled), Some(memory), Some(properties)) = (&self.compiled, &mut self.memory, &self.properties)
        else {
            return RuntimeOutput::default();
        };
        let mut debug = DebugCaptureSink::default();
        let context = RuntimeContextFrame;
        evaluate_compiled_graph(
            &compiled.graph,
            memory,
            EvaluationFrame {
                ctx,
                properties,
                context: &context,
                debug: &mut debug,
            },
        )
    }
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
