use golden_core::parameter::{ParamValue, ParameterEnumOption};
use serde::{Deserialize, Serialize};

pub(crate) const MQTT_PUBLISH_COMMAND_NODE_TYPE: &str = "mqtt_publish_command";

pub(crate) const MQTT_QOS_AT_MOST_ONCE: &str = "at_most_once";
pub(crate) const MQTT_QOS_AT_LEAST_ONCE: &str = "at_least_once";
pub(crate) const MQTT_QOS_EXACTLY_ONCE: &str = "exactly_once";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MqttQos {
    #[default]
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

impl MqttQos {
    pub(crate) fn from_variant(variant: &str) -> Option<Self> {
        match variant.trim() {
            MQTT_QOS_AT_MOST_ONCE | "0" | "qos0" | "qos_0" => Some(Self::AtMostOnce),
            MQTT_QOS_AT_LEAST_ONCE | "1" | "qos1" | "qos_1" => Some(Self::AtLeastOnce),
            MQTT_QOS_EXACTLY_ONCE | "2" | "qos2" | "qos_2" => Some(Self::ExactlyOnce),
            _ => None,
        }
    }

    pub(crate) fn variant(self) -> &'static str {
        match self {
            Self::AtMostOnce => MQTT_QOS_AT_MOST_ONCE,
            Self::AtLeastOnce => MQTT_QOS_AT_LEAST_ONCE,
            Self::ExactlyOnce => MQTT_QOS_EXACTLY_ONCE,
        }
    }

    pub(crate) fn to_rumqttc(self) -> rumqttc::QoS {
        match self {
            Self::AtMostOnce => rumqttc::QoS::AtMostOnce,
            Self::AtLeastOnce => rumqttc::QoS::AtLeastOnce,
            Self::ExactlyOnce => rumqttc::QoS::ExactlyOnce,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct MqttPublishRequest {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: MqttQos,
    pub retain: bool,
    pub description: String,
}

pub(crate) fn mqtt_qos_enum_options() -> Vec<ParameterEnumOption> {
    [
        (MQTT_QOS_AT_MOST_ONCE, "QoS 0 - At Most Once"),
        (MQTT_QOS_AT_LEAST_ONCE, "QoS 1 - At Least Once"),
        (MQTT_QOS_EXACTLY_ONCE, "QoS 2 - Exactly Once"),
    ]
    .into_iter()
    .enumerate()
    .map(|(ordering, (variant_id, label))| ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering as i32),
    })
    .collect()
}
