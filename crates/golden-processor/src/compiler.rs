use std::sync::Arc;

use golden_alchemist::{ANodeRegistry, AlchemistFormula, CompiledFormulaKernel, FormulaCompileCache};
use golden_context::{ContextError, ContextKey, ContextLayer, ContextLimits, LaneLayout, compose_context_layers};
use thiserror::Error;

use crate::ProcessorDefinition;

pub struct ProcessorPlan {
    pub definition: ProcessorDefinition,
    pub kernel: Arc<CompiledFormulaKernel>,
    pub lane_layout: LaneLayout,
    pub lane_keys: Vec<ContextKey>,
}

#[derive(Default)]
pub struct ProcessorCompiler;

impl ProcessorCompiler {
    pub fn compile(
        &self,
        definition: ProcessorDefinition,
        formula: &AlchemistFormula,
        context_layers: &[ContextLayer],
        limits: ContextLimits,
        cache: &FormulaCompileCache,
        registry: &ANodeRegistry,
    ) -> Result<ProcessorPlan, ProcessorCompileError> {
        if definition.formula != formula.id {
            return Err(ProcessorCompileError::FormulaMismatch);
        }
        for binding in &definition.inputs {
            if !formula.surface.inputs.iter().any(|input| input.id == binding.target) {
                return Err(ProcessorCompileError::MissingInput(binding.target));
            }
        }
        for output in &definition.outputs {
            if !formula.surface.outputs.iter().any(|item| item.id == *output) {
                return Err(ProcessorCompileError::MissingOutput(*output));
            }
        }
        let lane_layout = LaneLayout::compile(compose_context_layers(context_layers), limits)?;
        let lane_keys = (0..lane_layout.lane_count())
            .filter_map(|lane| lane_layout.key_at(lane))
            .collect();
        Ok(ProcessorPlan {
            definition,
            kernel: cache.compile(formula, registry)?,
            lane_layout,
            lane_keys,
        })
    }
}

#[derive(Debug, Error)]
pub enum ProcessorCompileError {
    #[error("processor references a different formula")]
    FormulaMismatch,
    #[error("processor binding references a missing formula input: {0:?}")]
    MissingInput(golden_alchemist::SurfaceItemId),
    #[error("processor binding references a missing formula output: {0:?}")]
    MissingOutput(golden_alchemist::SurfaceItemId),
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error(transparent)]
    Formula(#[from] golden_alchemist::CompileError),
}
