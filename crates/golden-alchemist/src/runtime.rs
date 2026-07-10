use std::sync::Arc;

use golden_values::{FiniteF64, Value};
use thiserror::Error;

use crate::{CompiledFormulaKernel, CompiledOp, ExecNodeId, SurfaceItemId, ValueSlot};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EvaluationOptions {
    pub capture_observation: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObservationSample {
    pub node: ExecNodeId,
    pub output: ValueSlot,
    pub value: Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EvaluationReport {
    pub executed_operations: usize,
    pub outputs_changed: bool,
    pub observation: Vec<ObservationSample>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BatchEvaluationReport {
    pub instances: usize,
    pub executed_operations: usize,
    pub outputs_changed: usize,
}

pub struct FormulaInstance {
    kernel: Arc<CompiledFormulaKernel>,
    slots: Vec<Value>,
    dirty: Vec<bool>,
    first_evaluation: bool,
}

impl FormulaInstance {
    pub fn new(kernel: Arc<CompiledFormulaKernel>) -> Self {
        let slots = kernel.slot_defaults.clone();
        Self {
            dirty: vec![false; slots.len()],
            kernel,
            slots,
            first_evaluation: true,
        }
    }

    pub fn kernel(&self) -> &Arc<CompiledFormulaKernel> {
        &self.kernel
    }

    pub fn set_input(&mut self, input: SurfaceItemId, value: Value) -> Result<bool, FormulaRuntimeError> {
        let slot = *self
            .kernel
            .surface_inputs
            .get(&input)
            .ok_or(FormulaRuntimeError::MissingInput(input))?;
        let index = slot.0 as usize;
        if self.slots[index] == value {
            return Ok(false);
        }
        self.slots[index] = value;
        self.dirty[index] = true;
        Ok(true)
    }

    pub fn output(&self, output: SurfaceItemId) -> Result<&Value, FormulaRuntimeError> {
        let slot = self
            .kernel
            .surface_outputs
            .get(&output)
            .ok_or(FormulaRuntimeError::MissingOutput(output))?;
        Ok(&self.slots[slot.0 as usize])
    }

    pub fn evaluate(&mut self, options: EvaluationOptions) -> Result<EvaluationReport, FormulaRuntimeError> {
        if !self.first_evaluation && !self.kernel.time_dependent && !self.dirty.iter().any(|dirty| *dirty) {
            return Ok(EvaluationReport::default());
        }
        let mut report = EvaluationReport::default();
        for operation in &self.kernel.operations {
            let should_run =
                self.first_evaluation || self.kernel.time_dependent || operation_is_dirty(operation, &self.dirty);
            if !should_run {
                continue;
            }
            let output = operation.output();
            let next = evaluate_operation(operation, &self.slots)?;
            let changed = self.slots[output.0 as usize] != next;
            if changed {
                self.slots[output.0 as usize] = next;
                self.dirty[output.0 as usize] = true;
                report.outputs_changed |= self.kernel.output_slots.contains(&output);
            }
            report.executed_operations += 1;
            if options.capture_observation {
                report.observation.push(ObservationSample {
                    node: operation.node(),
                    output,
                    value: self.slots[output.0 as usize].clone(),
                });
            }
        }
        self.first_evaluation = false;
        self.dirty.fill(false);
        Ok(report)
    }
}

pub fn evaluate_batch(
    instances: &mut [FormulaInstance],
    options: EvaluationOptions,
) -> Result<BatchEvaluationReport, FormulaRuntimeError> {
    let mut report = BatchEvaluationReport {
        instances: instances.len(),
        ..Default::default()
    };
    for instance in instances {
        let instance_report = instance.evaluate(options)?;
        report.executed_operations += instance_report.executed_operations;
        report.outputs_changed += usize::from(instance_report.outputs_changed);
    }
    Ok(report)
}

fn operation_is_dirty(operation: &CompiledOp, dirty: &[bool]) -> bool {
    match operation {
        CompiledOp::Constant { .. } => false,
        CompiledOp::AddFloat { left, right, .. } | CompiledOp::MultiplyFloat { left, right, .. } => {
            dirty[left.0 as usize] || dirty[right.0 as usize]
        }
        CompiledOp::PassThrough { input, .. } => dirty[input.0 as usize],
        CompiledOp::ConditionGate { condition, value, .. } => dirty[condition.0 as usize] || dirty[value.0 as usize],
    }
}

fn evaluate_operation(operation: &CompiledOp, slots: &[Value]) -> Result<Value, FormulaRuntimeError> {
    match operation {
        CompiledOp::Constant { value, .. } => Ok(value.clone()),
        CompiledOp::AddFloat { left, right, .. } => Ok(Value::Float(FiniteF64::new(
            float(slots, *left)? + float(slots, *right)?,
        )?)),
        CompiledOp::MultiplyFloat { left, right, .. } => Ok(Value::Float(FiniteF64::new(
            float(slots, *left)? * float(slots, *right)?,
        )?)),
        CompiledOp::PassThrough { input, .. } => Ok(slots[input.0 as usize].clone()),
        CompiledOp::ConditionGate { condition, value, .. } => match slots.get(condition.0 as usize) {
            Some(Value::Bool(true)) => Ok(slots[value.0 as usize].clone()),
            Some(Value::Bool(false)) => Ok(default_for(
                slots
                    .get(value.0 as usize)
                    .ok_or(FormulaRuntimeError::MissingSlot(*value))?,
            )),
            Some(actual) => Err(FormulaRuntimeError::TypeMismatch {
                expected: "bool",
                actual: actual.type_id(),
            }),
            None => Err(FormulaRuntimeError::MissingSlot(*condition)),
        },
    }
}

fn default_for(value: &Value) -> Value {
    match value {
        Value::Bool(_) => Value::Bool(false),
        Value::Integer(_) => Value::Integer(0),
        Value::Float(_) => Value::Float(FiniteF64::new(0.0).expect("zero is finite")),
        Value::String(_) => Value::String(String::new()),
        other => other.clone(),
    }
}

fn float(slots: &[Value], slot: ValueSlot) -> Result<f64, FormulaRuntimeError> {
    match slots.get(slot.0 as usize) {
        Some(Value::Float(value)) => Ok(value.get()),
        Some(value) => Err(FormulaRuntimeError::TypeMismatch {
            expected: "float",
            actual: value.type_id(),
        }),
        None => Err(FormulaRuntimeError::MissingSlot(slot)),
    }
}

#[derive(Debug, Error)]
pub enum FormulaRuntimeError {
    #[error("formula surface input does not exist: {0:?}")]
    MissingInput(SurfaceItemId),
    #[error("formula surface output does not exist: {0:?}")]
    MissingOutput(SurfaceItemId),
    #[error("formula value slot does not exist: {0:?}")]
    MissingSlot(ValueSlot),
    #[error("expected {expected}, received {actual}")]
    TypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    #[error(transparent)]
    InvalidNumber(#[from] golden_values::ValueError),
}
