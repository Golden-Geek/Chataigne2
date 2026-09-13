use crate::{ANodeInstance, ColorValue, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, value_to_f64};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MathOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

impl MathOperator {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "operator", "add").as_str() {
            "subtract" => Self::Subtract,
            "multiply" => Self::Multiply,
            "divide" => Self::Divide,
            "modulo" => Self::Modulo,
            _ => Self::Add,
        }
    }
}

#[derive(Debug)]
pub(super) struct MathEval {
    pub(super) operator: MathOperator,
}

impl CompiledNodeEvaluator for MathEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        fold_numeric_inputs(evaluation.inputs, self.operator).map(|value| vec![value])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReductionMode {
    Sum,
    Average,
}

#[derive(Debug)]
pub(super) struct ReductionEval {
    pub(super) mode: ReductionMode,
}

impl CompiledNodeEvaluator for ReductionEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let value = match self.mode {
            ReductionMode::Sum => fold_numeric_inputs(evaluation.inputs, MathOperator::Add)?,
            ReductionMode::Average => {
                if evaluation.inputs.is_empty() {
                    return Err("Average expects at least one input".into());
                }
                let sum = evaluation.inputs.iter().try_fold(0.0, |sum, input| match input {
                    RuntimeValue::Float(value) => Ok(sum + value),
                    _ => Err("Average requires float inputs".to_string()),
                })?;
                RuntimeValue::Float(sum / evaluation.inputs.len() as f64)
            }
        };
        Ok(vec![value])
    }
}

fn fold_numeric_inputs(inputs: &[RuntimeValue], operator: MathOperator) -> Result<RuntimeValue, String> {
    let Some((first, rest)) = inputs.split_first() else {
        return Err("Math expects at least one input".into());
    };
    let mut value = first.clone();
    for next in rest {
        value = numeric_binary(&value, next, operator)?;
    }
    Ok(value)
}

fn numeric_binary(left: &RuntimeValue, right: &RuntimeValue, operator: MathOperator) -> Result<RuntimeValue, String> {
    match (left, right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => {
            return match operator {
                MathOperator::Add => Ok(RuntimeValue::Int(left + right)),
                MathOperator::Subtract => Ok(RuntimeValue::Int(left - right)),
                MathOperator::Multiply => Ok(RuntimeValue::Int(left * right)),
                MathOperator::Divide => {
                    if *right == 0 {
                        Err("Math divide input cannot be zero".into())
                    } else {
                        Ok(RuntimeValue::Float(*left as f64 / *right as f64))
                    }
                }
                MathOperator::Modulo => {
                    if *right == 0 {
                        Err("Math modulo input cannot be zero".into())
                    } else {
                        Ok(RuntimeValue::Int(left % right))
                    }
                }
            };
        }
        (RuntimeValue::Vec2(left), RuntimeValue::Vec2(right)) => {
            return Ok(RuntimeValue::Vec2([
                numeric_scalar(left[0], right[0], operator)?,
                numeric_scalar(left[1], right[1], operator)?,
            ]));
        }
        (RuntimeValue::Vec3(left), RuntimeValue::Vec3(right)) => {
            return Ok(RuntimeValue::Vec3([
                numeric_scalar(left[0], right[0], operator)?,
                numeric_scalar(left[1], right[1], operator)?,
                numeric_scalar(left[2], right[2], operator)?,
            ]));
        }
        (RuntimeValue::Color(left), RuntimeValue::Color(right)) => {
            return Ok(RuntimeValue::Color(ColorValue {
                red: numeric_scalar(left.red, right.red, operator)?,
                green: numeric_scalar(left.green, right.green, operator)?,
                blue: numeric_scalar(left.blue, right.blue, operator)?,
                alpha: numeric_scalar(left.alpha, right.alpha, operator)?,
            }));
        }
        _ => {}
    }
    Ok(RuntimeValue::Float(numeric_scalar(
        value_to_f64(left),
        value_to_f64(right),
        operator,
    )?))
}

fn numeric_scalar(left: f64, right: f64, operator: MathOperator) -> Result<f64, String> {
    match operator {
        MathOperator::Add => Ok(left + right),
        MathOperator::Subtract => Ok(left - right),
        MathOperator::Multiply => Ok(left * right),
        MathOperator::Divide => {
            if right.abs() <= f64::EPSILON {
                Err("Math divide input cannot be zero".into())
            } else {
                Ok(left / right)
            }
        }
        MathOperator::Modulo => {
            if right.abs() <= f64::EPSILON {
                Err("Math modulo input cannot be zero".into())
            } else {
                Ok(left % right)
            }
        }
    }
}
