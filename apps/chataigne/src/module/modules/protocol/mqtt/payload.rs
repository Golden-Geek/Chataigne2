use super::*;

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
