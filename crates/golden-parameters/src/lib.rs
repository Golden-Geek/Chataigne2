//! Authoring and control semantics layered on canonical Golden values.

use std::collections::BTreeMap;

use golden_model::{EntityId, Revision};
use golden_values::{FiniteF64, StableValueRef, Value};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    Numeric {
        minimum: Option<FiniteF64>,
        maximum: Option<FiniteF64>,
    },
    Length {
        minimum: usize,
        maximum: Option<usize>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ControlMode {
    Static,
    Reference(StableValueRef),
    ContextPath(Vec<SmolStr>),
    Expression(String),
    Script(String),
    Automation(EntityId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChangeBehavior {
    Always,
    IfChanged,
    Coalesce,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterDeclaration {
    pub id: EntityId,
    pub value_type: SmolStr,
    pub default: Value,
    pub constraints: Vec<Constraint>,
    pub ui_hints: BTreeMap<SmolStr, SmolStr>,
}

impl ParameterDeclaration {
    pub fn validate(&self, value: &Value) -> Result<(), ParameterError> {
        if value.type_id() != self.value_type.as_str() {
            return Err(ParameterError::TypeMismatch {
                expected: self.value_type.clone(),
                actual: value.type_id(),
            });
        }

        for constraint in &self.constraints {
            validate_constraint(constraint, value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterState {
    pub declaration: EntityId,
    pub control: ControlMode,
    pub value: Value,
    pub revision: Revision,
}

impl ParameterState {
    pub fn apply(&mut self, value: Value, behavior: ChangeBehavior) -> Result<bool, ParameterError> {
        if behavior != ChangeBehavior::Always && self.value == value {
            return Ok(false);
        }
        self.revision = self.revision.next().ok_or(ParameterError::RevisionExhausted)?;
        self.value = value;
        Ok(true)
    }
}

fn validate_constraint(constraint: &Constraint, value: &Value) -> Result<(), ParameterError> {
    match (constraint, value) {
        (Constraint::Numeric { minimum, maximum }, Value::Float(value))
            if minimum.is_some_and(|minimum| value < &minimum) || maximum.is_some_and(|maximum| value > &maximum) =>
        {
            Err(ParameterError::ConstraintViolation)
        }
        (Constraint::Length { minimum, maximum }, Value::String(value))
            if value.chars().count() < *minimum || maximum.is_some_and(|maximum| value.chars().count() > maximum) =>
        {
            Err(ParameterError::ConstraintViolation)
        }
        _ => Ok(()),
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ParameterError {
    #[error("expected {expected}, received {actual}")]
    TypeMismatch { expected: SmolStr, actual: &'static str },
    #[error("parameter value violates its declaration constraints")]
    ConstraintViolation,
    #[error("parameter revision space is exhausted")]
    RevisionExhausted,
}

#[cfg(test)]
mod tests;
