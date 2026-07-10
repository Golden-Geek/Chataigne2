use std::{cell::Cell, marker::PhantomData, sync::Arc};

use crate::{
    CompilationRequestId, CompilationService, EffectCommitter, GenerationSpec, GenerationSwapReport, InputUpdate,
    RuntimeCompileError, RuntimeGeneration, SemanticRuntime, SemanticRuntimeError, TickMetrics,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ControlPlaneEvent {
    GenerationActivated {
        request: CompilationRequestId,
        swap: GenerationSwapReport,
    },
    CompilationRejected {
        request: CompilationRequestId,
        error: RuntimeCompileError,
    },
    Superseded {
        request: CompilationRequestId,
    },
}

/// Single-owner actor state. The `Cell` marker intentionally makes this type
/// `!Sync`; commands must be serialized by the host actor or event loop.
pub struct RuntimeControlPlane {
    semantic: SemanticRuntime,
    compilation: CompilationService,
    latest_request: Option<CompilationRequestId>,
    _single_owner: PhantomData<Cell<()>>,
}

impl RuntimeControlPlane {
    pub fn new(initial: Arc<RuntimeGeneration>) -> Self {
        Self {
            semantic: SemanticRuntime::new(initial),
            compilation: CompilationService::spawn(),
            latest_request: None,
            _single_owner: PhantomData,
        }
    }

    pub fn semantic(&self) -> &SemanticRuntime {
        &self.semantic
    }

    pub fn semantic_mut(&mut self) -> &mut SemanticRuntime {
        &mut self.semantic
    }

    pub fn request_compilation(&mut self, spec: GenerationSpec) -> CompilationRequestId {
        let request = self.compilation.request(spec);
        self.latest_request = Some(request);
        request
    }

    pub fn poll_compilation(&mut self) -> Option<ControlPlaneEvent> {
        let result = self.compilation.try_receive()?;
        if self.latest_request.is_some_and(|latest| result.request < latest) {
            return Some(ControlPlaneEvent::Superseded {
                request: result.request,
            });
        }
        match result.generation {
            Ok(generation) => {
                let swap = self.semantic.swap_generation(generation);
                self.latest_request = None;
                Some(ControlPlaneEvent::GenerationActivated {
                    request: result.request,
                    swap,
                })
            }
            Err(error) => {
                self.latest_request = None;
                Some(ControlPlaneEvent::CompilationRejected {
                    request: result.request,
                    error,
                })
            }
        }
    }

    pub fn tick(
        &mut self,
        updates: &[InputUpdate],
        committer: &mut impl EffectCommitter,
    ) -> Result<TickMetrics, SemanticRuntimeError> {
        self.semantic.tick(updates, committer)
    }
}
