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
            Self::Ref(value) => value.value_type.clone(),
            Self::Extension(value) => value.value_type.clone(),
        }
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
    StableRef,
    Extension,
}
