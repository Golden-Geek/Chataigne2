//! Typed, UI-facing encoding of authored command bindings.

use std::time::Duration;

use chataigne_alchemist::{ColorValue, StableRef, ValueComponent, ValueLaneKey, ValueTypeId};
use golden_values::Value as RuntimeValue;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{OutputArgumentBinding, OutputBindingConfig, OutputSendPolicy, OutputValueSource};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MappingOutputBindingsDto {
    pub value: MappingOutputSourceDto,
    pub arguments: Vec<MappingOutputArgumentDto>,
    pub send_policy: MappingOutputSendPolicyDto,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MappingOutputArgumentDto {
    pub parameter: MappingTargetParameterDto,
    pub source: MappingOutputSourceDto,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MappingTargetParameterDto {
    pub value_type: String,
    pub stable_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum MappingOutputSendPolicyDto {
    EveryDelivery,
    OnChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum MappingValueComponentDto {
    X,
    Y,
    Z,
    R,
    G,
    B,
    A,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum MappingOutputSourceDto {
    Whole,
    Element {
        id: String,
    },
    Component {
        element: Option<String>,
        component: MappingValueComponentDto,
    },
    Constant {
        value: MappingConstantDto,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum MappingConstantDto {
    Unit,
    Bool {
        value: bool,
    },
    Int {
        #[ts(type = "number")]
        value: i64,
    },
    Float {
        value: f64,
    },
    String {
        value: String,
    },
    Vec2 {
        value: [f64; 2],
    },
    Vec3 {
        value: [f64; 3],
    },
    Color {
        red: f64,
        green: f64,
        blue: f64,
        alpha: f64,
    },
    Duration {
        seconds: f64,
    },
    /// Preserves uncommon Golden values without narrowing the binding contract.
    Raw {
        json: String,
    },
}

impl MappingConstantDto {
    fn from_domain(value: &RuntimeValue) -> Result<Self, String> {
        Ok(match value {
            RuntimeValue::Unit => Self::Unit,
            RuntimeValue::Bool(value) => Self::Bool { value: *value },
            RuntimeValue::Int(value) if value.unsigned_abs() <= 9_007_199_254_740_991 => Self::Int { value: *value },
            RuntimeValue::Int(_) => Self::Raw {
                json: serde_json::to_string(value).map_err(|error| error.to_string())?,
            },
            RuntimeValue::Float(value) => Self::Float { value: *value },
            RuntimeValue::String(value) => Self::String {
                value: value.to_string(),
            },
            RuntimeValue::Vec2(value) => Self::Vec2 { value: *value },
            RuntimeValue::Vec3(value) => Self::Vec3 { value: *value },
            RuntimeValue::Color(value) => Self::Color {
                red: value.red,
                green: value.green,
                blue: value.blue,
                alpha: value.alpha,
            },
            RuntimeValue::Duration(value) => Self::Duration {
                seconds: value.as_secs_f64(),
            },
            _ => Self::Raw {
                json: serde_json::to_string(value).map_err(|error| error.to_string())?,
            },
        })
    }

    fn into_domain(self) -> Result<RuntimeValue, String> {
        Ok(match self {
            Self::Unit => RuntimeValue::Unit,
            Self::Bool { value } => RuntimeValue::Bool(value),
            Self::Int { value } => RuntimeValue::Int(value),
            Self::Float { value } => RuntimeValue::Float(value),
            Self::String { value } => RuntimeValue::String(value.into()),
            Self::Vec2 { value } => RuntimeValue::Vec2(value),
            Self::Vec3 { value } => RuntimeValue::Vec3(value),
            Self::Color {
                red,
                green,
                blue,
                alpha,
            } => RuntimeValue::Color(ColorValue {
                red,
                green,
                blue,
                alpha,
            }),
            Self::Duration { seconds } if seconds.is_finite() && seconds >= 0.0 => {
                RuntimeValue::Duration(Duration::try_from_secs_f64(seconds).map_err(|error| error.to_string())?)
            }
            Self::Duration { .. } => return Err("constant duration must be finite and non-negative".to_owned()),
            Self::Raw { json } => serde_json::from_str(&json).map_err(|error| error.to_string())?,
        })
    }
}

impl MappingOutputBindingsDto {
    pub fn from_domain(config: &OutputBindingConfig) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            value: MappingOutputSourceDto::from_domain(&config.value)?,
            arguments: config
                .arguments
                .iter()
                .map(|binding| {
                    Ok(MappingOutputArgumentDto {
                        parameter: MappingTargetParameterDto {
                            value_type: binding.parameter.value_type.to_string(),
                            stable_id: binding.parameter.stable_id.to_string(),
                        },
                        source: MappingOutputSourceDto::from_domain(&binding.source)?,
                    })
                })
                .collect::<Result<_, String>>()?,
            send_policy: match config.send_policy {
                OutputSendPolicy::EveryDelivery => MappingOutputSendPolicyDto::EveryDelivery,
                OutputSendPolicy::OnChange => MappingOutputSendPolicyDto::OnChange,
            },
        })
    }

    pub fn into_domain(self) -> Result<OutputBindingConfig, String> {
        let config = OutputBindingConfig {
            value: self.value.into_domain()?,
            arguments: self
                .arguments
                .into_iter()
                .map(|binding| {
                    Ok(OutputArgumentBinding {
                        parameter: StableRef::new(
                            ValueTypeId::new(binding.parameter.value_type),
                            binding.parameter.stable_id,
                        ),
                        source: binding.source.into_domain()?,
                    })
                })
                .collect::<Result<_, String>>()?,
            send_policy: match self.send_policy {
                MappingOutputSendPolicyDto::EveryDelivery => OutputSendPolicy::EveryDelivery,
                MappingOutputSendPolicyDto::OnChange => OutputSendPolicy::OnChange,
            },
        };
        config.validate()?;
        Ok(config)
    }
}

impl MappingOutputSourceDto {
    fn from_domain(source: &OutputValueSource) -> Result<Self, String> {
        Ok(match source {
            OutputValueSource::Whole => Self::Whole,
            OutputValueSource::Element(id) => Self::Element {
                id: id.as_str().to_owned(),
            },
            OutputValueSource::Component { element, component } => Self::Component {
                element: element.as_ref().map(|id| id.as_str().to_owned()),
                component: (*component).into(),
            },
            OutputValueSource::Constant(value) => Self::Constant {
                value: MappingConstantDto::from_domain(value)?,
            },
        })
    }

    fn into_domain(self) -> Result<OutputValueSource, String> {
        Ok(match self {
            Self::Whole => OutputValueSource::Whole,
            Self::Element { id } => {
                OutputValueSource::Element(ValueLaneKey::new(id).map_err(|error| error.to_string())?)
            }
            Self::Component { element, component } => OutputValueSource::Component {
                element: element
                    .map(|id| ValueLaneKey::new(id).map_err(|error| error.to_string()))
                    .transpose()?,
                component: component.into(),
            },
            Self::Constant { value } => OutputValueSource::Constant(value.into_domain()?),
        })
    }
}

impl From<ValueComponent> for MappingValueComponentDto {
    fn from(value: ValueComponent) -> Self {
        match value {
            ValueComponent::X => Self::X,
            ValueComponent::Y => Self::Y,
            ValueComponent::Z => Self::Z,
            ValueComponent::R => Self::R,
            ValueComponent::G => Self::G,
            ValueComponent::B => Self::B,
            ValueComponent::A => Self::A,
        }
    }
}

impl From<MappingValueComponentDto> for ValueComponent {
    fn from(value: MappingValueComponentDto) -> Self {
        match value {
            MappingValueComponentDto::X => Self::X,
            MappingValueComponentDto::Y => Self::Y,
            MappingValueComponentDto::Z => Self::Z,
            MappingValueComponentDto::R => Self::R,
            MappingValueComponentDto::G => Self::G,
            MappingValueComponentDto::B => Self::B,
            MappingValueComponentDto::A => Self::A,
        }
    }
}
