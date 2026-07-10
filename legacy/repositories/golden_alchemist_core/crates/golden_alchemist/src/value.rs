use std::{sync::Arc, time::Duration};

use crate::ValueTypeId;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColorValue {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl ColorValue {
    pub const BLACK: Self = Self {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ValueComponent {
    X,
    Y,
    Z,
    R,
    G,
    B,
    A,
}

impl ValueComponent {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "z" => Some(Self::Z),
            "r" => Some(Self::R),
            "g" => Some(Self::G),
            "b" => Some(Self::B),
            "a" => Some(Self::A),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TriggerValue {
    pub fired: bool,
    pub edge_id: u64,
    pub logical_tick: u64,
}

impl TriggerValue {
    #[must_use]
    pub const fn fired(edge_id: u64, logical_tick: u64) -> Self {
        Self {
            fired: true,
            edge_id,
            logical_tick,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StableRef {
    pub value_type: ValueTypeId,
    pub stable_id: Arc<str>,
}

impl StableRef {
    #[must_use]
    pub fn new(value_type: ValueTypeId, stable_id: impl Into<Arc<str>>) -> Self {
        Self {
            value_type,
            stable_id: stable_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtensionValue {
    pub value_type: ValueTypeId,
    pub payload: Arc<[u8]>,
}

impl ExtensionValue {
    #[must_use]
    pub fn new(value_type: ValueTypeId, payload: impl Into<Arc<[u8]>>) -> Self {
        Self {
            value_type,
            payload: payload.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Trigger(TriggerValue),
    Int(i64),
    Float(f64),
    String(Arc<str>),
    Vec2([f64; 2]),
    Vec3([f64; 3]),
    Color(ColorValue),
    Duration(Duration),
    Array(Vec<RuntimeValue>),
    Ref(StableRef),
    Extension(ExtensionValue),
}

impl RuntimeValue {
    #[must_use]
    pub fn value_type(&self) -> ValueTypeId {
        match self {
            Self::Unit => ValueTypeId::new("unit"),
            Self::Bool(_) => ValueTypeId::new("bool"),
            Self::Trigger(_) => ValueTypeId::new("trigger"),
            Self::Int(_) => ValueTypeId::new("int"),
            Self::Float(_) => ValueTypeId::new("float"),
            Self::String(_) => ValueTypeId::new("string"),
            Self::Vec2(_) => ValueTypeId::new("vec2"),
            Self::Vec3(_) => ValueTypeId::new("vec3"),
            Self::Color(_) => ValueTypeId::new("color"),
            Self::Duration(_) => ValueTypeId::new("duration"),
            Self::Array(_) => ValueTypeId::new("value_array"),
            Self::Ref(value) => value.value_type.clone(),
            Self::Extension(value) => value.value_type.clone(),
        }
    }

    pub fn convert_to(&self, target: &ValueTypeId) -> Result<Self, String> {
        if self.value_type() == *target {
            return Ok(self.clone());
        }

        Ok(match target.as_str() {
            "unit" => Self::Unit,
            "bool" => Self::Bool(self.to_bool_lossy()),
            "int" => Self::Int(self.to_int_lossy()),
            "float" => Self::Float(self.to_float_lossy()),
            "string" => Self::String(Arc::from(self.to_string_lossy())),
            "vec2" => Self::Vec2(self.to_vec2_lossy()),
            "vec3" => Self::Vec3(self.to_vec3_lossy()),
            "color" => Self::Color(self.to_color_lossy()),
            "duration" => Self::Duration(Duration::from_secs_f64(self.to_float_lossy().max(0.0))),
            "value_array" => Self::Array(match self {
                Self::Array(values) => values.clone(),
                _ => vec![self.clone()],
            }),
            _ => {
                return Err(format!("cannot convert `{}` to `{target}`", self.value_type()));
            }
        })
    }

    #[must_use]
    pub fn component(&self, component: ValueComponent) -> Option<Self> {
        match (self, component) {
            (Self::Vec2(value), ValueComponent::X) => Some(Self::Float(value[0])),
            (Self::Vec2(value), ValueComponent::Y) => Some(Self::Float(value[1])),
            (Self::Vec3(value), ValueComponent::X) => Some(Self::Float(value[0])),
            (Self::Vec3(value), ValueComponent::Y) => Some(Self::Float(value[1])),
            (Self::Vec3(value), ValueComponent::Z) => Some(Self::Float(value[2])),
            (Self::Color(value), ValueComponent::R) => Some(Self::Float(f64::from(value.red))),
            (Self::Color(value), ValueComponent::G) => Some(Self::Float(f64::from(value.green))),
            (Self::Color(value), ValueComponent::B) => Some(Self::Float(f64::from(value.blue))),
            (Self::Color(value), ValueComponent::A) => Some(Self::Float(f64::from(value.alpha))),
            _ => None,
        }
    }

    pub fn with_component(&self, component: ValueComponent, value: &RuntimeValue) -> Result<Self, String> {
        let value = value.convert_to(&ValueTypeId::new("float"))?.to_float_lossy();
        match (self, component) {
            (Self::Vec2(current), ValueComponent::X) => Ok(Self::Vec2([value, current[1]])),
            (Self::Vec2(current), ValueComponent::Y) => Ok(Self::Vec2([current[0], value])),
            (Self::Vec3(current), ValueComponent::X) => Ok(Self::Vec3([value, current[1], current[2]])),
            (Self::Vec3(current), ValueComponent::Y) => Ok(Self::Vec3([current[0], value, current[2]])),
            (Self::Vec3(current), ValueComponent::Z) => Ok(Self::Vec3([current[0], current[1], value])),
            (Self::Color(current), ValueComponent::R) => Ok(Self::Color(ColorValue {
                red: value as f32,
                ..*current
            })),
            (Self::Color(current), ValueComponent::G) => Ok(Self::Color(ColorValue {
                green: value as f32,
                ..*current
            })),
            (Self::Color(current), ValueComponent::B) => Ok(Self::Color(ColorValue {
                blue: value as f32,
                ..*current
            })),
            (Self::Color(current), ValueComponent::A) => Ok(Self::Color(ColorValue {
                alpha: value as f32,
                ..*current
            })),
            _ => Err(format!(
                "value type `{}` has no component `{component:?}`",
                self.value_type()
            )),
        }
    }

    fn to_bool_lossy(&self) -> bool {
        match self {
            Self::Unit => false,
            Self::Bool(value) => *value,
            Self::Trigger(value) => value.fired,
            Self::Int(value) => *value != 0,
            Self::Float(value) => *value != 0.0,
            Self::String(value) => parse_bool(value),
            Self::Vec2(value) => value.iter().any(|component| *component != 0.0),
            Self::Vec3(value) => value.iter().any(|component| *component != 0.0),
            Self::Color(value) => value.red != 0.0 || value.green != 0.0 || value.blue != 0.0 || value.alpha != 0.0,
            Self::Duration(value) => !value.is_zero(),
            Self::Array(value) => !value.is_empty(),
            Self::Ref(value) => !value.stable_id.is_empty(),
            Self::Extension(value) => !value.payload.is_empty(),
        }
    }

    pub(crate) fn to_int_lossy(&self) -> i64 {
        match self {
            Self::Int(value) => *value,
            _ => finite_i64(self.to_float_lossy()),
        }
    }

    pub(crate) fn to_float_lossy(&self) -> f64 {
        match self {
            Self::Unit => 0.0,
            Self::Bool(value) => f64::from(*value),
            Self::Trigger(value) => f64::from(value.fired),
            Self::Int(value) => *value as f64,
            Self::Float(value) => *value,
            Self::String(value) => parse_float(value),
            Self::Vec2(value) => value[0],
            Self::Vec3(value) => value[0],
            Self::Color(value) => f64::from(value.red),
            Self::Duration(value) => value.as_secs_f64(),
            Self::Array(value) => value.first().map_or(0.0, RuntimeValue::to_float_lossy),
            Self::Ref(_) | Self::Extension(_) => 0.0,
        }
    }

    pub(crate) fn to_string_lossy(&self) -> String {
        match self {
            Self::Unit => String::new(),
            Self::Bool(value) => value.to_string(),
            Self::Trigger(value) => value.fired.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::String(value) => value.to_string(),
            Self::Vec2(value) => format!("{},{}", value[0], value[1]),
            Self::Vec3(value) => format!("{},{},{}", value[0], value[1], value[2]),
            Self::Color(value) => format!("{},{},{},{}", value.red, value.green, value.blue, value.alpha),
            Self::Duration(value) => value.as_secs_f64().to_string(),
            Self::Array(values) => values
                .iter()
                .map(RuntimeValue::to_string_lossy)
                .collect::<Vec<_>>()
                .join(","),
            Self::Ref(value) => value.stable_id.to_string(),
            Self::Extension(value) => value.payload.iter().map(|byte| format!("{byte:02x}")).collect(),
        }
    }

    pub(crate) fn to_vec2_lossy(&self) -> [f64; 2] {
        match self {
            Self::Vec2(value) => *value,
            Self::Vec3(value) => [value[0], value[1]],
            Self::Color(value) => [f64::from(value.red), f64::from(value.green)],
            Self::String(value) => parse_components(value)
                .map(|components| [components[0], components.get(1).copied().unwrap_or(components[0])])
                .unwrap_or_else(|| {
                    let value = parse_float(value);
                    [value, value]
                }),
            _ => {
                let value = self.to_float_lossy();
                [value, value]
            }
        }
    }

    pub(crate) fn to_vec3_lossy(&self) -> [f64; 3] {
        match self {
            Self::Vec2(value) => [value[0], value[1], 0.0],
            Self::Vec3(value) => *value,
            Self::Color(value) => [f64::from(value.red), f64::from(value.green), f64::from(value.blue)],
            Self::String(value) => parse_components(value)
                .map(|components| {
                    [
                        components[0],
                        components.get(1).copied().unwrap_or(components[0]),
                        components.get(2).copied().unwrap_or(0.0),
                    ]
                })
                .unwrap_or_else(|| {
                    let value = parse_float(value);
                    [value, value, value]
                }),
            _ => {
                let value = self.to_float_lossy();
                [value, value, value]
            }
        }
    }

    pub(crate) fn to_color_lossy(&self) -> ColorValue {
        match self {
            Self::Color(value) => *value,
            Self::Vec2(value) => ColorValue {
                red: value[0] as f32,
                green: value[1] as f32,
                blue: 0.0,
                alpha: 1.0,
            },
            Self::Vec3(value) => ColorValue {
                red: value[0] as f32,
                green: value[1] as f32,
                blue: value[2] as f32,
                alpha: 1.0,
            },
            Self::String(value) => parse_color(value).unwrap_or_else(|| {
                let value = parse_float(value) as f32;
                ColorValue {
                    red: value,
                    green: value,
                    blue: value,
                    alpha: 1.0,
                }
            }),
            _ => {
                let value = self.to_float_lossy() as f32;
                ColorValue {
                    red: value,
                    green: value,
                    blue: value,
                    alpha: 1.0,
                }
            }
        }
    }
}

#[must_use]
pub fn component_value_type(value_type: &ValueTypeId, component: ValueComponent) -> Option<ValueTypeId> {
    match (value_type.as_str(), component) {
        ("vec2", ValueComponent::X | ValueComponent::Y)
        | ("vec3", ValueComponent::X | ValueComponent::Y | ValueComponent::Z)
        | ("color", ValueComponent::R | ValueComponent::G | ValueComponent::B | ValueComponent::A) => {
            Some(ValueTypeId::new("float"))
        }
        _ => None,
    }
}

fn parse_bool(value: &str) -> bool {
    let value = value.trim();
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes") || value.eq_ignore_ascii_case("on") {
        return true;
    }
    if value.eq_ignore_ascii_case("false")
        || value.eq_ignore_ascii_case("no")
        || value.eq_ignore_ascii_case("off")
        || value.is_empty()
    {
        return false;
    }
    parse_float(value) != 0.0
}

fn parse_float(value: &str) -> f64 {
    value.trim().parse::<f64>().unwrap_or(0.0)
}

fn finite_i64(value: f64) -> i64 {
    if value.is_finite() { value as i64 } else { 0 }
}

fn parse_components(value: &str) -> Option<Vec<f64>> {
    let components = value
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (!components.is_empty()).then_some(components)
}

fn parse_color(value: &str) -> Option<ColorValue> {
    let value = value.trim();
    let hex = value.strip_prefix('#')?;
    let component = |range: std::ops::Range<usize>| {
        u8::from_str_radix(hex.get(range)?, 16)
            .ok()
            .map(|value| f32::from(value) / 255.0)
    };
    match hex.len() {
        6 => Some(ColorValue {
            red: component(0..2)?,
            green: component(2..4)?,
            blue: component(4..6)?,
            alpha: 1.0,
        }),
        8 => Some(ColorValue {
            red: component(0..2)?,
            green: component(2..4)?,
            blue: component(4..6)?,
            alpha: component(6..8)?,
        }),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ValueStorageKind {
    Unit,
    InlineBool,
    Trigger,
    InlineI64,
    InlineF64,
    SharedString,
    InlineVec2,
    InlineVec3,
    InlineColor,
    Duration,
    Array,
    StableRef,
    Extension,
}
