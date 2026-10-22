use super::*;
use crate::app::module::common::{mqtt::MqttQos, streaming::parser::decode_text_escape_sequences};
use crate::app::module_command::ModuleCommandExecuteEvent;

pub(super) fn prepare(invocation: &CapturedInvocation, limit: usize) -> Result<MqttPublishRequest, String> {
    let execution = invocation
        .execution
        .as_ref()
        .map(|value| {
            serde_json::from_value::<ModuleCommandExecuteEvent>((**value).clone())
                .map_err(|error| format!("Invalid MQTT command invocation: {error}"))
        })
        .transpose()?;
    if execution
        .as_ref()
        .is_some_and(|event| event.command_id != invocation.command)
    {
        return Err("MQTT invocation targets another command".into());
    }
    // Borrow only the four consumed fields. A whole-snapshot overlay would clone
    // replaced parameter values before discarding them, outside the output budget.
    let value = |name| {
        let id = invocation.snapshot.find_child_by_decl_id(invocation.command, name)?;
        execution
            .as_ref()
            .and_then(|event| event.param_overrides.iter().rev().find(|value| value.param_id == id))
            .map(|value| &value.value)
            .or_else(|| invocation.snapshot.node(id)?.param_value.as_ref())
    };
    let topic = text(value("topic"), "topic", MAX_TOPIC)?.to_owned();
    let source = text(value("payload"), "payload", MAX_PACKET)?;
    if source.len() > limit {
        return Err("MQTT command payload exceeds its admitted output bound".into());
    }
    let mut payload = decode_text_escape_sequences(source).into_bytes();
    payload.shrink_to_fit();
    let qos = MqttQos::from_variant(text(value("qos"), "qos", 32)?).ok_or("Invalid MQTT command QoS")?;
    let retain = value("retain")
        .and_then(ParamValue::as_bool)
        .ok_or("MQTT Retain must be boolean")?;
    Ok(MqttPublishRequest {
        topic,
        payload,
        qos,
        retain,
        description: "publish".into(),
    })
}
fn text<'a>(value: Option<&'a ParamValue>, name: &str, limit: usize) -> Result<&'a str, String> {
    match value {
        Some(ParamValue::Str(text) | ParamValue::Enum(text) | ParamValue::File(text)) if text.len() <= limit => {
            Ok(text)
        }
        _ => Err(format!("MQTT command field {name} must be text within its byte limit")),
    }
}
