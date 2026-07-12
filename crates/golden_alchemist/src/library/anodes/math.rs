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
        let Some((first, rest)) = evaluation.inputs.split_first() else {
            return Err("Math expects at least one input".into());
        };
        let mut value = first.clone();
        for next in rest {
            value = numeric_binary(&value, next, self.operator)?;
        }
        Ok(vec![value])
    }
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
                red: numeric_scalar(f64::from(left.red), f64::from(right.red), operator)? as f32,
                green: numeric_scalar(f64::from(left.green), f64::from(right.green), operator)? as f32,
                blue: numeric_scalar(f64::from(left.blue), f64::from(right.blue), operator)? as f32,
                alpha: numeric_scalar(f64::from(left.alpha), f64::from(right.alpha), operator)? as f32,
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
