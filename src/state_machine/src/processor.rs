use uuid::Uuid;

use golden_alchemist::{
    AlchemistGraph, AlchemistRuntime, CompileCtx, Diagnostic, EvaluationCtx, ExposedSurface, RuntimeOutput,
    RuntimeSubscription, compile_graph,
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
pub struct ProcessorNode {
    pub id: ProcessorId,
    pub label: String,
    pub model_ref: Option<String>,
    pub graph: AlchemistGraph,
    pub exposed: ExposedSurface,
    pub lifecycle: ProcessorLifecyclePolicy,
    pub memory_policy: ProcessorMemoryPolicy,
    pub command_policy: ProcessorCommandPolicy,
}

impl ProcessorNode {
    #[must_use]
    pub fn new(label: impl Into<String>, graph: AlchemistGraph) -> Self {
        let exposed = graph.exposed.clone();
        Self {
            id: ProcessorId::new(),
            label: label.into(),
            model_ref: None,
            graph,
            exposed,
            lifecycle: ProcessorLifecyclePolicy::default(),
            memory_policy: ProcessorMemoryPolicy::default(),
            command_policy: ProcessorCommandPolicy::default(),
        }
    }

    #[must_use]
    pub fn ui_model(&self, diagnostics: Vec<Diagnostic>) -> ProcessorUiModel {
        ProcessorUiModel {
            id: self.id,
            label: self.label.clone(),
            exposed: self.exposed.clone(),
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
    pub runtime: Option<AlchemistRuntime>,
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
            runtime: None,
            active: false,
            dirty: ProcessorDirtyFlags {
                graph: true,
                ..ProcessorDirtyFlags::default()
            },
            subscriptions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn compile(&mut self, processor: &ProcessorNode, ctx: &CompileCtx<'_>) -> bool {
        let result = compile_graph(&processor.graph, ctx);
        self.diagnostics = result.diagnostics;
        let Some(compiled) = result.compiled else {
            self.runtime = None;
            return false;
        };
        self.subscriptions = compiled.subscriptions.clone();
        self.runtime = Some(AlchemistRuntime::new(compiled));
        self.dirty = ProcessorDirtyFlags::default();
        true
    }

    pub fn apply_lifecycle(&mut self, processor: &ProcessorNode, event: ProcessorLifecycleEvent) {
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
        if reset && let Some(compiled) = self.runtime.as_ref().map(|runtime| runtime.compiled.clone()) {
            self.runtime = Some(AlchemistRuntime::new(compiled));
        }
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        if !self.active {
            return RuntimeOutput::default();
        }
        self.runtime
            .as_mut()
            .map_or_else(RuntimeOutput::default, |runtime| runtime.evaluate(ctx))
    }
}

#[derive(Clone, Debug)]
pub struct ProcessorUiModel {
    pub id: ProcessorId,
    pub label: String,
    pub exposed: ExposedSurface,
    pub diagnostics: Vec<Diagnostic>,
}
