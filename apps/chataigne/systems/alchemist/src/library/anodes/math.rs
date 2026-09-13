use crate::{ANodeInstance, ColorValue, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::config_string;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MathOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Minimum,
    Maximum,
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
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        fold_numeric_inputs(evaluation.inputs, self.operator).map(|value| crate::node_outputs![value])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReductionMode {
    Sum,
    Average,
    Product,
    Minimum,
    Maximum,
    Difference,
    Distance,
}

#[derive(Debug)]
pub(super) struct ReductionEval {
    pub(super) mode: ReductionMode,
}

impl CompiledNodeEvaluator for ReductionEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let value = match self.mode {
            ReductionMode::Sum => fold_numeric_inputs(evaluation.inputs, MathOperator::Add)?,
            ReductionMode::Product => fold_numeric_inputs(evaluation.inputs, MathOperator::Multiply)?,
            ReductionMode::Minimum => fold_numeric_inputs(evaluation.inputs, MathOperator::Minimum)?,
            ReductionMode::Maximum => fold_numeric_inputs(evaluation.inputs, MathOperator::Maximum)?,
            ReductionMode::Difference => fold_numeric_inputs(evaluation.inputs, MathOperator::Subtract)?,
            ReductionMode::Distance => {
                let [RuntimeValue::Float(left), RuntimeValue::Float(right)] = evaluation.inputs else {
                    return Err("Distance requires exactly two float inputs".into());
                };
                RuntimeValue::Float(finite_scalar((left - right).abs())?)
            }
            ReductionMode::Average => {
                if evaluation.inputs.is_empty() {
                    return Err("Average expects at least one input".into());
                }
                let sum = evaluation.inputs.iter().try_fold(0.0, |sum, input| match input {
                    RuntimeValue::Float(value) => finite_scalar(sum + value),
                    _ => Err("Average requires float inputs".to_string()),
                })?;
                RuntimeValue::Float(finite_scalar(sum / evaluation.inputs.len() as f64)?)
            }
        };
        Ok(crate::node_outputs![value])
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
                MathOperator::Add => left.checked_add(*right),
                MathOperator::Subtract => left.checked_sub(*right),
                MathOperator::Multiply => left.checked_mul(*right),
                MathOperator::Divide if *right == 0 => return Err("Math divide input cannot be zero".into()),
                MathOperator::Divide => left.checked_div(*right),
                MathOperator::Modulo if *right == 0 => return Err("Math modulo input cannot be zero".into()),
                MathOperator::Modulo => left.checked_rem(*right),
                MathOperator::Minimum => Some((*left).min(*right)),
                MathOperator::Maximum => Some((*left).max(*right)),
            }
            .map(RuntimeValue::Int)
            .ok_or_else(|| "Math integer overflow".into());
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
    match (left, right) {
        (RuntimeValue::Float(left), RuntimeValue::Float(right)) => {
            Ok(RuntimeValue::Float(numeric_scalar(*left, *right, operator)?))
        }
        _ => Err("Math requires matching numeric inputs".into()),
    }
}

fn numeric_scalar(left: f64, right: f64, operator: MathOperator) -> Result<f64, String> {
    if !left.is_finite() || !right.is_finite() {
        return Err("Math requires finite inputs".into());
    }
    let result = match operator {
        MathOperator::Add => left + right,
        MathOperator::Subtract => left - right,
        MathOperator::Multiply => left * right,
        MathOperator::Divide => {
            if right == 0.0 {
                return Err("Math divide input cannot be zero".into());
            }
            left / right
        }
        MathOperator::Modulo => {
            if right == 0.0 {
                return Err("Math modulo input cannot be zero".into());
            }
            left % right
        }
        MathOperator::Minimum => left.min(right),
        MathOperator::Maximum => left.max(right),
    };
    finite_scalar(result)
}

fn finite_scalar(value: f64) -> Result<f64, String> {
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| "Math result is non-finite".into())
}
