use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fmt;
use ts_rs::TS;

use golden_model::NodeUuid;

use super::{Color, CssUnit, CssValue, Enum, File, NodeReference, Vec2, Vec3};

/// Runtime value variants used by parameter nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub enum ParamValue {
    /// Trigger-like pulse with no payload.
    Trigger(),

    /// Signed integer value.
    Int(i32),
    /// Floating-point value.
    Float(f64),
    /// UTF-8 string value.
    Str(String),
    /// File path value.
    File(String),
    /// Enum variant identifier.
    Enum(String),
    /// Boolean value.
    Bool(bool),
    /// CSS scalar value with explicit unit.
    CssValue(CssValue),

    /// 2D vector value.
    Vec2(f64, f64),
    /// 3D vector value.
    Vec3(f64, f64, f64),
    /// RGBA color value.
    Color(f64, f64, f64, f64),

    /// Reference to another node.
    Reference(NodeReference),
}

/// Comparison operator used by macro-generated parameter dependency predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterDependencyOperator {
    /// Equality comparison.
    Eq,
    /// Inequality comparison.
    Ne,
    /// Strictly less-than comparison.
    Lt,
    /// Less-than-or-equal comparison.
    Le,
    /// Strictly greater-than comparison.
    Gt,
    /// Greater-than-or-equal comparison.
    Ge,
}

/// Returns whether a value should be treated as enabled/active for dependency predicates.
pub fn dependency_truthy(value: &ParamValue) -> bool {
    match value {
        ParamValue::Trigger() => true,
        ParamValue::Int(value) => *value != 0,
        ParamValue::Float(value) => value.abs() > f64::EPSILON,
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => !value.is_empty(),
        ParamValue::Bool(value) => *value,
        ParamValue::CssValue(value) => value.value.abs() > f64::EPSILON,
        ParamValue::Vec2(x, y) => x.abs() > f64::EPSILON || y.abs() > f64::EPSILON,
        ParamValue::Vec3(x, y, z) => x.abs() > f64::EPSILON || y.abs() > f64::EPSILON || z.abs() > f64::EPSILON,
        ParamValue::Color(r, g, b, a) => {
            r.abs() > f64::EPSILON || g.abs() > f64::EPSILON || b.abs() > f64::EPSILON || a.abs() > f64::EPSILON
        }
        ParamValue::Reference(reference) => !reference.is_empty(),
    }
}

/// Compares two runtime values for a parameter dependency predicate.
pub fn dependency_binary_compare(lhs: &ParamValue, rhs: &ParamValue, operator: ParameterDependencyOperator) -> bool {
    if matches!(lhs, ParamValue::CssValue(_)) || matches!(rhs, ParamValue::CssValue(_)) {
        match (lhs, rhs) {
            (ParamValue::CssValue(lhs), ParamValue::CssValue(rhs)) if lhs.unit == rhs.unit => {
                return compare_partial_ord(lhs.value, rhs.value, operator);
            }
            (ParamValue::CssValue(lhs), other) => {
                if let Some(rhs) = other.as_float() {
                    return compare_partial_ord(lhs.value, rhs, operator);
                }
            }
            (other, ParamValue::CssValue(rhs)) => {
                if let Some(lhs) = other.as_float() {
                    return compare_partial_ord(lhs, rhs.value, operator);
                }
            }
            _ => {}
        }

        if matches!(
            operator,
            ParameterDependencyOperator::Eq | ParameterDependencyOperator::Ne
        ) && let (Some(lhs), Some(rhs)) = (lhs.as_str(), rhs.as_str())
        {
            return compare_partial_ord(lhs.as_str(), rhs.as_str(), operator);
        }

        return false;
    }

    if (matches!(lhs, ParamValue::Int(_) | ParamValue::Float(_))
        || matches!(rhs, ParamValue::Int(_) | ParamValue::Float(_)))
        && let (Some(lhs), Some(rhs)) = (lhs.as_float(), rhs.as_float())
    {
        return compare_partial_ord(lhs, rhs, operator);
    }

    if (matches!(lhs, ParamValue::Bool(_)) || matches!(rhs, ParamValue::Bool(_)))
        && let (Some(lhs), Some(rhs)) = (lhs.as_bool(), rhs.as_bool())
    {
        return compare_partial_ord(lhs, rhs, operator);
    }

    if (matches!(lhs, ParamValue::Str(_) | ParamValue::File(_) | ParamValue::Enum(_))
        || matches!(rhs, ParamValue::Str(_) | ParamValue::File(_) | ParamValue::Enum(_)))
        && let (Some(lhs), Some(rhs)) = (lhs.as_str(), rhs.as_str())
    {
        return compare_partial_ord(lhs.as_str(), rhs.as_str(), operator);
    }

    match operator {
        ParameterDependencyOperator::Eq => lhs == rhs,
        ParameterDependencyOperator::Ne => lhs != rhs,
        ParameterDependencyOperator::Lt
        | ParameterDependencyOperator::Le
        | ParameterDependencyOperator::Gt
        | ParameterDependencyOperator::Ge => false,
    }
}

fn compare_partial_ord<T>(lhs: T, rhs: T, operator: ParameterDependencyOperator) -> bool
where
    T: PartialEq + PartialOrd,
{
    match operator {
        ParameterDependencyOperator::Eq => lhs == rhs,
        ParameterDependencyOperator::Ne => lhs != rhs,
        ParameterDependencyOperator::Lt => lhs < rhs,
        ParameterDependencyOperator::Le => lhs <= rhs,
        ParameterDependencyOperator::Gt => lhs > rhs,
        ParameterDependencyOperator::Ge => lhs >= rhs,
    }
}

impl fmt::Display for ParamValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trigger() => write!(f, "trigger"),
            Self::Int(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
            Self::Str(value) => write!(f, "\"{value}\""),
            Self::File(path) => write!(f, "file:{path}"),
            Self::Enum(variant) => write!(f, "enum:{variant}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::CssValue(value) => write!(f, "{value}"),
            Self::Vec2(x, y) => write!(f, "[{x}, {y}]"),
            Self::Vec3(x, y, z) => write!(f, "[{x}, {y}, {z}]"),
            Self::Color(r, g, b, a) => write!(f, "[{r}, {g}, {b}, {a}]"),
            Self::Reference(reference) => {
                if let Some(name) = reference.cached_name() {
                    write!(f, "ref:{name} ({})", reference.uuid().0)
                } else {
                    write!(f, "ref:{}", reference.uuid().0)
                }
            }
        }
    }
}

impl From<i32> for ParamValue {
    fn from(value: i32) -> Self {
        ParamValue::Int(value)
    }
}

impl From<f64> for ParamValue {
    fn from(value: f64) -> Self {
        ParamValue::Float(value)
    }
}

impl From<String> for ParamValue {
    fn from(value: String) -> Self {
        ParamValue::Str(value)
    }
}

impl From<&str> for ParamValue {
    fn from(value: &str) -> Self {
        ParamValue::Str(value.to_string())
    }
}

impl From<File> for ParamValue {
    fn from(value: File) -> Self {
        ParamValue::File(value.into_inner())
    }
}

impl From<std::path::PathBuf> for ParamValue {
    fn from(value: std::path::PathBuf) -> Self {
        ParamValue::File(value.to_string_lossy().to_string())
    }
}

impl From<bool> for ParamValue {
    fn from(value: bool) -> Self {
        ParamValue::Bool(value)
    }
}

impl From<(f64, CssUnit)> for ParamValue {
    fn from(value: (f64, CssUnit)) -> Self {
        ParamValue::CssValue(CssValue::from(value))
    }
}

impl From<CssValue> for ParamValue {
    fn from(value: CssValue) -> Self {
        ParamValue::CssValue(value)
    }
}

impl From<(f64, f64)> for ParamValue {
    fn from(value: (f64, f64)) -> Self {
        ParamValue::Vec2(value.0, value.1)
    }
}

impl From<(f64, f64, f64)> for ParamValue {
    fn from(value: (f64, f64, f64)) -> Self {
        ParamValue::Vec3(value.0, value.1, value.2)
    }
}

impl From<(f64, f64, f64, f64)> for ParamValue {
    fn from(value: (f64, f64, f64, f64)) -> Self {
        ParamValue::Color(value.0, value.1, value.2, value.3)
    }
}

impl From<Vec2> for ParamValue {
    fn from(value: Vec2) -> Self {
        ParamValue::Vec2(value.x, value.y)
    }
}

impl From<Vec3> for ParamValue {
    fn from(value: Vec3) -> Self {
        ParamValue::Vec3(value.x, value.y, value.z)
    }
}

impl From<Color> for ParamValue {
    fn from(value: Color) -> Self {
        ParamValue::Color(value.r(), value.g(), value.b(), value.a())
    }
}

impl From<Enum> for ParamValue {
    fn from(value: Enum) -> Self {
        ParamValue::Enum(value.into_inner())
    }
}

impl From<NodeReference> for ParamValue {
    fn from(value: NodeReference) -> Self {
        ParamValue::Reference(value)
    }
}

impl From<NodeUuid> for ParamValue {
    fn from(value: NodeUuid) -> Self {
        ParamValue::Reference(NodeReference::new(value))
    }
}

impl ParamValue {
    /// Returns whether every floating-point component can cross JSON and UI protocol boundaries.
    ///
    /// Alchemist and other compute domains may use NaN or infinity internally, but authored
    /// parameter state is serialized as JSON and therefore accepts only finite numeric values.
    pub fn has_only_finite_numbers(&self) -> bool {
        match self {
            ParamValue::Float(value) => value.is_finite(),
            ParamValue::CssValue(value) => value.value.is_finite(),
            ParamValue::Vec2(x, y) => x.is_finite() && y.is_finite(),
            ParamValue::Vec3(x, y, z) => x.is_finite() && y.is_finite() && z.is_finite(),
            ParamValue::Color(r, g, b, a) => r.is_finite() && g.is_finite() && b.is_finite() && a.is_finite(),
            ParamValue::Trigger()
            | ParamValue::Int(_)
            | ParamValue::Str(_)
            | ParamValue::File(_)
            | ParamValue::Enum(_)
            | ParamValue::Bool(_)
            | ParamValue::Reference(_) => true,
        }
    }

    /// Coerces this value into an integer, when possible.
    pub fn as_int(&self) -> Option<i32> {
        match self {
            ParamValue::Int(i) => Some(*i),
            ParamValue::Float(f) => Some(*f as i32),
            ParamValue::Str(s) | ParamValue::Enum(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(if *b { 1 } else { 0 }),
            ParamValue::CssValue(value) => Some(value.value as i32),
            _ => None,
        }
    }

    /// Coerces this value into a floating-point value, when possible.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            ParamValue::Int(i) => Some(*i as f64),
            ParamValue::Float(f) => Some(*f),
            ParamValue::Str(s) | ParamValue::Enum(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            ParamValue::CssValue(value) => Some(value.value),
            _ => None,
        }
    }

    /// Coerces this value into a string, when possible.
    pub fn as_str(&self) -> Option<String> {
        match self {
            ParamValue::Trigger() => Some("trigger".to_string()),
            ParamValue::Int(i) => Some(i.to_string()),
            ParamValue::Float(f) => Some(f.to_string()),
            ParamValue::Str(s) | ParamValue::File(s) | ParamValue::Enum(s) => Some(s.clone()),
            ParamValue::Bool(b) => Some(b.to_string()),
            ParamValue::CssValue(value) => Some(value.to_css_string()),
            ParamValue::Vec2(x, y) => Some(format!("{x},{y}")),
            ParamValue::Vec3(x, y, z) => Some(format!("{x},{y},{z}")),
            ParamValue::Color(r, g, b, a) => Some(format!("{r},{g},{b},{a}")),
            ParamValue::Reference(reference) => Some(
                reference
                    .cached_name()
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| reference.uuid().0.to_string()),
            ),
        }
    }

    /// Coerces this value into an enum variant id, when possible.
    pub fn as_enum(&self) -> Option<String> {
        match self {
            ParamValue::Enum(variant_id) => Some(variant_id.clone()),
            ParamValue::Str(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// Coerces this value into a boolean, when possible.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Int(i) => Some(*i != 0),
            ParamValue::Float(f) => Some(*f != 0.0),
            ParamValue::Str(s) | ParamValue::Enum(s) => s.parse().ok(),
            ParamValue::Bool(b) => Some(*b),
            ParamValue::CssValue(value) => Some(value.value != 0.0),
            _ => None,
        }
    }

    /// Coerces this value into a CSS value, when possible.
    pub fn as_css_value(&self) -> Option<CssValue> {
        self.as_css_value_with_unit(CssUnit::Rem)
    }

    /// Coerces this value into a CSS value using `default_unit` for unitless sources.
    pub fn as_css_value_with_unit(&self, default_unit: CssUnit) -> Option<CssValue> {
        match self {
            ParamValue::Int(i) => Some(CssValue::new(*i as f64, default_unit)),
            ParamValue::Float(f) => Some(CssValue::new(*f, default_unit)),
            ParamValue::Str(s) | ParamValue::Enum(s) | ParamValue::File(s) => {
                CssValue::parse_with_default_unit(s, Some(default_unit))
            }
            ParamValue::Bool(b) => Some(CssValue::new(if *b { 1.0 } else { 0.0 }, default_unit)),
            ParamValue::CssValue(value) => Some(*value),
            _ => None,
        }
    }

    /// Coerces this value into a 2D vector, when possible.
    pub fn as_vec2(&self) -> Option<(f64, f64)> {
        match self {
            ParamValue::Int(i) => Some((*i as f64, *i as f64)),
            ParamValue::Float(f) => Some((*f, *f)),
            ParamValue::Str(s) | ParamValue::Enum(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 2
                    && let (Ok(x), Ok(y)) = (parts[0].trim().parse(), parts[1].trim().parse())
                {
                    return Some((x, y));
                }
                None
            }
            ParamValue::Vec2(x, y) => Some((*x, *y)),
            _ => None,
        }
    }

    /// Coerces this value into a 3D vector, when possible.
    pub fn as_vec3(&self) -> Option<(f64, f64, f64)> {
        match self {
            ParamValue::Int(i) => Some((*i as f64, *i as f64, *i as f64)),
            ParamValue::Float(f) => Some((*f, *f, *f)),
            ParamValue::Str(s) | ParamValue::Enum(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 3
                    && let (Ok(x), Ok(y), Ok(z)) = (
                        parts[0].trim().parse(),
                        parts[1].trim().parse(),
                        parts[2].trim().parse(),
                    )
                {
                    return Some((x, y, z));
                }
                None
            }
            ParamValue::Vec3(x, y, z) => Some((*x, *y, *z)),
            ParamValue::Color(r, g, b, _) => Some((*r, *g, *b)),
            _ => None,
        }
    }

    /// Coerces this value into an RGBA color, when possible.
    pub fn as_color(&self) -> Option<(f64, f64, f64, f64)> {
        match self {
            ParamValue::Int(i) => Some(((*i as f64 / 255.0), (*i as f64 / 255.0), (*i as f64 / 255.0), 1.0)),
            ParamValue::Float(f) => Some((*f, *f, *f, 1.0)),
            ParamValue::Str(s) | ParamValue::Enum(s) => {
                let parts: Vec<&str> = s.split(',').collect();
                if parts.len() == 4
                    && let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
                        parts[0].trim().parse(),
                        parts[1].trim().parse(),
                        parts[2].trim().parse(),
                        parts[3].trim().parse(),
                    )
                {
                    return Some((r, g, b, a));
                }
                None
            }
            ParamValue::Color(r, g, b, a) => Some((*r, *g, *b, *a)),
            _ => None,
        }
    }

    /// Decodes one script JSON value into a runtime [`ParamValue`].
    pub fn from_script_json(value: &JsonValue) -> Result<Self, String> {
        if let Ok(decoded) = serde_json::from_value::<ParamValue>(value.clone()) {
            return Ok(decoded);
        }

        match value {
            JsonValue::Null => Ok(ParamValue::Trigger()),
            JsonValue::Bool(value) => Ok(ParamValue::Bool(*value)),
            JsonValue::Number(number) => {
                if let Some(value) = number.as_i64()
                    && let Ok(value) = i32::try_from(value)
                {
                    return Ok(ParamValue::Int(value));
                }
                if let Some(value) = number.as_f64() {
                    return Ok(ParamValue::Float(value));
                }
                Err("numeric value cannot be represented as int/float".to_string())
            }
            JsonValue::String(value) => Ok(ParamValue::Str(value.clone())),
            JsonValue::Array(values) => {
                if values.len() == 2 {
                    let x = values[0]
                        .as_f64()
                        .ok_or_else(|| "vec2 value expects numeric components".to_string())?;
                    let y = values[1]
                        .as_f64()
                        .ok_or_else(|| "vec2 value expects numeric components".to_string())?;
                    return Ok(ParamValue::Vec2(x, y));
                }
                if values.len() == 3 {
                    let x = values[0]
                        .as_f64()
                        .ok_or_else(|| "vec3 value expects numeric components".to_string())?;
                    let y = values[1]
                        .as_f64()
                        .ok_or_else(|| "vec3 value expects numeric components".to_string())?;
                    let z = values[2]
                        .as_f64()
                        .ok_or_else(|| "vec3 value expects numeric components".to_string())?;
                    return Ok(ParamValue::Vec3(x, y, z));
                }
                if values.len() == 4 {
                    let r = values[0]
                        .as_f64()
                        .ok_or_else(|| "color value expects numeric components".to_string())?;
                    let g = values[1]
                        .as_f64()
                        .ok_or_else(|| "color value expects numeric components".to_string())?;
                    let b = values[2]
                        .as_f64()
                        .ok_or_else(|| "color value expects numeric components".to_string())?;
                    let a = values[3]
                        .as_f64()
                        .ok_or_else(|| "color value expects numeric components".to_string())?;
                    return Ok(ParamValue::Color(r, g, b, a));
                }
                Err("array value must contain 2, 3, or 4 numeric components".to_string())
            }
            JsonValue::Object(_) => Err("object value cannot be coerced to ParamValue".to_string()),
        }
    }

    /// Encodes this runtime value into script-facing JSON.
    pub fn to_script_json(&self) -> JsonValue {
        match self {
            ParamValue::Trigger() => JsonValue::Null,
            ParamValue::Int(value) => serde_json::json!(value),
            ParamValue::Float(value) => serde_json::json!(value),
            ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => serde_json::json!(value),
            ParamValue::Bool(value) => serde_json::json!(value),
            ParamValue::CssValue(value) => serde_json::json!(value.to_css_string()),
            ParamValue::Vec2(x, y) => serde_json::json!([x, y]),
            ParamValue::Vec3(x, y, z) => serde_json::json!([x, y, z]),
            ParamValue::Color(r, g, b, a) => serde_json::json!([r, g, b, a]),
            ParamValue::Reference(reference) => serde_json::json!({
                "kind": "reference",
                "uuid": reference.uuid().0.to_string(),
                "cachedId": reference.cached_id().map(|node| node.0),
                "cachedName": reference.cached_name(),
                "relativePathFromRoot": reference.relative_path_from_root(),
                "projection": reference.projection().map(|projection| projection.variant_id()),
            }),
        }
    }
}
