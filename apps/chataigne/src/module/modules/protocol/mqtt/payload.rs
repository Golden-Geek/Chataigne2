use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MqttReceivedValue {
    pub(super) path_segments: Vec<String>,
    pub(super) payload: ReceivedValuePayload,
    pub(super) source_description: String,
}

pub(super) fn received_values_for_publish(
    message: &MqttReceivedPublish,
    payload_mode: &str,
) -> Result<Vec<MqttReceivedValue>, String> {
    let path_segments = mqtt_topic_segments(message.topic.as_str());
    let source_description = format!("MQTT topic '{}'", message.topic);

    match payload_mode {
        MQTT_PAYLOAD_MODE_JSON => json_received_values(message.payload.as_slice(), path_segments, source_description),
        MQTT_PAYLOAD_MODE_RAW => Ok(vec![MqttReceivedValue {
            path_segments,
            payload: raw_payload_value(message.payload.as_slice()),
            source_description,
        }]),
        MQTT_PAYLOAD_MODE_TEXT => Ok(vec![MqttReceivedValue {
            path_segments,
            payload: text_payload_value(message.payload.as_slice()),
            source_description,
        }]),
        _ => auto_received_values(message.payload.as_slice(), path_segments, source_description),
    }
}

pub(super) fn auto_received_values(
    payload: &[u8],
    path_segments: Vec<String>,
    source_description: String,
) -> Result<Vec<MqttReceivedValue>, String> {
    if payload_looks_like_json(payload) {
        if let Ok(values) = json_received_values(payload, path_segments.clone(), source_description.clone()) {
            return Ok(values);
        }
    }

    Ok(vec![MqttReceivedValue {
        path_segments,
        payload: text_payload_value(payload),
        source_description,
    }])
}

pub(super) fn json_received_values(
    payload: &[u8],
    path_segments: Vec<String>,
    source_description: String,
) -> Result<Vec<MqttReceivedValue>, String> {
    let value: serde_json::Value =
        serde_json::from_slice(payload).map_err(|error| format!("invalid JSON payload: {error}"))?;
    let mut messages = Vec::new();
    let mut current_path = path_segments;
    collect_json_received_values(&value, &mut current_path, source_description.as_str(), &mut messages)?;
    Ok(messages)
}

pub(super) fn collect_json_received_values(
    value: &serde_json::Value,
    path_segments: &mut Vec<String>,
    source_description: &str,
    messages: &mut Vec<MqttReceivedValue>,
) -> Result<(), String> {
    if let Some(payload) = decode_json_payload(value)? {
        messages.push(MqttReceivedValue {
            path_segments: path_segments.clone(),
            payload,
            source_description: source_description.to_string(),
        });
        return Ok(());
    }

    match value {
        serde_json::Value::Object(entries) => {
            for (key, child) in entries {
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                path_segments.push(key.to_string());
                collect_json_received_values(child, path_segments, source_description, messages)?;
                path_segments.pop();
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path_segments.push((index + 1).to_string());
                collect_json_received_values(child, path_segments, source_description, messages)?;
                path_segments.pop();
            }
        }
        _ => {}
    }

    Ok(())
}

pub(super) fn decode_json_payload(value: &serde_json::Value) -> Result<Option<ReceivedValuePayload>, String> {
    match value {
        serde_json::Value::Object(_) => Ok(None),
        serde_json::Value::Array(items) => {
            if let Ok(value) = ParamValue::from_script_json(value) {
                return Ok(Some(ReceivedValuePayload::Single(value)));
            }

            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let Ok(value) = ParamValue::from_script_json(item) else {
                    return Ok(None);
                };
                values.push(value);
            }

            Ok(Some(match values.len() {
                0 => ReceivedValuePayload::Single(ParamValue::Trigger()),
                1 => ReceivedValuePayload::Single(values.remove(0)),
                _ => ReceivedValuePayload::Multi(values),
            }))
        }
        _ => ParamValue::from_script_json(value)
            .map(ReceivedValuePayload::Single)
            .map(Some)
            .map_err(|error| format!("invalid JSON value: {error}")),
    }
}

pub(super) fn text_payload_value(payload: &[u8]) -> ReceivedValuePayload {
    if payload.is_empty() {
        return ReceivedValuePayload::Single(ParamValue::Trigger());
    }

    ReceivedValuePayload::Single(parse_scalar_value(String::from_utf8_lossy(payload).as_ref()))
}

pub(super) fn raw_payload_value(payload: &[u8]) -> ReceivedValuePayload {
    if payload.is_empty() {
        return ReceivedValuePayload::Single(ParamValue::Trigger());
    }

    ReceivedValuePayload::Multi(payload.iter().map(|byte| ParamValue::Int(i32::from(*byte))).collect())
}

pub(super) fn payload_looks_like_json(payload: &[u8]) -> bool {
    let trimmed = payload
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .collect::<Vec<_>>();
    matches!(trimmed.first(), Some(b'{') | Some(b'['))
}

pub(super) fn mqtt_topic_segments(topic: &str) -> Vec<String> {
    let segments = topic
        .split('/')
        .enumerate()
        .map(|(index, segment)| {
            let segment = segment.trim();
            if segment.is_empty() {
                format!("level_{}", index + 1)
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>();

    if segments.is_empty() {
        vec!["message".to_string()]
    } else {
        segments
    }
}

pub(super) fn mqtt_publish_request_from_script(
    method: &str,
    args: &[ParamValue],
) -> Option<Result<MqttPublishRequest, String>> {
    match method {
        "publish" | "publishText" | "publishJson" => {}
        _ => return None,
    }

    let Some(topic) = args.first().and_then(ParamValue::as_str) else {
        return Some(Err(format!("method '{method}' expects an MQTT topic string")));
    };

    let payload = match method {
        "publishJson" => args
            .get(1)
            .map(ParamValue::to_script_json)
            .map(|value| serde_json::to_vec(&value).map_err(|error| format!("failed to encode JSON payload: {error}")))
            .transpose()
            .map(|payload| payload.unwrap_or_default()),
        _ => Ok(args
            .get(1)
            .and_then(ParamValue::as_str)
            .unwrap_or_default()
            .into_bytes()),
    };

    Some(payload.and_then(|payload| {
        Ok(MqttPublishRequest {
            topic,
            payload,
            qos: script_qos_arg(args.get(2))?,
            retain: args.get(3).and_then(ParamValue::as_bool).unwrap_or(false),
            description: method.to_string(),
        })
    }))
}

pub(super) fn script_qos_arg(value: Option<&ParamValue>) -> Result<MqttQos, String> {
    let Some(value) = value else {
        return Ok(MqttQos::AtMost);
    };

    if let Some(value) = value.as_int() {
        return match value {
            0 => Ok(MqttQos::AtMost),
            1 => Ok(MqttQos::AtLeast),
            2 => Ok(MqttQos::Exactly),
            _ => Err(format!("MQTT QoS integer must be 0, 1, or 2, got {value}")),
        };
    }

    let variant = value
        .as_str()
        .ok_or_else(|| "MQTT QoS must be an integer or string".to_string())?;
    MqttQos::from_variant(variant.as_str()).ok_or_else(|| format!("invalid MQTT QoS '{variant}'"))
}

pub(super) fn mqtt_payload_script_arg(payload: &[u8]) -> serde_json::Value {
    std::str::from_utf8(payload)
        .map(|text| serde_json::json!(text))
        .unwrap_or_else(|_| crate::app::module::script_api::bytes_arg(payload))
}

pub(super) fn format_mqtt_payload_for_log(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "<empty>".to_string();
    }

    match std::str::from_utf8(payload) {
        Ok(text) if !text.chars().any(char::is_control) => format!("\"{text}\""),
        _ => payload
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}
