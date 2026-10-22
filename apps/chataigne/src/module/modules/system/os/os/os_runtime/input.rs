use super::*;
use crate::app::module_command::{CapturedInvocation, ModuleCommandExecuteEvent};
use golden_core::{engine::NodeWaker, parameter::ParamValue, script::ScriptArguments};

pub(in super::super) enum Input {
    Command(CapturedInvocation),
    Script { method: Method, args: ScriptArguments },
}
#[derive(Clone, Copy, Debug)]
pub(in super::super) enum Method {
    Shutdown,
    Reboot,
    Logout,
    WakeOnLan,
}
impl Method {
    pub(in super::super) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "shutdown" => Self::Shutdown,
            "reboot" => Self::Reboot,
            "logout" => Self::Logout,
            "wakeOnLan" => Self::WakeOnLan,
            _ => return None,
        })
    }
    pub(in super::super) fn name(self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown",
            Self::Reboot => "reboot",
            Self::Logout => "logout",
            Self::WakeOnLan => "wakeOnLan",
        }
    }
    fn action(self) -> Option<HostControlAction> {
        Some(match self {
            Self::Shutdown => HostControlAction::Shutdown,
            Self::Reboot => HostControlAction::Reboot,
            Self::Logout => HostControlAction::Logout,
            Self::WakeOnLan => return None,
        })
    }
}
pub(in super::super) struct Source {
    pub(in super::super) time: golden_core::engine::EngineTime,
    pub(super) _admission: Option<golden_io::RetirementPermit>,
    pub(in super::super) input: Input,
    pub(in super::super) producer: NodeWaker,
    pub(in super::super) fence: Fence,
    pub(in super::super) method: Method,
}
impl Source {
    pub(in super::super) fn live(&self) -> bool {
        !self.producer.is_closed() && self.fence.current()
    }
    pub(in super::super) fn weight(&self) -> usize {
        WEIGHT
            + match &self.input {
                Input::Command(invocation) => 3 * invocation.override_weight,
                _ => 0,
            }
    }
    pub(in super::super) fn prepare(&self) -> Result<Request, String> {
        if !self.live() {
            return Err("OS source retired; request was not sent".into());
        }
        match &self.input {
            Input::Script { method, args } => {
                if let Some(action) = method.action() {
                    if !args.is_empty() {
                        return Err(format!("{} does not accept arguments", method.name()));
                    }
                    return Ok(Request::Control(action));
                }
                if args.is_empty() || args.len() > 3 {
                    return Err("wakeOnLan expects MAC, optional host and port".into());
                }
                let mac = text(args.first(), 64, "MAC")?.to_owned();
                let host = args
                    .get(1)
                    .map(|value| text(Some(value), 253, "host"))
                    .transpose()?
                    .unwrap_or("255.255.255.255")
                    .to_owned();
                let port = args.get(2).map(integer).transpose()?.unwrap_or(9);
                wake(mac, host, port)
            }
            Input::Command(invocation) => {
                if let Some(action) = self.method.action() {
                    return Ok(Request::Control(action));
                }
                let execution = invocation
                    .execution
                    .as_ref()
                    .map(|value| {
                        serde_json::from_value::<ModuleCommandExecuteEvent>((**value).clone())
                            .map_err(|error| format!("Invalid OS command: {error}"))
                    })
                    .transpose()?;
                if execution
                    .as_ref()
                    .is_some_and(|event| event.command_id != invocation.command)
                {
                    return Err("OS invocation targets another command".into());
                }
                let value = |name| {
                    let id = invocation.snapshot.find_child_by_decl_id(invocation.command, name)?;
                    execution
                        .as_ref()
                        .and_then(|event| event.param_overrides.iter().rev().find(|entry| entry.param_id == id))
                        .map(|entry| &entry.value)
                        .or_else(|| invocation.snapshot.node(id)?.param_value.as_ref())
                };
                wake(
                    text(value("mac_address"), 64, "MAC")?.to_owned(),
                    text(value("broadcast_host"), 253, "host")?.to_owned(),
                    integer(value("port").ok_or("Missing OS port")?)?,
                )
            }
        }
    }
}
fn text<'a>(value: Option<&'a ParamValue>, max: usize, name: &str) -> Result<&'a str, String> {
    match value {
        Some(ParamValue::Str(value) | ParamValue::Enum(value) | ParamValue::File(value)) if value.len() <= max => {
            Ok(value)
        }
        _ => Err(format!("OS {name} requires text within {max} bytes")),
    }
}
fn integer(value: &ParamValue) -> Result<i32, String> {
    if matches!(value, ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) if text.len() > 32) {
        return Err("OS numeric text exceeds 32 bytes".into());
    }
    value.as_int().ok_or_else(|| "OS port must be an integer".into())
}
fn wake(mac_address: String, broadcast_host: String, port: i32) -> Result<Request, String> {
    native::parse_mac_address(&mac_address)?;
    if broadcast_host.trim().is_empty() {
        return Err("Wake-on-LAN host cannot be empty".into());
    }
    Ok(Request::WakeOnLan(WakeOnLanRequest {
        mac_address,
        broadcast_host,
        port: u16::try_from(port).map_err(|_| "Wake-on-LAN port must be within 0..65535")?,
        description: "send Wake-on-LAN packet".into(),
    }))
}
#[derive(Clone, Debug)]
pub(in super::super) enum Request {
    Control(HostControlAction),
    WakeOnLan(WakeOnLanRequest),
}
impl Request {
    pub(in super::super) fn details(&self) -> serde_json::Value {
        match self {
            Self::Control(action) => serde_json::json!({"description": format!("{} host", action.as_human_label())}),
            Self::WakeOnLan(request) => {
                serde_json::json!({"description":request.description,"macAddress":request.mac_address,"broadcastHost":request.broadcast_host,"port":request.port})
            }
        }
    }
}
