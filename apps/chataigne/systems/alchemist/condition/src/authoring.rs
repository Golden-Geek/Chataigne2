use std::fmt;

use golden_values::{StableRef, Value};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct ConditionId(Uuid);

impl ConditionId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for ConditionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ConditionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl fmt::Display for ConditionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConditionDefinition {
    pub id: ConditionId,
    pub label: String,
    pub enabled: bool,
    pub kind: ConditionKind,
}

impl ConditionDefinition {
    #[must_use]
    pub fn input_value(label: impl Into<String>, input: StableRef, expected: Value) -> Self {
        Self {
            id: ConditionId::new(),
            label: label.into(),
            enabled: true,
            kind: ConditionKind::InputValue(InputValueCondition {
                input,
                projection: ConditionProjection::Identity,
                comparator: TypedComparator::Equal,
                expected: ConditionOperand::Literal(expected),
                expected_max: None,
                behavior: ConditionBehavior::default(),
            }),
        }
    }

    #[must_use]
    pub fn group(label: impl Into<String>, policy: ConditionGroupPolicy, children: Vec<Self>) -> Self {
        Self {
            id: ConditionId::new(),
            label: label.into(),
            enabled: true,
            kind: ConditionKind::Group { policy, children },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConditionKind {
    Constant(bool),
    InputValue(InputValueCondition),
    InputNode(InputNodeCondition),
    Group {
        policy: ConditionGroupPolicy,
        children: Vec<ConditionDefinition>,
    },
    Script(ScriptCondition),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InputValueCondition {
    pub input: StableRef,
    pub projection: ConditionProjection,
    pub comparator: TypedComparator,
    pub expected: ConditionOperand,
    pub expected_max: Option<ConditionOperand>,
    pub behavior: ConditionBehavior,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InputNodeCondition {
    pub provider: String,
    pub node: StableRef,
    pub projection: ConditionProjection,
    pub comparator: TypedComparator,
    pub expected: ConditionOperand,
    pub expected_max: Option<ConditionOperand>,
    pub behavior: ConditionBehavior,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScriptCondition {
    pub script: String,
    pub behavior: ConditionBehavior,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConditionOperand {
    Literal(Value),
    Input(StableRef),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConditionGroupPolicy {
    #[default]
    All,
    Any,
    None,
    AtLeast(usize),
    Exactly(usize),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConditionProjection {
    #[default]
    Identity,
    Vec2Magnitude,
    Vec2X,
    Vec2Y,
    Vec3Magnitude,
    Vec3X,
    Vec3Y,
    Vec3Z,
    ColorLuminance,
    ColorRed,
    ColorGreen,
    ColorBlue,
    ColorAlpha,
    StringLength,
    DurationSeconds,
    Speed,
    AbsoluteSpeed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TypedComparator {
    #[default]
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Contains,
    StartsWith,
    EndsWith,
    DoesNotContain,
    RegexMatch,
    Between,
    Outside,
    Changed,
    Triggered,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EdgePolicy {
    #[default]
    Level,
    Rising,
    Falling,
    Either,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConditionBehavior {
    pub edge: EdgePolicy,
    pub toggle: bool,
    pub transient_ticks: u32,
    pub validation_delay_ms: u64,
    pub invalidation_delay_ms: u64,
}
