use std::sync::Arc;

use golden_values::{ColorValue, ExtensionValue, TriggerValue, Value, ValueTypeId};
use thiserror::Error;

use super::{CssValue, ParamValue};

const FILE_VALUE_TYPE: &str = "golden.parameter.file";
const ENUM_VALUE_TYPE: &str = "golden.parameter.enum";
const CSS_VALUE_TYPE: &str = "golden.parameter.css-value";
const NODE_REFERENCE_VALUE_TYPE: &str = "golden.parameter.node-reference";

/// Failure to convert between a parameter value and the canonical runtime value model.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CanonicalValueError {
    /// A canonical integer cannot be represented by the current parameter integer storage.
    #[error("canonical integer `{0}` is outside the parameter i32 range")]
    IntegerOutOfRange(i64),
    /// A canonical value has no parameter representation.
    #[error("canonical value type `{0}` has no parameter representation")]
    UnsupportedValueType(ValueTypeId),
    /// A trigger without a fired edge cannot be materialized as a parameter pulse.
    #[error("an unfired canonical trigger cannot be materialized as a parameter pulse")]
    UnfiredTrigger,
    /// Extension payload bytes are not valid UTF-8.
    #[error("canonical extension `{value_type}` contains invalid UTF-8: {message}")]
    InvalidUtf8 {
        /// Extension value type.
        value_type: ValueTypeId,
        /// Decoder error.
        message: String,
    },
    /// An extension payload does not match its declared parameter schema.
    #[error("canonical extension `{value_type}` is invalid: {message}")]
    InvalidExtension {
        /// Extension value type.
        value_type: ValueTypeId,
        /// Decoder error.
        message: String,
    },
}

impl TryFrom<&ParamValue> for Value {
    type Error = CanonicalValueError;

    fn try_from(value: &ParamValue) -> Result<Self, Self::Error> {
        Ok(match value {
            ParamValue::Trigger() => Value::Trigger(TriggerValue::fired(0, 0)),
            ParamValue::Int(value) => Value::Int(i64::from(*value)),
            ParamValue::Float(value) => Value::Float(*value),
            ParamValue::Str(value) => Value::String(Arc::from(value.as_str())),
            ParamValue::File(value) => extension(FILE_VALUE_TYPE, value.as_bytes()),
            ParamValue::Enum(value) => extension(ENUM_VALUE_TYPE, value.as_bytes()),
            ParamValue::Bool(value) => Value::Bool(*value),
            ParamValue::CssValue(value) => extension_json(CSS_VALUE_TYPE, value)?,
            ParamValue::Vec2(x, y) => Value::Vec2([*x, *y]),
            ParamValue::Vec3(x, y, z) => Value::Vec3([*x, *y, *z]),
            ParamValue::Color(red, green, blue, alpha) => Value::Color(ColorValue {
                red: *red,
                green: *green,
                blue: *blue,
                alpha: *alpha,
            }),
            ParamValue::Reference(reference) => extension_json(NODE_REFERENCE_VALUE_TYPE, reference)?,
        })
    }
}

impl TryFrom<&Value> for ParamValue {
    type Error = CanonicalValueError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(value) => Ok(ParamValue::Bool(*value)),
            Value::Trigger(value) if value.fired => Ok(ParamValue::Trigger()),
            Value::Trigger(_) => Err(CanonicalValueError::UnfiredTrigger),
            Value::Int(value) => i32::try_from(*value)
                .map(ParamValue::Int)
                .map_err(|_| CanonicalValueError::IntegerOutOfRange(*value)),
            Value::Float(value) => Ok(ParamValue::Float(*value)),
            Value::String(value) => Ok(ParamValue::Str(value.to_string())),
            Value::Vec2(value) => Ok(ParamValue::Vec2(value[0], value[1])),
            Value::Vec3(value) => Ok(ParamValue::Vec3(value[0], value[1], value[2])),
            Value::Color(value) => Ok(ParamValue::Color(
                f64::from(value.red),
                f64::from(value.green),
                f64::from(value.blue),
                f64::from(value.alpha),
            )),
            Value::Extension(value) => parameter_from_extension(value),
            unsupported => Err(CanonicalValueError::UnsupportedValueType(unsupported.value_type())),
        }
    }
}

fn extension(value_type: &str, payload: &[u8]) -> Value {
    Value::Extension(ExtensionValue::new(
        ValueTypeId::new(value_type),
        Arc::<[u8]>::from(payload),
    ))
}

fn extension_json<T: serde::Serialize>(value_type: &str, value: &T) -> Result<Value, CanonicalValueError> {
    let value_type = ValueTypeId::new(value_type);
    let payload = serde_json::to_vec(value).map_err(|error| CanonicalValueError::InvalidExtension {
        value_type: value_type.clone(),
        message: error.to_string(),
    })?;
    Ok(Value::Extension(ExtensionValue::new(value_type, payload)))
}

fn parameter_from_extension(value: &ExtensionValue) -> Result<ParamValue, CanonicalValueError> {
    match value.value_type.as_str() {
        FILE_VALUE_TYPE => extension_text(value).map(ParamValue::File),
        ENUM_VALUE_TYPE => extension_text(value).map(ParamValue::Enum),
        CSS_VALUE_TYPE => extension_json_value::<CssValue>(value).map(ParamValue::CssValue),
        NODE_REFERENCE_VALUE_TYPE => extension_json_value(value).map(ParamValue::Reference),
        _ => Err(CanonicalValueError::UnsupportedValueType(value.value_type.clone())),
    }
}

fn extension_text(value: &ExtensionValue) -> Result<String, CanonicalValueError> {
    std::str::from_utf8(&value.payload)
        .map(str::to_owned)
        .map_err(|error| CanonicalValueError::InvalidUtf8 {
            value_type: value.value_type.clone(),
            message: error.to_string(),
        })
}

fn extension_json_value<T: serde::de::DeserializeOwned>(value: &ExtensionValue) -> Result<T, CanonicalValueError> {
    serde_json::from_slice(&value.payload).map_err(|error| CanonicalValueError::InvalidExtension {
        value_type: value.value_type.clone(),
        message: error.to_string(),
    })
}
