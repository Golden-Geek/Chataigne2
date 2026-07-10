//! Authored predicates compiled once into a compact stack program.

use std::collections::BTreeMap;

use golden_values::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConditionInputId(pub SmolStr);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Comparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConditionExpr {
    Literal(bool),
    Truthy(ConditionInputId),
    Compare {
        input: ConditionInputId,
        comparison: Comparison,
        expected: Value,
    },
    Not(Box<ConditionExpr>),
    All(Vec<ConditionExpr>),
    Any(Vec<ConditionExpr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConditionOp {
    Push(bool),
    Truthy(ConditionInputId),
    Compare {
        input: ConditionInputId,
        comparison: Comparison,
        expected: Value,
    },
    Not,
    All(u32),
    Any(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConditionProgram {
    operations: Vec<ConditionOp>,
    maximum_stack: usize,
}

impl ConditionProgram {
    pub fn operations(&self) -> &[ConditionOp] {
        &self.operations
    }

    pub fn evaluate(&self, inputs: &BTreeMap<ConditionInputId, Value>) -> Result<bool, ConditionRuntimeError> {
        let mut stack = Vec::with_capacity(self.maximum_stack);
        for operation in &self.operations {
            match operation {
                ConditionOp::Push(value) => stack.push(*value),
                ConditionOp::Truthy(input) => stack.push(truthy(value(inputs, input)?)),
                ConditionOp::Compare {
                    input,
                    comparison,
                    expected,
                } => stack.push(compare(value(inputs, input)?, expected, *comparison)?),
                ConditionOp::Not => {
                    let value = stack.pop().ok_or(ConditionRuntimeError::InvalidProgram)?;
                    stack.push(!value);
                }
                ConditionOp::All(count) => reduce(&mut stack, *count, true, |a, b| a && b)?,
                ConditionOp::Any(count) => reduce(&mut stack, *count, false, |a, b| a || b)?,
            }
        }
        match stack.as_slice() {
            [result] => Ok(*result),
            _ => Err(ConditionRuntimeError::InvalidProgram),
        }
    }
}

#[derive(Default)]
pub struct ConditionCompiler;

impl ConditionCompiler {
    pub fn compile(&self, expression: &ConditionExpr) -> Result<ConditionProgram, ConditionCompileError> {
        let mut operations = Vec::new();
        let mut depth = 0_usize;
        let mut maximum_stack = 0_usize;
        emit(expression, &mut operations, &mut depth, &mut maximum_stack)?;
        Ok(ConditionProgram {
            operations,
            maximum_stack,
        })
    }
}

fn emit(
    expression: &ConditionExpr,
    operations: &mut Vec<ConditionOp>,
    depth: &mut usize,
    maximum_stack: &mut usize,
) -> Result<(), ConditionCompileError> {
    match expression {
        ConditionExpr::Literal(value) => operations.push(ConditionOp::Push(*value)),
        ConditionExpr::Truthy(input) => operations.push(ConditionOp::Truthy(input.clone())),
        ConditionExpr::Compare {
            input,
            comparison,
            expected,
        } => operations.push(ConditionOp::Compare {
            input: input.clone(),
            comparison: *comparison,
            expected: expected.clone(),
        }),
        ConditionExpr::Not(inner) => {
            emit(inner, operations, depth, maximum_stack)?;
            operations.push(ConditionOp::Not);
            return Ok(());
        }
        ConditionExpr::All(expressions) | ConditionExpr::Any(expressions) => {
            if expressions.is_empty() {
                operations.push(ConditionOp::Push(matches!(expression, ConditionExpr::All(_))));
            } else {
                for expression in expressions {
                    emit(expression, operations, depth, maximum_stack)?;
                }
                let count = u32::try_from(expressions.len()).map_err(|_| ConditionCompileError::TooManyTerms)?;
                *depth -= expressions.len() - 1;
                operations.push(if matches!(expression, ConditionExpr::All(_)) {
                    ConditionOp::All(count)
                } else {
                    ConditionOp::Any(count)
                });
                return Ok(());
            }
        }
    }
    *depth = depth.checked_add(1).ok_or(ConditionCompileError::TooManyTerms)?;
    *maximum_stack = (*maximum_stack).max(*depth);
    Ok(())
}

fn value<'a>(
    inputs: &'a BTreeMap<ConditionInputId, Value>,
    input: &ConditionInputId,
) -> Result<&'a Value, ConditionRuntimeError> {
    inputs
        .get(input)
        .ok_or_else(|| ConditionRuntimeError::MissingInput(input.clone()))
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Integer(value) => *value != 0,
        Value::Float(value) => value.get() != 0.0,
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

fn compare(left: &Value, right: &Value, comparison: Comparison) -> Result<bool, ConditionRuntimeError> {
    if matches!(comparison, Comparison::Equal | Comparison::NotEqual) {
        let equal = left == right;
        return Ok(if comparison == Comparison::Equal { equal } else { !equal });
    }
    let (left, right) = match (left, right) {
        (Value::Integer(left), Value::Integer(right)) => (*left as f64, *right as f64),
        (Value::Float(left), Value::Float(right)) => (left.get(), right.get()),
        (Value::Integer(left), Value::Float(right)) => (*left as f64, right.get()),
        (Value::Float(left), Value::Integer(right)) => (left.get(), *right as f64),
        _ => return Err(ConditionRuntimeError::NotOrderable),
    };
    Ok(match comparison {
        Comparison::Less => left < right,
        Comparison::LessOrEqual => left <= right,
        Comparison::Greater => left > right,
        Comparison::GreaterOrEqual => left >= right,
        Comparison::Equal | Comparison::NotEqual => unreachable!(),
    })
}

fn reduce(
    stack: &mut Vec<bool>,
    count: u32,
    identity: bool,
    operation: impl Fn(bool, bool) -> bool,
) -> Result<(), ConditionRuntimeError> {
    let count = count as usize;
    if count > stack.len() {
        return Err(ConditionRuntimeError::InvalidProgram);
    }
    let start = stack.len() - count;
    let result = stack.drain(start..).fold(identity, operation);
    stack.push(result);
    Ok(())
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ConditionCompileError {
    #[error("condition contains too many terms")]
    TooManyTerms,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ConditionRuntimeError {
    #[error("condition input is missing: {0:?}")]
    MissingInput(ConditionInputId),
    #[error("condition operands are not orderable")]
    NotOrderable,
    #[error("compiled condition program is invalid")]
    InvalidProgram,
}

#[cfg(test)]
mod tests;
