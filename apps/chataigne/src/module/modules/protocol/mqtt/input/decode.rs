use super::*;
use crate::app::module::common::{json, received_values::ReceivedValuePayload, streaming::parser::parse_scalar_value};
use golden_core::parameter::ParamValue;

pub(super) fn values(message: &MqttReceivedPublish, mode: PayloadMode) -> Result<Vec<ReceivedValueMessage>, String> {
    if message.payload.len() > MAX_PAYLOAD || message.topic.len() > 4096 {
        return Err("MQTT input exceeds 64 KiB payload / 4096 topic bytes".into());
    }
    let mut path = Vec::new();
    for (index, segment) in message.topic.split('/').enumerate() {
        if path.len() == MAX_PATH {
            return Err("MQTT input path exceeds 24 levels".into());
        }
        path.push(if segment.trim().is_empty() {
            format!("level_{}", index + 1)
        } else {
            segment.trim().to_owned()
        });
    }
    let source = format!("MQTT topic '{}'", message.topic);
    let mut output = Output {
        messages: Vec::new(),
        units: 0,
        bytes: 0,
        source,
    };
    let looks_json = matches!(
        message.payload.iter().copied().find(|byte| !byte.is_ascii_whitespace()),
        Some(b'{') | Some(b'[')
    );
    if mode == PayloadMode::Json || mode == PayloadMode::Auto && looks_json {
        let parsed = std::str::from_utf8(&message.payload)
            .map_err(|error| error.to_string())
            .and_then(|text| json::parse(text, MAX_PAYLOAD, 16_384, MAX_PATH));
        match parsed {
            Ok(value) => {
                output.json(&value, &mut path)?;
                return Ok(output.messages);
            }
            Err(error) if mode == PayloadMode::Json => return Err(format!("Invalid MQTT JSON: {error}")),
            Err(_) => {}
        }
    }
    let payload = if message.payload.is_empty() {
        ReceivedValuePayload::Single(ParamValue::Trigger())
    } else if mode == PayloadMode::Raw {
        if message.payload.len() > MAX_UNITS {
            return Err("MQTT raw values exceed 8192 units".into());
        }
        ReceivedValuePayload::Multi(
            message
                .payload
                .iter()
                .map(|byte| ParamValue::Int(i32::from(*byte)))
                .collect(),
        )
    } else {
        ReceivedValuePayload::Single(parse_scalar_value(String::from_utf8_lossy(&message.payload).as_ref()))
    };
    output.push(&path, payload)?;
    Ok(output.messages)
}

struct Output {
    messages: Vec<ReceivedValueMessage>,
    units: usize,
    bytes: usize,
    source: String,
}
impl Output {
    fn push(
        &mut self,
        path: &[String],
        payload: crate::app::module::common::received_values::ReceivedValuePayload,
    ) -> Result<(), String> {
        use crate::app::module::common::received_values::ReceivedValuePayload;
        let units = match &payload {
            ReceivedValuePayload::Single(_) => 1,
            ReceivedValuePayload::Multi(values) => values.len(),
        };
        let bytes = 2 * size_of::<ReceivedValueMessage>()
            + std::mem::size_of_val(path)
            + path.iter().map(String::len).sum::<usize>()
            + self.source.len()
            + payload.retained_weight();
        if path.len() > MAX_PATH
            || self.units.saturating_add(units) > MAX_UNITS
            || self.bytes.saturating_add(bytes) > MAX_VALUES_BYTES
        {
            return Err("MQTT values exceed 24 path levels / 8192 units / 4 MiB; message was not admitted".into());
        }
        self.units += units;
        self.bytes += bytes;
        self.messages.push(ReceivedValueMessage {
            path_segments: path.to_vec(),
            payload,
            source_description: self.source.clone(),
        });
        Ok(())
    }
    fn json(&mut self, value: &serde_json::Value, path: &mut Vec<String>) -> Result<(), String> {
        use crate::app::module::common::received_values::ReceivedValuePayload;
        if path.len() > MAX_PATH {
            return Err("MQTT input path exceeds 24 levels".into());
        }
        if !value.is_object() {
            if let Ok(value) = ParamValue::from_script_json(value) {
                return self.push(path, ReceivedValuePayload::Single(value));
            }
        }
        match value {
            serde_json::Value::Object(entries) => {
                for (key, child) in entries {
                    if key.trim().is_empty() {
                        continue;
                    }
                    path.push(key.trim().to_owned());
                    self.json(child, path)?;
                    path.pop();
                }
            }
            serde_json::Value::Array(items) => {
                if items.len() > MAX_UNITS {
                    return Err("MQTT JSON list exceeds 8192 values".into());
                }
                let flat = items
                    .iter()
                    .map(ParamValue::from_script_json)
                    .collect::<Result<Vec<_>, _>>();
                if let Ok(mut values) = flat {
                    let payload = match values.len() {
                        0 => ReceivedValuePayload::Single(ParamValue::Trigger()),
                        1 => ReceivedValuePayload::Single(values.remove(0)),
                        _ => ReceivedValuePayload::Multi(values),
                    };
                    self.push(path, payload)?;
                } else {
                    for (index, child) in items.iter().enumerate() {
                        path.push((index + 1).to_string());
                        self.json(child, path)?;
                        path.pop();
                    }
                }
            }
            _ => return Err("Invalid MQTT JSON value".into()),
        }
        Ok(())
    }
}
