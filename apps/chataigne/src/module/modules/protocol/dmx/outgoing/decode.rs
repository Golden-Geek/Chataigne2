use super::*;
use crate::app::module_command::ModuleCommandExecuteEvent;

pub(super) fn prepare(source: RetainedPayload<Source>) -> Result<Prepared, String> {
    if !source.is_live() {
        return Err("DMX source or session retired before preparation; request was not sent".into());
    }
    let operation = match &source.input {
        Input::Command(invocation) => command(invocation)?,
        Input::Script { method, args } => match *method {
            "setChannel" => script_set_channel(args)?,
            "sendFrame" => script_send_frame(args)?,
            "blackout" => script_blackout(args)?,
            _ => unreachable!(),
        },
        Input::Property(text) => DmxCommandRequest::SendFrame {
            slots: parse_slots_json(text)?,
        },
    };
    let baseline = if matches!(operation, DmxCommandRequest::SetChannel { .. }) {
        let text = source
            .snapshot
            .node(source.output.0)
            .and_then(|node| node.param_value.as_ref());
        Some(parse_slots_json(text_value(text, 8192, "output channels")?)?)
    } else {
        None
    };
    Ok(Prepared {
        source,
        operation,
        baseline,
    })
}

fn command(invocation: &CapturedInvocation) -> Result<DmxCommandRequest, String> {
    let execution = invocation
        .execution
        .as_ref()
        .map(|value| {
            serde_json::from_value::<ModuleCommandExecuteEvent>((**value).clone())
                .map_err(|error| format!("Invalid DMX command invocation: {error}"))
        })
        .transpose()?;
    if execution
        .as_ref()
        .is_some_and(|event| event.command_id != invocation.command)
    {
        return Err("DMX invocation targets another command".into());
    }
    // Resolve only consumed fields; never clone a whole snapshot overlay (which
    // would copy an oversized authored value even when an override replaces it).
    let value = |name| {
        let id = invocation.snapshot.find_child_by_decl_id(invocation.command, name)?;
        execution
            .as_ref()
            .and_then(|event| event.param_overrides.iter().rev().find(|entry| entry.param_id == id))
            .map(|entry| &entry.value)
            .or_else(|| invocation.snapshot.node(id)?.param_value.as_ref())
    };
    let node_type = invocation
        .snapshot
        .node(invocation.command)
        .map(|node| node.node_type.as_str());
    use crate::app::module_modules_protocol_dmx_commands::*;
    match node_type {
        Some(DMX_SET_CHANNEL_COMMAND_NODE_TYPE) => {
            script_set_channel(&[integer(value("channel"), "channel")?, integer(value("value"), "value")?])
        }
        Some(DMX_SEND_FRAME_COMMAND_NODE_TYPE) => parse_slots_json(text_value(value("slots"), 8192, "channels")?)
            .map(|slots| DmxCommandRequest::SendFrame { slots }),
        Some(DMX_BLACKOUT_COMMAND_NODE_TYPE) => Ok(DmxCommandRequest::Blackout),
        _ => Err("Unsupported DMX command type".into()),
    }
}
fn integer(value: Option<&ParamValue>, name: &str) -> Result<ParamValue, String> {
    let value = value.ok_or_else(|| format!("Missing DMX {name}"))?;
    if matches!(value, ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) if text.len() > 32) {
        return Err(format!("DMX {name} numeric text exceeds 32 bytes"));
    }
    value
        .as_int()
        .map(ParamValue::Int)
        .ok_or_else(|| format!("Invalid DMX {name}"))
}
fn text_value<'a>(value: Option<&'a ParamValue>, limit: usize, name: &str) -> Result<&'a str, String> {
    match value {
        Some(ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text)) if text.len() <= limit => {
            Ok(text)
        }
        _ => Err(format!("DMX {name} requires text within {limit} bytes")),
    }
}
