use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fmt;
use std::path::{Path, PathBuf};
use ts_rs::TS;

use crate::node::{NodeReference, NodeUuid};

pub use crate::color::Color;

#[path = "parameter/control.rs"]
mod control;
#[path = "parameter/constraints.rs"]
mod constraints;
#[path = "parameter/node.rs"]
mod parameter_node;

pub use control::*;
pub use constraints::*;
pub use parameter_node::Parameter;

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
        ) {
            if let (Some(lhs), Some(rhs)) = (lhs.as_str(), rhs.as_str()) {
                return compare_partial_ord(lhs.as_str(), rhs.as_str(), operator);
            }
        }

        return false;
    }

    if matches!(lhs, ParamValue::Int(_) | ParamValue::Float(_))
        || matches!(rhs, ParamValue::Int(_) | ParamValue::Float(_))
    {
        if let (Some(lhs), Some(rhs)) = (lhs.as_float(), rhs.as_float()) {
            return compare_partial_ord(lhs, rhs, operator);
        }
    }

    if matches!(lhs, ParamValue::Bool(_)) || matches!(rhs, ParamValue::Bool(_)) {
        if let (Some(lhs), Some(rhs)) = (lhs.as_bool(), rhs.as_bool()) {
            return compare_partial_ord(lhs, rhs, operator);
        }
    }

    if matches!(lhs, ParamValue::Str(_) | ParamValue::File(_) | ParamValue::Enum(_))
        || matches!(rhs, ParamValue::Str(_) | ParamValue::File(_) | ParamValue::Enum(_))
    {
        if let (Some(lhs), Some(rhs)) = (lhs.as_str(), rhs.as_str()) {
            return compare_partial_ord(lhs.as_str(), rhs.as_str(), operator);
        }
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

/// Explicit source projection used before coercion.
///
/// Projections are intentionally explicit so non-trivial coercions
/// (component picks, reshapes, color-space mappings) stay user-controlled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ParamValueProjection {
    /// Expand one float to `(v, 0)`.
    FloatToVec2X0,
    /// Expand one float to `(0, v)`.
    FloatToVec20Y,
    /// Expand one float to `(v, v)`.
    FloatToVec2XX,
    /// Expand one float to `(v, 0, 0)`.
    FloatToVec3X00,
    /// Expand one float to `(0, v, 0)`.
    FloatToVec30Y0,
    /// Expand one float to `(0, 0, v)`.
    FloatToVec300Z,
    /// Expand one float to `(v, v, v)`.
    FloatToVec3XXX,
    /// Select `x` from a `vec2`.
    Vec2X,
    /// Select `y` from a `vec2`.
    Vec2Y,
    /// Lift one `vec2` to `(x, y, 0)`.
    Vec2ToVec3XY0,
    /// Lift one `vec2` to `(x, 0, y)`.
    Vec2ToVec3X0Y,
    /// Interpret one `vec2` as `(hue, sat)` and map to RGBA (value=1, alpha=1).
    Vec2ToColorHs,
    /// Select `x` from a `vec3`.
    Vec3X,
    /// Select `y` from a `vec3`.
    Vec3Y,
    /// Select `z` from a `vec3`.
    Vec3Z,
    /// Collapse one `vec3` to `(x, y)`.
    Vec3ToVec2XY,
    /// Collapse one `vec3` to `(x, z)`.
    Vec3ToVec2XZ,
    /// Collapse one `vec3` to `(y, z)`.
    Vec3ToVec2YZ,
    /// Interpret one `vec3` as RGB and map to RGBA (alpha=1).
    Vec3ToColorRgb,
    /// Interpret one `vec3` as HSV and map to RGBA (alpha=1).
    Vec3ToColorHsv,
    /// Select `r` from a `color`.
    ColorR,
    /// Select `g` from a `color`.
    ColorG,
    /// Select `b` from a `color`.
    ColorB,
    /// Select `a` from a `color`.
    ColorA,
    /// Convert one color to `vec3` RGB.
    ColorToVec3Rgb,
    /// Convert one color to `vec3` HSV.
    ColorToVec3Hsv,
    /// Convert one color to `vec2` `(hue, sat)`.
    ColorToVec2Hs,
}

impl ParamValueProjection {
    /// Returns all projections valid for `source`.
    pub fn available_for_source(source: &ParamValue) -> Vec<Self> {
        match source {
            ParamValue::Float(_) => vec![
                Self::FloatToVec2X0,
                Self::FloatToVec20Y,
                Self::FloatToVec2XX,
                Self::FloatToVec3X00,
                Self::FloatToVec30Y0,
                Self::FloatToVec300Z,
                Self::FloatToVec3XXX,
            ],
            ParamValue::Vec2(_, _) => vec![
                Self::Vec2X,
                Self::Vec2Y,
                Self::Vec2ToVec3XY0,
                Self::Vec2ToVec3X0Y,
                Self::Vec2ToColorHs,
            ],
            ParamValue::Vec3(_, _, _) => vec![
                Self::Vec3X,
                Self::Vec3Y,
                Self::Vec3Z,
                Self::Vec3ToVec2XY,
                Self::Vec3ToVec2XZ,
                Self::Vec3ToVec2YZ,
                Self::Vec3ToColorRgb,
                Self::Vec3ToColorHsv,
            ],
            ParamValue::Color(_, _, _, _) => {
                vec![
                    Self::ColorR,
                    Self::ColorG,
                    Self::ColorB,
                    Self::ColorA,
                    Self::ColorToVec3Rgb,
                    Self::ColorToVec3Hsv,
                    Self::ColorToVec2Hs,
                ]
            }
            _ => Vec::new(),
        }
    }

    /// Stable id used by enum-style UI controls.
    pub fn variant_id(self) -> &'static str {
        match self {
            Self::FloatToVec2X0 => "floatToVec2X0",
            Self::FloatToVec20Y => "floatToVec20Y",
            Self::FloatToVec2XX => "floatToVec2XX",
            Self::FloatToVec3X00 => "floatToVec3X00",
            Self::FloatToVec30Y0 => "floatToVec30Y0",
            Self::FloatToVec300Z => "floatToVec300Z",
            Self::FloatToVec3XXX => "floatToVec3XXX",
            Self::Vec2X => "vec2X",
            Self::Vec2Y => "vec2Y",
            Self::Vec2ToVec3XY0 => "vec2ToVec3XY0",
            Self::Vec2ToVec3X0Y => "vec2ToVec3X0Y",
            Self::Vec2ToColorHs => "vec2ToColorHs",
            Self::Vec3X => "vec3X",
            Self::Vec3Y => "vec3Y",
            Self::Vec3Z => "vec3Z",
            Self::Vec3ToVec2XY => "vec3ToVec2XY",
            Self::Vec3ToVec2XZ => "vec3ToVec2XZ",
            Self::Vec3ToVec2YZ => "vec3ToVec2YZ",
            Self::Vec3ToColorRgb => "vec3ToColorRgb",
            Self::Vec3ToColorHsv => "vec3ToColorHsv",
            Self::ColorR => "colorR",
            Self::ColorG => "colorG",
            Self::ColorB => "colorB",
            Self::ColorA => "colorA",
            Self::ColorToVec3Rgb => "colorToVec3Rgb",
            Self::ColorToVec3Hsv => "colorToVec3Hsv",
            Self::ColorToVec2Hs => "colorToVec2Hs",
        }
    }

    /// Parses one projection id.
    pub fn from_variant_id(value: &str) -> Option<Self> {
        match value.trim() {
            "floatToVec2X0" => Some(Self::FloatToVec2X0),
            "floatToVec20Y" => Some(Self::FloatToVec20Y),
            "floatToVec2XX" => Some(Self::FloatToVec2XX),
            "floatToVec3X00" => Some(Self::FloatToVec3X00),
            "floatToVec30Y0" => Some(Self::FloatToVec30Y0),
            "floatToVec300Z" => Some(Self::FloatToVec300Z),
            "floatToVec3XXX" => Some(Self::FloatToVec3XXX),
            "vec2X" => Some(Self::Vec2X),
            "vec2Y" => Some(Self::Vec2Y),
            "vec2ToVec3XY0" => Some(Self::Vec2ToVec3XY0),
            "vec2ToVec3X0Y" => Some(Self::Vec2ToVec3X0Y),
            "vec2ToColorHs" => Some(Self::Vec2ToColorHs),
            "vec3X" => Some(Self::Vec3X),
            "vec3Y" => Some(Self::Vec3Y),
            "vec3Z" => Some(Self::Vec3Z),
            "vec3ToVec2XY" => Some(Self::Vec3ToVec2XY),
            "vec3ToVec2XZ" => Some(Self::Vec3ToVec2XZ),
            "vec3ToVec2YZ" => Some(Self::Vec3ToVec2YZ),
            "vec3ToColorRgb" => Some(Self::Vec3ToColorRgb),
            "vec3ToColorHsv" => Some(Self::Vec3ToColorHsv),
            "colorR" => Some(Self::ColorR),
            "colorG" => Some(Self::ColorG),
            "colorB" => Some(Self::ColorB),
            "colorA" => Some(Self::ColorA),
            "colorToVec3Rgb" => Some(Self::ColorToVec3Rgb),
            "colorToVec3Hsv" => Some(Self::ColorToVec3Hsv),
            "colorToVec2Hs" => Some(Self::ColorToVec2Hs),
            _ => None,
        }
    }
}

/// Compatibility report between one source value and one target value kind.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamValueCompatibility {
    /// `true` when direct coercion works without projection.
    pub direct: bool,
    /// Projections that make coercion possible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projections: Vec<ParamValueProjection>,
}

impl ParamValueCompatibility {
    /// Returns `true` when either direct or projected coercion works.
    pub fn is_compatible(&self) -> bool {
        self.direct || !self.projections.is_empty()
    }
}

/// Returns one default value prototype for a parameter type id.
pub fn default_param_value_for_type_id(type_id: &str) -> Option<ParamValue> {
    match type_id.trim().to_ascii_lowercase().as_str() {
        "trigger" => Some(ParamValue::Trigger()),
        "int" => Some(ParamValue::Int(0)),
        "float" => Some(ParamValue::Float(0.0)),
        "str" => Some(ParamValue::Str(String::new())),
        "file" => Some(ParamValue::File(String::new())),
        "enum" => Some(ParamValue::Enum(String::new())),
        "bool" => Some(ParamValue::Bool(false)),
        "css_value" | "css-value" => Some(ParamValue::CssValue(CssValue::default())),
        "vec2" => Some(ParamValue::Vec2(0.0, 0.0)),
        "vec3" => Some(ParamValue::Vec3(0.0, 0.0, 0.0)),
        "color" => Some(ParamValue::Color(0.0, 0.0, 0.0, 1.0)),
        "reference" => Some(ParamValue::Reference(NodeReference::default())),
        _ => None,
    }
}

fn rgb_to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;

    let mut hue = if delta <= f64::EPSILON {
        0.0
    } else if (max - r).abs() <= f64::EPSILON {
        ((g - b) / delta).rem_euclid(6.0)
    } else if (max - g).abs() <= f64::EPSILON {
        ((b - r) / delta) + 2.0
    } else {
        ((r - g) / delta) + 4.0
    };
    hue /= 6.0;

    let sat = if max <= f64::EPSILON { 0.0 } else { delta / max };
    (hue, sat, max)
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    if s <= f64::EPSILON {
        return (v, v, v);
    }

    let hh = h.rem_euclid(1.0) * 6.0;
    let sector = hh.floor() as i32;
    let frac = hh - sector as f64;

    let p = v * (1.0 - s);
    let q = v * (1.0 - s * frac);
    let t = v * (1.0 - s * (1.0 - frac));

    match sector.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// Applies one explicit projection to `source`.
pub fn project_param_value(source: &ParamValue, projection: ParamValueProjection) -> Option<ParamValue> {
    let projected = match (source, projection) {
        (ParamValue::Float(value), ParamValueProjection::FloatToVec2X0) => ParamValue::Vec2(*value, 0.0),
        (ParamValue::Float(value), ParamValueProjection::FloatToVec20Y) => ParamValue::Vec2(0.0, *value),
        (ParamValue::Float(value), ParamValueProjection::FloatToVec2XX) => ParamValue::Vec2(*value, *value),
        (ParamValue::Float(value), ParamValueProjection::FloatToVec3X00) => ParamValue::Vec3(*value, 0.0, 0.0),
        (ParamValue::Float(value), ParamValueProjection::FloatToVec30Y0) => ParamValue::Vec3(0.0, *value, 0.0),
        (ParamValue::Float(value), ParamValueProjection::FloatToVec300Z) => ParamValue::Vec3(0.0, 0.0, *value),
        (ParamValue::Float(value), ParamValueProjection::FloatToVec3XXX) => ParamValue::Vec3(*value, *value, *value),
        (ParamValue::Vec2(x, _), ParamValueProjection::Vec2X) => ParamValue::Float(*x),
        (ParamValue::Vec2(_, y), ParamValueProjection::Vec2Y) => ParamValue::Float(*y),
        (ParamValue::Vec2(x, y), ParamValueProjection::Vec2ToVec3XY0) => ParamValue::Vec3(*x, *y, 0.0),
        (ParamValue::Vec2(x, y), ParamValueProjection::Vec2ToVec3X0Y) => ParamValue::Vec3(*x, 0.0, *y),
        (ParamValue::Vec2(h, s), ParamValueProjection::Vec2ToColorHs) => {
            let (r, g, b) = hsv_to_rgb(*h, *s, 1.0);
            ParamValue::Color(r, g, b, 1.0)
        }
        (ParamValue::Vec3(x, _, _), ParamValueProjection::Vec3X) => ParamValue::Float(*x),
        (ParamValue::Vec3(_, y, _), ParamValueProjection::Vec3Y) => ParamValue::Float(*y),
        (ParamValue::Vec3(_, _, z), ParamValueProjection::Vec3Z) => ParamValue::Float(*z),
        (ParamValue::Vec3(x, y, _), ParamValueProjection::Vec3ToVec2XY) => ParamValue::Vec2(*x, *y),
        (ParamValue::Vec3(x, _, z), ParamValueProjection::Vec3ToVec2XZ) => ParamValue::Vec2(*x, *z),
        (ParamValue::Vec3(_, y, z), ParamValueProjection::Vec3ToVec2YZ) => ParamValue::Vec2(*y, *z),
        (ParamValue::Vec3(r, g, b), ParamValueProjection::Vec3ToColorRgb) => ParamValue::Color(*r, *g, *b, 1.0),
        (ParamValue::Vec3(h, s, v), ParamValueProjection::Vec3ToColorHsv) => {
            let (r, g, b) = hsv_to_rgb(*h, *s, *v);
            ParamValue::Color(r, g, b, 1.0)
        }
        (ParamValue::Color(r, _, _, _), ParamValueProjection::ColorR) => ParamValue::Float(*r),
        (ParamValue::Color(_, g, _, _), ParamValueProjection::ColorG) => ParamValue::Float(*g),
        (ParamValue::Color(_, _, b, _), ParamValueProjection::ColorB) => ParamValue::Float(*b),
        (ParamValue::Color(_, _, _, a), ParamValueProjection::ColorA) => ParamValue::Float(*a),
        (ParamValue::Color(r, g, b, _), ParamValueProjection::ColorToVec3Rgb) => ParamValue::Vec3(*r, *g, *b),
        (ParamValue::Color(r, g, b, _), ParamValueProjection::ColorToVec3Hsv) => {
            let (h, s, v) = rgb_to_hsv(*r, *g, *b);
            ParamValue::Vec3(h, s, v)
        }
        (ParamValue::Color(r, g, b, _), ParamValueProjection::ColorToVec2Hs) => {
            let (h, s, _) = rgb_to_hsv(*r, *g, *b);
            ParamValue::Vec2(h, s)
        }
        _ => return None,
    };
    Some(projected)
}

fn project_param_value_for_target(
    source: &ParamValue,
    target: &ParamValue,
    projection: ParamValueProjection,
) -> Option<ParamValue> {
    let projected = match (source, target, projection) {
        (ParamValue::Float(value), ParamValue::Vec2(_, y), ParamValueProjection::FloatToVec2X0) => {
            ParamValue::Vec2(*value, *y)
        }
        (ParamValue::Float(value), ParamValue::Vec2(x, _), ParamValueProjection::FloatToVec20Y) => {
            ParamValue::Vec2(*x, *value)
        }
        (ParamValue::Float(value), ParamValue::Vec3(_, y, z), ParamValueProjection::FloatToVec3X00) => {
            ParamValue::Vec3(*value, *y, *z)
        }
        (ParamValue::Float(value), ParamValue::Vec3(x, _, z), ParamValueProjection::FloatToVec30Y0) => {
            ParamValue::Vec3(*x, *value, *z)
        }
        (ParamValue::Float(value), ParamValue::Vec3(x, y, _), ParamValueProjection::FloatToVec300Z) => {
            ParamValue::Vec3(*x, *y, *value)
        }
        (ParamValue::Vec2(x, y), ParamValue::Vec3(_, _, z), ParamValueProjection::Vec2ToVec3XY0) => {
            ParamValue::Vec3(*x, *y, *z)
        }
        (ParamValue::Vec2(x, z), ParamValue::Vec3(_, y, _), ParamValueProjection::Vec2ToVec3X0Y) => {
            ParamValue::Vec3(*x, *y, *z)
        }
        (
            ParamValue::Vec2(h, s),
            ParamValue::Color(target_r, target_g, target_b, target_a),
            ParamValueProjection::Vec2ToColorHs,
        ) => {
            let (_, _, value) = rgb_to_hsv(*target_r, *target_g, *target_b);
            let (r, g, b) = hsv_to_rgb(*h, *s, value);
            ParamValue::Color(r, g, b, *target_a)
        }
        (ParamValue::Vec3(r, g, b), ParamValue::Color(_, _, _, target_a), ParamValueProjection::Vec3ToColorRgb) => {
            ParamValue::Color(*r, *g, *b, *target_a)
        }
        (ParamValue::Vec3(h, s, v), ParamValue::Color(_, _, _, target_a), ParamValueProjection::Vec3ToColorHsv) => {
            let (r, g, b) = hsv_to_rgb(*h, *s, *v);
            ParamValue::Color(r, g, b, *target_a)
        }
        _ => return project_param_value(source, projection),
    };
    Some(projected)
}

fn project_param_value_for_target_reverse(
    source: &ParamValue,
    target: &ParamValue,
    projection: ParamValueProjection,
) -> Option<ParamValue> {
    let projected = match (source, target, projection) {
        (ParamValue::Vec2(x, _), ParamValue::Float(_), ParamValueProjection::FloatToVec2X0) => ParamValue::Float(*x),
        (ParamValue::Vec2(_, y), ParamValue::Float(_), ParamValueProjection::FloatToVec20Y) => ParamValue::Float(*y),
        (ParamValue::Vec2(x, y), ParamValue::Float(_), ParamValueProjection::FloatToVec2XX) => {
            ParamValue::Float((*x + *y) * 0.5)
        }
        (ParamValue::Vec3(x, _, _), ParamValue::Float(_), ParamValueProjection::FloatToVec3X00) => {
            ParamValue::Float(*x)
        }
        (ParamValue::Vec3(_, y, _), ParamValue::Float(_), ParamValueProjection::FloatToVec30Y0) => {
            ParamValue::Float(*y)
        }
        (ParamValue::Vec3(_, _, z), ParamValue::Float(_), ParamValueProjection::FloatToVec300Z) => {
            ParamValue::Float(*z)
        }
        (ParamValue::Vec3(x, y, z), ParamValue::Float(_), ParamValueProjection::FloatToVec3XXX) => {
            ParamValue::Float((*x + *y + *z) / 3.0)
        }
        (ParamValue::Float(value), ParamValue::Vec2(_, y), ParamValueProjection::Vec2X) => ParamValue::Vec2(*value, *y),
        (ParamValue::Float(value), ParamValue::Vec2(x, _), ParamValueProjection::Vec2Y) => ParamValue::Vec2(*x, *value),
        (ParamValue::Vec3(x, y, _), ParamValue::Vec2(_, _), ParamValueProjection::Vec2ToVec3XY0) => {
            ParamValue::Vec2(*x, *y)
        }
        (ParamValue::Vec3(x, _, z), ParamValue::Vec2(_, _), ParamValueProjection::Vec2ToVec3X0Y) => {
            ParamValue::Vec2(*x, *z)
        }
        (ParamValue::Color(r, g, b, _), ParamValue::Vec2(_, _), ParamValueProjection::Vec2ToColorHs) => {
            let (h, s, _) = rgb_to_hsv(*r, *g, *b);
            ParamValue::Vec2(h, s)
        }
        (ParamValue::Float(value), ParamValue::Vec3(_, y, z), ParamValueProjection::Vec3X) => {
            ParamValue::Vec3(*value, *y, *z)
        }
        (ParamValue::Float(value), ParamValue::Vec3(x, _, z), ParamValueProjection::Vec3Y) => {
            ParamValue::Vec3(*x, *value, *z)
        }
        (ParamValue::Float(value), ParamValue::Vec3(x, y, _), ParamValueProjection::Vec3Z) => {
            ParamValue::Vec3(*x, *y, *value)
        }
        (ParamValue::Vec2(x, y), ParamValue::Vec3(_, _, z), ParamValueProjection::Vec3ToVec2XY) => {
            ParamValue::Vec3(*x, *y, *z)
        }
        (ParamValue::Vec2(x, z), ParamValue::Vec3(_, y, _), ParamValueProjection::Vec3ToVec2XZ) => {
            ParamValue::Vec3(*x, *y, *z)
        }
        (ParamValue::Vec2(y, z), ParamValue::Vec3(x, _, _), ParamValueProjection::Vec3ToVec2YZ) => {
            ParamValue::Vec3(*x, *y, *z)
        }
        (ParamValue::Color(r, g, b, _), ParamValue::Vec3(_, _, _), ParamValueProjection::Vec3ToColorRgb) => {
            ParamValue::Vec3(*r, *g, *b)
        }
        (ParamValue::Color(r, g, b, _), ParamValue::Vec3(_, _, _), ParamValueProjection::Vec3ToColorHsv) => {
            let (h, s, v) = rgb_to_hsv(*r, *g, *b);
            ParamValue::Vec3(h, s, v)
        }
        (ParamValue::Float(value), ParamValue::Color(_, g, b, a), ParamValueProjection::ColorR) => {
            ParamValue::Color(*value, *g, *b, *a)
        }
        (ParamValue::Float(value), ParamValue::Color(r, _, b, a), ParamValueProjection::ColorG) => {
            ParamValue::Color(*r, *value, *b, *a)
        }
        (ParamValue::Float(value), ParamValue::Color(r, g, _, a), ParamValueProjection::ColorB) => {
            ParamValue::Color(*r, *g, *value, *a)
        }
        (ParamValue::Float(value), ParamValue::Color(r, g, b, _), ParamValueProjection::ColorA) => {
            ParamValue::Color(*r, *g, *b, *value)
        }
        (ParamValue::Vec3(r, g, b), ParamValue::Color(_, _, _, target_a), ParamValueProjection::ColorToVec3Rgb) => {
            ParamValue::Color(*r, *g, *b, *target_a)
        }
        (ParamValue::Vec3(h, s, v), ParamValue::Color(_, _, _, target_a), ParamValueProjection::ColorToVec3Hsv) => {
            let (r, g, b) = hsv_to_rgb(*h, *s, *v);
            ParamValue::Color(r, g, b, *target_a)
        }
        (
            ParamValue::Vec2(h, s),
            ParamValue::Color(target_r, target_g, target_b, target_a),
            ParamValueProjection::ColorToVec2Hs,
        ) => {
            let (_, _, value) = rgb_to_hsv(*target_r, *target_g, *target_b);
            let (r, g, b) = hsv_to_rgb(*h, *s, value);
            ParamValue::Color(r, g, b, *target_a)
        }
        _ => return None,
    };
    Some(projected)
}

fn coerce_param_value_for_target_kind(source: &ParamValue, target: &ParamValue) -> Option<ParamValue> {
    match target {
        ParamValue::Trigger() => {
            if matches!(source, ParamValue::Trigger()) {
                Some(ParamValue::Trigger())
            } else {
                None
            }
        }
        ParamValue::Int(_) => source.as_int().map(ParamValue::Int),
        ParamValue::Float(_) => source.as_float().map(ParamValue::Float),
        ParamValue::Str(_) => source.as_str().map(ParamValue::Str),
        ParamValue::File(_) => source.as_str().map(ParamValue::File),
        ParamValue::Enum(_) => source.as_enum().map(ParamValue::Enum),
        ParamValue::Bool(_) => source.as_bool().map(ParamValue::Bool),
        ParamValue::CssValue(target_value) => source
            .as_css_value_with_unit(target_value.unit)
            .map(ParamValue::CssValue),
        ParamValue::Vec2(_, _) => source.as_vec2().map(|(x, y)| ParamValue::Vec2(x, y)),
        ParamValue::Vec3(_, _, _) => source.as_vec3().map(|(x, y, z)| ParamValue::Vec3(x, y, z)),
        ParamValue::Color(_, _, _, _) => source.as_color().map(|(r, g, b, a)| ParamValue::Color(r, g, b, a)),
        ParamValue::Reference(_) => match source {
            ParamValue::Reference(reference) => Some(ParamValue::Reference(reference.clone())),
            _ => None,
        },
    }
}

/// Coerces `source` to the value kind represented by `target`.
///
/// When `projection` is provided, it is applied before coercion.
pub fn coerce_param_value_for_target(
    source: &ParamValue,
    target: &ParamValue,
    projection: Option<ParamValueProjection>,
) -> Option<ParamValue> {
    let projected = if let Some(projection) = projection {
        project_param_value_for_target(source, target, projection)?
    } else {
        source.clone()
    };

    coerce_param_value_for_target_kind(&projected, target)
}

/// Coerces `source` to `target` using the reverse direction of one projection.
///
/// This is primarily used by bidirectional binding so one selected projection
/// can be applied in both write directions.
pub fn coerce_param_value_for_target_reverse(
    source: &ParamValue,
    target: &ParamValue,
    projection: Option<ParamValueProjection>,
) -> Option<ParamValue> {
    let projected = if let Some(projection) = projection {
        project_param_value_for_target_reverse(source, target, projection)?
    } else {
        source.clone()
    };

    coerce_param_value_for_target_kind(&projected, target)
}

/// Computes compatibility between `source` and `target`.
pub fn compatibility_for_values(source: &ParamValue, target: &ParamValue) -> ParamValueCompatibility {
    let direct = coerce_param_value_for_target(source, target, None).is_some();
    let projections = ParamValueProjection::available_for_source(source)
        .into_iter()
        .filter(|projection| coerce_param_value_for_target(source, target, Some(*projection)).is_some())
        .collect();

    ParamValueCompatibility { direct, projections }
}

/// Computes binding compatibility between `source` and `target`.
///
/// Binding is bidirectional, so direct/projection compatibility is only
/// considered valid when conversion succeeds in both directions.
pub fn compatibility_for_binding_values(source: &ParamValue, target: &ParamValue) -> ParamValueCompatibility {
    let direct = coerce_param_value_for_target(source, target, None).is_some()
        && coerce_param_value_for_target(target, source, None).is_some();
    let projections = ParamValueProjection::available_for_source(source)
        .into_iter()
        .filter(|projection| {
            coerce_param_value_for_target(source, target, Some(*projection)).is_some()
                && coerce_param_value_for_target_reverse(target, source, Some(*projection)).is_some()
        })
        .collect();

    ParamValueCompatibility { direct, projections }
}

/// Strongly-typed file path wrapper for parameter handles and params DSL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct File(pub String);

impl File {
    /// Creates a new file value from a path-like string.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Returns the wrapped path string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns this file value as a [`Path`].
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    /// Consumes this wrapper and returns the inner path string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for File {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for File {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<PathBuf> for File {
    fn from(value: PathBuf) -> Self {
        Self(value.to_string_lossy().to_string())
    }
}

impl From<File> for String {
    fn from(value: File) -> Self {
        value.0
    }
}

/// CSS unit used by [`CssValue`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CssUnit {
    /// CSS pixels.
    Px,
    /// Root-em units.
    #[default]
    Rem,
    /// Element-em units.
    Em,
    /// Percentage values.
    Percent,
    /// Viewport width percentage.
    Vw,
    /// Viewport height percentage.
    Vh,
}

impl CssUnit {
    /// Returns the CSS suffix used when formatting this unit.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Px => "px",
            Self::Rem => "rem",
            Self::Em => "em",
            Self::Percent => "%",
            Self::Vw => "vw",
            Self::Vh => "vh",
        }
    }
}

impl fmt::Display for CssUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.suffix())
    }
}

/// CSS scalar value with an explicit unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct CssValue {
    /// Numeric component.
    pub value: f64,
    /// CSS unit.
    pub unit: CssUnit,
}

impl CssValue {
    /// Creates a new CSS value.
    pub fn new(value: f64, unit: CssUnit) -> Self {
        Self { value, unit }
    }

    /// Parses a CSS value string such as `12rem` or `50%`.
    pub fn parse(value: &str) -> Option<Self> {
        Self::parse_with_default_unit(value, None)
    }

    /// Parses a CSS value string, accepting raw numbers via `default_unit`.
    pub fn parse_with_default_unit(value: &str, default_unit: Option<CssUnit>) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lowercase = trimmed.to_ascii_lowercase();
        for (suffix, unit) in [
            ("rem", CssUnit::Rem),
            ("px", CssUnit::Px),
            ("em", CssUnit::Em),
            ("vw", CssUnit::Vw),
            ("vh", CssUnit::Vh),
            ("%", CssUnit::Percent),
        ] {
            if lowercase.ends_with(suffix) {
                let number_text = trimmed[..trimmed.len() - suffix.len()].trim();
                let parsed = number_text.parse::<f64>().ok()?;
                return Some(Self::new(parsed, unit));
            }
        }

        let unit = default_unit?;
        trimmed.parse::<f64>().ok().map(|parsed| Self::new(parsed, unit))
    }

    /// Formats this CSS value as a CSS string.
    pub fn to_css_string(self) -> String {
        format!("{}{}", self.value, self.unit.suffix())
    }
}

impl fmt::Display for CssValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_css_string())
    }
}

impl From<(f64, CssUnit)> for CssValue {
    fn from(value: (f64, CssUnit)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl From<CssValue> for (f64, CssUnit) {
    fn from(value: CssValue) -> Self {
        (value.value, value.unit)
    }
}

impl From<File> for PathBuf {
    fn from(value: File) -> Self {
        PathBuf::from(value.0)
    }
}

/// Strongly-typed 2D vector value for parameter handles and params DSL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Vec2 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
}

impl Vec2 {
    /// Creates a new 2D vector.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl From<(f64, f64)> for Vec2 {
    fn from(value: (f64, f64)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl From<Vec2> for (f64, f64) {
    fn from(value: Vec2) -> Self {
        (value.x, value.y)
    }
}

/// Strongly-typed 3D vector value for parameter handles and params DSL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct Vec3 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl Vec3 {
    /// Creates a new 3D vector.
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

impl From<(f64, f64, f64)> for Vec3 {
    fn from(value: (f64, f64, f64)) -> Self {
        Self::new(value.0, value.1, value.2)
    }
}

impl From<Vec3> for (f64, f64, f64) {
    fn from(value: Vec3) -> Self {
        (value.x, value.y, value.z)
    }
}

/// Strongly-typed enum variant wrapper for parameter handles and params DSL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct Enum(pub String);

impl Enum {
    /// Creates a new enum value from a variant id.
    pub fn new(variant_id: impl Into<String>) -> Self {
        Self(variant_id.into())
    }

    /// Returns the wrapped variant id.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the inner variant id.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for Enum {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Enum {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<Enum> for String {
    fn from(value: Enum) -> Self {
        value.0
    }
}

//implement into for ParamValue
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

//Implement value coercion
impl ParamValue {
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
                if parts.len() == 2 {
                    if let (Ok(x), Ok(y)) = (parts[0].trim().parse(), parts[1].trim().parse()) {
                        return Some((x, y));
                    }
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
                if parts.len() == 3 {
                    if let (Ok(x), Ok(y), Ok(z)) = (
                        parts[0].trim().parse(),
                        parts[1].trim().parse(),
                        parts[2].trim().parse(),
                    ) {
                        return Some((x, y, z));
                    }
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
                if parts.len() == 4 {
                    if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
                        parts[0].trim().parse(),
                        parts[1].trim().parse(),
                        parts[2].trim().parse(),
                        parts[3].trim().parse(),
                    ) {
                        return Some((r, g, b, a));
                    }
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
                if let Some(value) = number.as_i64() {
                    if let Ok(value) = i32::try_from(value) {
                        return Ok(ParamValue::Int(value));
                    }
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_nodes_are_not_disableable_by_default() {
        let parameter = Parameter::new("Amount", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange);

        assert!(!parameter.node_data().meta.can_be_disabled);
    }

    fn approx_eq(left: f64, right: f64) {
        assert!((left - right).abs() < 1e-9, "expected {left} ~= {right}");
    }

    #[test]
    fn file_constraints_accept_matching_extension() {
        let constraints = ParameterConstraints {
            file: FileConstraints {
                allowed_types: vec![FileTypeGroup::Audio],
                allowed_extensions: vec![".WAV".to_string()],
            },
            ..Default::default()
        };

        let normalized = constraints
            .normalize(ParamValue::File("C:/tmp/kick.wav".to_string()))
            .expect("wav should pass file constraints");
        assert_eq!(normalized, ParamValue::File("C:/tmp/kick.wav".to_string()));
    }

    #[test]
    fn file_constraints_reject_non_matching_extension() {
        let constraints = ParameterConstraints {
            file: FileConstraints {
                allowed_types: vec![FileTypeGroup::Audio],
                allowed_extensions: vec!["wav".to_string(), "flac".to_string()],
            },
            ..Default::default()
        };

        let error = constraints
            .normalize(ParamValue::File("C:/tmp/clip.mp4".to_string()))
            .expect_err("mp4 should fail audio constraints");
        assert!(error.contains("not allowed"));
    }

    #[test]
    fn template_text_mode_is_only_supported_for_string_parameters() {
        let string_value = ParamValue::Str("demo".to_string());
        let int_value = ParamValue::Int(42);

        assert!(control_mode_supported_for_value(
            ParameterControlMode::TemplateText,
            &string_value
        ));
        assert!(!control_mode_supported_for_value(
            ParameterControlMode::TemplateText,
            &int_value
        ));

        let string_modes = available_control_modes_for_value(&string_value);
        assert!(string_modes.contains(&ParameterControlMode::TemplateText));

        let int_modes = available_control_modes_for_value(&int_value);
        assert!(!int_modes.contains(&ParameterControlMode::TemplateText));
    }

    #[test]
    fn projection_supports_float_expansions() {
        let source = ParamValue::Float(2.5);

        assert_eq!(
            project_param_value(&source, ParamValueProjection::FloatToVec2X0),
            Some(ParamValue::Vec2(2.5, 0.0))
        );
        assert_eq!(
            project_param_value(&source, ParamValueProjection::FloatToVec20Y),
            Some(ParamValue::Vec2(0.0, 2.5))
        );
        assert_eq!(
            project_param_value(&source, ParamValueProjection::FloatToVec2XX),
            Some(ParamValue::Vec2(2.5, 2.5))
        );
        assert_eq!(
            project_param_value(&source, ParamValueProjection::FloatToVec3X00),
            Some(ParamValue::Vec3(2.5, 0.0, 0.0))
        );
        assert_eq!(
            project_param_value(&source, ParamValueProjection::FloatToVec30Y0),
            Some(ParamValue::Vec3(0.0, 2.5, 0.0))
        );
        assert_eq!(
            project_param_value(&source, ParamValueProjection::FloatToVec300Z),
            Some(ParamValue::Vec3(0.0, 0.0, 2.5))
        );
        assert_eq!(
            project_param_value(&source, ParamValueProjection::FloatToVec3XXX),
            Some(ParamValue::Vec3(2.5, 2.5, 2.5))
        );
    }

    #[test]
    fn projection_supports_vec_reshapes() {
        let vec3 = ParamValue::Vec3(1.0, 2.0, 3.0);
        let vec2 = ParamValue::Vec2(4.0, 5.0);

        assert_eq!(
            project_param_value(&vec3, ParamValueProjection::Vec3ToVec2XY),
            Some(ParamValue::Vec2(1.0, 2.0))
        );
        assert_eq!(
            project_param_value(&vec3, ParamValueProjection::Vec3ToVec2XZ),
            Some(ParamValue::Vec2(1.0, 3.0))
        );
        assert_eq!(
            project_param_value(&vec3, ParamValueProjection::Vec3ToVec2YZ),
            Some(ParamValue::Vec2(2.0, 3.0))
        );
        assert_eq!(
            project_param_value(&vec2, ParamValueProjection::Vec2ToVec3XY0),
            Some(ParamValue::Vec3(4.0, 5.0, 0.0))
        );
        assert_eq!(
            project_param_value(&vec2, ParamValueProjection::Vec2ToVec3X0Y),
            Some(ParamValue::Vec3(4.0, 0.0, 5.0))
        );
    }

    #[test]
    fn projection_supports_color_rgb_hsv_mappings() {
        let color = ParamValue::Color(1.0, 0.0, 0.0, 0.75);
        let vec_hs = ParamValue::Vec2(0.0, 1.0);
        let vec_hsv = ParamValue::Vec3(1.0 / 3.0, 1.0, 1.0);
        let vec_rgb = ParamValue::Vec3(0.1, 0.2, 0.3);

        assert_eq!(
            project_param_value(&color, ParamValueProjection::ColorToVec3Rgb),
            Some(ParamValue::Vec3(1.0, 0.0, 0.0))
        );
        let hsv = project_param_value(&color, ParamValueProjection::ColorToVec3Hsv)
            .expect("color->hsv projection should succeed");
        let ParamValue::Vec3(h, s, v) = hsv else {
            panic!("expected vec3 hsv result");
        };
        approx_eq(h, 0.0);
        approx_eq(s, 1.0);
        approx_eq(v, 1.0);

        let hs = project_param_value(&color, ParamValueProjection::ColorToVec2Hs)
            .expect("color->hs projection should succeed");
        let ParamValue::Vec2(h, s) = hs else {
            panic!("expected vec2 hs result");
        };
        approx_eq(h, 0.0);
        approx_eq(s, 1.0);

        let color_from_hs = project_param_value(&vec_hs, ParamValueProjection::Vec2ToColorHs)
            .expect("vec2 hs->color projection should succeed");
        let ParamValue::Color(r, g, b, a) = color_from_hs else {
            panic!("expected color result");
        };
        approx_eq(r, 1.0);
        approx_eq(g, 0.0);
        approx_eq(b, 0.0);
        approx_eq(a, 1.0);

        let color_from_hsv = project_param_value(&vec_hsv, ParamValueProjection::Vec3ToColorHsv)
            .expect("vec3 hsv->color projection should succeed");
        let ParamValue::Color(r, g, b, a) = color_from_hsv else {
            panic!("expected color result");
        };
        approx_eq(r, 0.0);
        approx_eq(g, 1.0);
        approx_eq(b, 0.0);
        approx_eq(a, 1.0);

        assert_eq!(
            project_param_value(&vec_rgb, ParamValueProjection::Vec3ToColorRgb),
            Some(ParamValue::Color(0.1, 0.2, 0.3, 1.0))
        );
    }

    #[test]
    fn compatibility_includes_float_to_vec2_projections() {
        let compatibility = compatibility_for_values(&ParamValue::Float(3.0), &ParamValue::Vec2(0.0, 0.0));
        assert!(compatibility.direct, "float->vec2 keeps direct coercion");
        assert!(compatibility.projections.contains(&ParamValueProjection::FloatToVec2X0));
        assert!(compatibility.projections.contains(&ParamValueProjection::FloatToVec20Y));
        assert!(compatibility.projections.contains(&ParamValueProjection::FloatToVec2XX));
    }

    #[test]
    fn projected_expansions_preserve_target_components() {
        assert_eq!(
            coerce_param_value_for_target(
                &ParamValue::Float(2.5),
                &ParamValue::Vec2(9.0, 8.0),
                Some(ParamValueProjection::FloatToVec20Y),
            ),
            Some(ParamValue::Vec2(9.0, 2.5))
        );

        assert_eq!(
            coerce_param_value_for_target(
                &ParamValue::Float(2.5),
                &ParamValue::Vec3(9.0, 8.0, 7.0),
                Some(ParamValueProjection::FloatToVec3X00),
            ),
            Some(ParamValue::Vec3(2.5, 8.0, 7.0))
        );

        assert_eq!(
            coerce_param_value_for_target(
                &ParamValue::Vec2(4.0, 5.0),
                &ParamValue::Vec3(9.0, 8.0, 7.0),
                Some(ParamValueProjection::Vec2ToVec3X0Y),
            ),
            Some(ParamValue::Vec3(4.0, 8.0, 5.0))
        );
    }

    #[test]
    fn projected_color_expansions_preserve_existing_channels() {
        assert_eq!(
            coerce_param_value_for_target(
                &ParamValue::Vec3(0.1, 0.2, 0.3),
                &ParamValue::Color(0.0, 0.0, 0.0, 0.7),
                Some(ParamValueProjection::Vec3ToColorRgb),
            ),
            Some(ParamValue::Color(0.1, 0.2, 0.3, 0.7))
        );

        let converted = coerce_param_value_for_target(
            &ParamValue::Vec2(0.0, 1.0),
            &ParamValue::Color(0.2, 0.4, 0.6, 0.25),
            Some(ParamValueProjection::Vec2ToColorHs),
        )
        .expect("vec2 hs projection should convert against color target");
        let ParamValue::Color(r, g, b, a) = converted else {
            panic!("expected color result");
        };
        approx_eq(r, 0.6);
        approx_eq(g, 0.0);
        approx_eq(b, 0.0);
        approx_eq(a, 0.25);
    }

    #[test]
    fn reverse_projection_preserves_target_components() {
        assert_eq!(
            coerce_param_value_for_target_reverse(
                &ParamValue::Float(3.5),
                &ParamValue::Vec2(1.0, 2.0),
                Some(ParamValueProjection::Vec2X)
            ),
            Some(ParamValue::Vec2(3.5, 2.0))
        );
        assert_eq!(
            coerce_param_value_for_target_reverse(
                &ParamValue::Float(9.0),
                &ParamValue::Vec3(1.0, 2.0, 3.0),
                Some(ParamValueProjection::Vec3Y)
            ),
            Some(ParamValue::Vec3(1.0, 9.0, 3.0))
        );
        assert_eq!(
            coerce_param_value_for_target_reverse(
                &ParamValue::Float(0.6),
                &ParamValue::Color(0.1, 0.2, 0.3, 0.4),
                Some(ParamValueProjection::ColorB)
            ),
            Some(ParamValue::Color(0.1, 0.2, 0.6, 0.4))
        );
    }

    #[test]
    fn binding_compatibility_requires_bidirectional_direct_conversion() {
        let compatibility =
            compatibility_for_binding_values(&ParamValue::Color(0.1, 0.2, 0.3, 1.0), &ParamValue::Vec3(0.0, 0.0, 0.0));
        assert!(!compatibility.direct, "color->vec3 is direct, but vec3->color is not");
        assert!(
            compatibility
                .projections
                .contains(&ParamValueProjection::ColorToVec3Rgb)
        );
    }

    #[test]
    fn binding_projection_roundtrips_vec2_x_to_float() {
        let forward = coerce_param_value_for_target(
            &ParamValue::Vec2(8.0, 4.0),
            &ParamValue::Float(0.0),
            Some(ParamValueProjection::Vec2X),
        )
        .expect("vec2 x projection should produce float");
        assert_eq!(forward, ParamValue::Float(8.0));

        let reverse = coerce_param_value_for_target_reverse(
            &ParamValue::Float(6.0),
            &ParamValue::Vec2(8.0, 4.0),
            Some(ParamValueProjection::Vec2X),
        )
        .expect("reverse vec2 x projection should update vec2");
        assert_eq!(reverse, ParamValue::Vec2(6.0, 4.0));
    }
}
