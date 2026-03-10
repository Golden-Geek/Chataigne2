use std::path::PathBuf;

use crate::color::Color;
use crate::node::{NodeReference, NodeUuid};

use super::{CssUnit, CssValue, Enum, File, ParamValue, Vec2, Vec3};

/// Typed conversion contract between Rust values and [`ParamValue`].
pub trait ParameterValueType: Clone {
    /// Converts a typed value into its runtime parameter representation.
    fn to_param_value(value: Self) -> ParamValue;

    /// Attempts to decode a runtime parameter value to this type.
    fn from_param_value(value: &ParamValue) -> Option<Self>;
}

impl ParameterValueType for i32 {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Int(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_int()
    }
}

impl ParameterValueType for f64 {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Float(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_float()
    }
}

impl ParameterValueType for String {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Str(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_str()
    }
}

impl ParameterValueType for File {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::File(value.into_inner())
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        match value {
            ParamValue::File(path) | ParamValue::Str(path) => Some(File::new(path.clone())),
            _ => None,
        }
    }
}

impl ParameterValueType for PathBuf {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::File(value.to_string_lossy().to_string())
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        match value {
            ParamValue::File(path) | ParamValue::Str(path) => Some(PathBuf::from(path)),
            _ => None,
        }
    }
}

impl ParameterValueType for Enum {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Enum(value.into_inner())
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_enum().map(Enum::new)
    }
}

impl ParameterValueType for bool {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Bool(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_bool()
    }
}

impl ParameterValueType for (f64, CssUnit) {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::CssValue(CssValue::from(value))
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_css_value().map(Into::into)
    }
}

impl ParameterValueType for CssValue {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::CssValue(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_css_value()
    }
}

impl ParameterValueType for (f64, f64) {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Vec2(value.0, value.1)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_vec2()
    }
}

impl ParameterValueType for Vec2 {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Vec2(value.x, value.y)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_vec2().map(|(x, y)| Vec2::new(x, y))
    }
}

impl ParameterValueType for (f64, f64, f64) {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Vec3(value.0, value.1, value.2)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_vec3()
    }
}

impl ParameterValueType for Vec3 {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Vec3(value.x, value.y, value.z)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_vec3().map(|(x, y, z)| Vec3::new(x, y, z))
    }
}

impl ParameterValueType for (f64, f64, f64, f64) {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Color(value.0, value.1, value.2, value.3)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_color()
    }
}

impl ParameterValueType for Color {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Color(value.r(), value.g(), value.b(), value.a())
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        value.as_color().map(|(r, g, b, a)| Color::new(r, g, b, a))
    }
}

impl ParameterValueType for NodeReference {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Reference(value)
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        if let ParamValue::Reference(reference) = value {
            Some(reference.clone())
        } else {
            None
        }
    }
}

impl ParameterValueType for NodeUuid {
    fn to_param_value(value: Self) -> ParamValue {
        ParamValue::Reference(NodeReference::new(value))
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        if let ParamValue::Reference(reference) = value {
            Some(reference.uuid())
        } else {
            None
        }
    }
}

impl ParameterValueType for ParamValue {
    fn to_param_value(value: Self) -> ParamValue {
        value
    }

    fn from_param_value(value: &ParamValue) -> Option<Self> {
        Some(value.clone())
    }
}