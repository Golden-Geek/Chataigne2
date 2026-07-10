use std::sync::Arc;

use golden_alchemist::{FormulaId, SurfaceItemId};
use golden_condition::ConditionProgram;
use golden_model::EntityId;
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProcessorId(pub EntityId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessorKind {
    Action,
    Mapping,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessorSurfaceBinding {
    pub source: SmolStr,
    pub target: SurfaceItemId,
}

#[derive(Clone)]
pub struct MappingSpec {
    pub formula: FormulaId,
    pub inputs: Vec<ProcessorSurfaceBinding>,
    pub outputs: Vec<SurfaceItemId>,
    pub condition: Option<Arc<ConditionProgram>>,
}

#[derive(Clone)]
pub struct ProcessorDefinition {
    pub id: ProcessorId,
    pub kind: ProcessorKind,
    pub formula: FormulaId,
    pub inputs: Vec<ProcessorSurfaceBinding>,
    pub outputs: Vec<SurfaceItemId>,
    pub condition: Option<Arc<ConditionProgram>>,
}

impl ProcessorDefinition {
    pub fn mapping(id: ProcessorId, spec: MappingSpec) -> Result<Self, ProcessorDefinitionError> {
        if spec.inputs.is_empty() {
            return Err(ProcessorDefinitionError::MissingInputs);
        }
        if spec.outputs.is_empty() {
            return Err(ProcessorDefinitionError::MissingOutputs);
        }
        Ok(Self {
            id,
            kind: ProcessorKind::Mapping,
            formula: spec.formula,
            inputs: spec.inputs,
            outputs: spec.outputs,
            condition: spec.condition,
        })
    }

    pub fn action(
        id: ProcessorId,
        formula: FormulaId,
        inputs: Vec<ProcessorSurfaceBinding>,
        outputs: Vec<SurfaceItemId>,
    ) -> Self {
        Self {
            id,
            kind: ProcessorKind::Action,
            formula,
            inputs,
            outputs,
            condition: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ProcessorDefinitionError {
    #[error("mapping requires at least one input")]
    MissingInputs,
    #[error("mapping requires at least one output")]
    MissingOutputs,
}
