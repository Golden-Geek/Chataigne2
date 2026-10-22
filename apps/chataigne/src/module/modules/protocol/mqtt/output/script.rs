use super::*;
use crate::app::module::common::mqtt::MqttQos;
use std::io::{self, Write};

pub(super) fn text_bound(value: &ParamValue) -> usize {
    match value {
        ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) => text.len(),
        ParamValue::Reference(reference) => reference.cached_name().map_or(36, str::len),
        _ => 4096,
    }
}

pub(in super::super) fn prepare(
    method: Method,
    args: &[ParamValue],
    limit: usize,
) -> Result<MqttPublishRequest, String> {
    let value = args.first().ok_or("MQTT publish requires a topic")?;
    if text_bound(value) > MAX_TOPIC {
        return Err("MQTT publish topic exceeds 4096 bytes".into());
    }
    let topic = value.as_str().ok_or("MQTT publish topic must be text")?;
    let payload = if matches!(method, Method::Json) {
        match args.get(1) {
            Some(value) => {
                let value = value.to_script_json();
                let mut writer = Output {
                    bytes: Vec::with_capacity(limit),
                    limit,
                };
                serde_json::to_writer(&mut writer, &value)
                    .map_err(|error| format!("MQTT JSON encoding failed: {error}"))?;
                writer.bytes
            }
            None => Vec::new(),
        }
    } else {
        let Some(value) = args.get(1) else {
            return request(topic, Vec::new(), args, method);
        };
        if text_bound(value) > limit {
            return Err("MQTT text payload exceeds its admitted output bound".into());
        }
        value.as_str().unwrap_or_default().into_bytes()
    };
    request(topic, payload, args, method)
}

fn request(
    topic: String,
    mut payload: Vec<u8>,
    args: &[ParamValue],
    method: Method,
) -> Result<MqttPublishRequest, String> {
    payload.shrink_to_fit();
    Ok(MqttPublishRequest {
        topic,
        payload,
        qos: qos(args.get(2))?,
        retain: args.get(3).and_then(ParamValue::as_bool).unwrap_or(false),
        description: method.name().to_owned(),
    })
}

fn qos(value: Option<&ParamValue>) -> Result<MqttQos, String> {
    let Some(value) = value else { return Ok(MqttQos::AtMost) };
    if let Some(value) = value.as_int() {
        return match value {
            0 => Ok(MqttQos::AtMost),
            1 => Ok(MqttQos::AtLeast),
            2 => Ok(MqttQos::Exactly),
            _ => Err("MQTT QoS integer must be 0, 1, or 2".into()),
        };
    }
    if text_bound(value) > 32 {
        return Err("MQTT QoS text exceeds 32 bytes".into());
    }
    value
        .as_str()
        .and_then(|variant| MqttQos::from_variant(&variant))
        .ok_or_else(|| "Invalid MQTT QoS".into())
}

struct Output {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|length| *length <= self.limit)
            .ok_or_else(|| io::Error::other("MQTT JSON output exceeds its admitted bound"))?;
        debug_assert!(length <= self.bytes.capacity());
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
