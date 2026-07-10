//! Canonical authored and runtime-facing value semantics.

use std::collections::BTreeMap;

use golden_model::EntityId;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct FiniteF64(f64);

impl FiniteF64 {
    pub fn new(value: f64) -> Result<Self, ValueError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(ValueError::NonFiniteNumber)
        }
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FiniteF64 {
    type Error = ValueError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FiniteF64> for f64 {
    fn from(value: FiniteF64) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ValueTypeId(SmolStr);

impl ValueTypeId {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, ValueError> {
        let value = value.into();
        if value.is_empty() {
            Err(ValueError::EmptyTypeId)
        } else {
            Ok(Self(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TriggerEdgeId(pub u64);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LaneKey(pub SmolStr);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProjectionSegment {
    Field(SmolStr),
    Index(u32),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StableValueRef {
    pub entity: EntityId,
    pub projection: Vec<ProjectionSegment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    Integer(i64),
    Float(FiniteF64),
    String(String),
    File(String),
    Enum(SmolStr),
    Css(String),
    Vec2([FiniteF64; 2]),
    Vec3([FiniteF64; 3]),
    Color([FiniteF64; 4]),
    DurationNanos(u64),
    Trigger(TriggerEdgeId),
    Array(Vec<Value>),
    Reference(StableValueRef),
    Extension {
        type_id: ValueTypeId,
        payload: serde_json::Value,
    },
}

impl Value {
    pub fn type_id(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::File(_) => "file",
            Self::Enum(_) => "enum",
            Self::Css(_) => "css",
            Self::Vec2(_) => "vec2",
            Self::Vec3(_) => "vec3",
            Self::Color(_) => "color",
            Self::DurationNanos(_) => "duration",
            Self::Trigger(_) => "trigger",
            Self::Array(_) => "array",
            Self::Reference(_) => "reference",
            Self::Extension { .. } => "extension",
        }
    }

    pub fn convert_to(&self, target: &str) -> Result<Self, ValueError> {
        match (self, target) {
            (value, target) if value.type_id() == target => Ok(value.clone()),
            (Self::Integer(value), "float") => Ok(Self::Float(FiniteF64::new(*value as f64)?)),
            (Self::Float(value), "integer") => {
                let number = value.get();
                if number.fract() == 0.0 && number >= i64::MIN as f64 && number <= i64::MAX as f64 {
                    Ok(Self::Integer(number as i64))
                } else {
                    Err(ValueError::LossyConversion {
                        from: "float",
                        to: "integer",
                    })
                }
            }
            _ => Err(ValueError::UnsupportedConversion {
                from: self.type_id(),
                to: target.to_owned(),
            }),
        }
    }
}

pub type ValueSet = BTreeMap<LaneKey, Value>;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ValueError {
    #[error("numeric values must be finite")]
    NonFiniteNumber,
    #[error("value type identifiers cannot be empty")]
    EmptyTypeId,
    #[error("conversion from {from} to {to} would lose information")]
    LossyConversion { from: &'static str, to: &'static str },
    #[error("conversion from {from} to {to} is not defined")]
    UnsupportedConversion { from: &'static str, to: String },
}

#[cfg(test)]
mod tests;
