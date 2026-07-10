use std::{collections::BTreeMap, sync::Arc};

use golden_alchemist::{CompiledFormulaKernel, EvaluationOptions, FormulaInstance, FormulaRuntimeError, SurfaceItemId};
use golden_condition::{ConditionInputId, ConditionRuntimeError};
use golden_context::ContextKey;
use golden_values::Value;
use thiserror::Error;

use crate::ProcessorPlan;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProcessorBatchReport {
    pub lanes: usize,
    pub condition_rejected: usize,
    pub executed_operations: usize,
    pub outputs_changed: usize,
}

pub struct ProcessorRuntime {
    kernel: Arc<CompiledFormulaKernel>,
    condition: Option<Arc<golden_condition::ConditionProgram>>,
    lane_keys: Vec<ContextKey>,
    instances: Vec<FormulaInstance>,
}

impl ProcessorRuntime {
    pub fn new(plan: ProcessorPlan) -> Self {
        let instances = (0..plan.lane_layout.lane_count())
            .map(|_| FormulaInstance::new(Arc::clone(&plan.kernel)))
            .collect();
        Self {
            kernel: plan.kernel,
            condition: plan.definition.condition,
            lane_keys: plan.lane_keys,
            instances,
        }
    }

    pub fn kernel(&self) -> &Arc<CompiledFormulaKernel> {
        &self.kernel
    }

    pub fn lane_keys(&self) -> &[ContextKey] {
        &self.lane_keys
    }

    pub fn set_input(
        &mut self,
        lane: usize,
        input: SurfaceItemId,
        value: Value,
    ) -> Result<bool, ProcessorRuntimeError> {
        self.instances
            .get_mut(lane)
            .ok_or(ProcessorRuntimeError::MissingLane(lane))?
            .set_input(input, value)
            .map_err(Into::into)
    }

    pub fn output(&self, lane: usize, output: SurfaceItemId) -> Result<&Value, ProcessorRuntimeError> {
        self.instances
            .get(lane)
            .ok_or(ProcessorRuntimeError::MissingLane(lane))?
            .output(output)
            .map_err(Into::into)
    }

    pub fn evaluate(
        &mut self,
        condition_inputs: &[BTreeMap<ConditionInputId, Value>],
        options: EvaluationOptions,
    ) -> Result<ProcessorBatchReport, ProcessorRuntimeError> {
        if self.condition.is_some() && condition_inputs.len() != self.instances.len() {
            return Err(ProcessorRuntimeError::ConditionLaneCount {
                expected: self.instances.len(),
                actual: condition_inputs.len(),
            });
        }
        let mut report = ProcessorBatchReport {
            lanes: self.instances.len(),
            ..Default::default()
        };
        for (lane, instance) in self.instances.iter_mut().enumerate() {
            if let Some(condition) = &self.condition
                && !condition.evaluate(&condition_inputs[lane])?
            {
                report.condition_rejected += 1;
                continue;
            }
            let evaluation = instance.evaluate(options)?;
            report.executed_operations += evaluation.executed_operations;
            report.outputs_changed += usize::from(evaluation.outputs_changed);
        }
        Ok(report)
    }
}

#[derive(Debug, Error)]
pub enum ProcessorRuntimeError {
    #[error("processor lane does not exist: {0}")]
    MissingLane(usize),
    #[error("condition input lane count mismatch: expected {expected}, received {actual}")]
    ConditionLaneCount { expected: usize, actual: usize },
    #[error(transparent)]
    Formula(#[from] FormulaRuntimeError),
    #[error(transparent)]
    Condition(#[from] ConditionRuntimeError),
}
