use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::node::NodeReference;

use super::{CssValue, ParamValue};

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
