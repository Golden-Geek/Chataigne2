#[cfg(all(windows, feature = "d3d11"))]
use std::fmt::Write;

/// Typed fragment input shared by source metadata and draw preparation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UniformValue {
    /// One GLSL float.
    Float(f32),
    /// One GLSL int.
    Int(i32),
    /// One GLSL bool.
    Bool(bool),
    /// A two-component vector.
    Point([f32; 2]),
    /// A three-component vector.
    Vector3([f32; 3]),
    /// Straight RGBA.
    Color([f32; 4]),
}

/// A scalar, point or color input declared by the source.
#[derive(Clone, Debug, PartialEq)]
pub struct FragmentUniform {
    /// Exact shader identifier.
    pub name: String,
    /// Optional display label.
    pub label: Option<String>,
    /// Value used without an authored override.
    pub default: UniformValue,
    /// Optional inclusive lower bound.
    pub min: Option<UniformValue>,
    /// Optional inclusive upper bound.
    pub max: Option<UniformValue>,
}

impl UniformValue {
    pub(super) fn scalar(self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(f64::from(value)),
            Self::Int(value) => Some(f64::from(value)),
            Self::Bool(value) => Some(f64::from(u8::from(value))),
            Self::Point(_) | Self::Vector3(_) | Self::Color(_) => None,
        }
    }

    pub(super) fn finite(self) -> bool {
        match self {
            Self::Float(value) => value.is_finite(),
            Self::Point(values) => values.into_iter().all(f32::is_finite),
            Self::Vector3(values) => values.into_iter().all(f32::is_finite),
            Self::Color(values) => values.into_iter().all(f32::is_finite),
            Self::Int(_) | Self::Bool(_) => true,
        }
    }

    #[cfg(all(windows, feature = "d3d11"))]
    pub(super) fn pack(self) -> [u32; 4] {
        match self {
            Self::Float(value) => [value.to_bits(), 0, 0, 0],
            Self::Int(value) => [value as u32, 0, 0, 0],
            Self::Bool(value) => [u32::from(value), 0, 0, 0],
            Self::Point([x, y]) => [x.to_bits(), y.to_bits(), 0, 0],
            Self::Vector3([x, y, z]) => [x.to_bits(), y.to_bits(), z.to_bits(), 0],
            Self::Color(value) => value.map(f32::to_bits),
        }
    }

    #[cfg(all(windows, feature = "d3d11"))]
    pub(super) fn unpack(self, words: [u32; 4]) -> Self {
        match self {
            Self::Float(_) => Self::Float(f32::from_bits(words[0])),
            Self::Int(_) => Self::Int(words[0] as i32),
            Self::Bool(_) => Self::Bool(words[0] != 0),
            Self::Point(_) => Self::Point([f32::from_bits(words[0]), f32::from_bits(words[1])]),
            Self::Vector3(_) => Self::Vector3([
                f32::from_bits(words[0]),
                f32::from_bits(words[1]),
                f32::from_bits(words[2]),
            ]),
            Self::Color(_) => Self::Color(words.map(f32::from_bits)),
        }
    }
}

#[cfg(all(windows, feature = "d3d11"))]
pub(super) const BUFFER_BINDING: u32 = super::TEXTURE_BINDINGS as u32 + 1;

#[cfg(all(windows, feature = "d3d11"))]
pub(super) fn glsl(uniforms: &[FragmentUniform], declarations: &mut String, setup: &mut String) {
    if uniforms.is_empty() {
        return;
    }
    let count = uniforms.len();
    writeln!(
        declarations,
        "layout(std140, binding = {BUFFER_BINDING}) uniform GoldenInputs {{ uvec4 slots[{count}]; }} goldenInputs;"
    )
    .unwrap();
    for (index, uniform) in uniforms.iter().enumerate() {
        let data = format!("goldenInputs.slots[{index}]");
        let (ty, expression) = match uniform.default {
            UniformValue::Float(_) => ("float", format!("uintBitsToFloat({data}.x)")),
            UniformValue::Int(_) => ("int", format!("int({data}.x)")),
            UniformValue::Bool(_) => ("bool", format!("{data}.x != 0u")),
            UniformValue::Point(_) => ("vec2", format!("uintBitsToFloat({data}.xy)")),
            UniformValue::Vector3(_) => ("vec3", format!("uintBitsToFloat({data}.xyz)")),
            UniformValue::Color(_) => ("vec4", format!("uintBitsToFloat({data})")),
        };
        writeln!(declarations, "{ty} {};", uniform.name).unwrap();
        writeln!(setup, "    {} = {expression};", uniform.name).unwrap();
    }
}
