use super::*;
use crate::app::module::common::buttplug::*;
use crate::app::module_command::{CapturedInvocation, ModuleCommandExecuteEvent};
use golden_core::{
    engine::{EngineTime, NodeWaker},
    node::NodeId,
    parameter::ParamValue,
    process_ctx::ProcessTreeSnapshot,
    script::ScriptArguments,
};
use golden_io::{RetirementPermit, RevisionGuard};

pub(in super::super) enum Input {
    Command(CapturedInvocation),
    Script {
        module: NodeId,
        method: &'static str,
        args: ScriptArguments,
        snapshot: RetainedPayload<ProcessTreeSnapshot>,
    },
    #[cfg(test)]
    Prepared(ButtplugWorkerCommand),
}
pub(in super::super) struct Source {
    pub(in super::super) input: Input,
    pub(in super::super) time: EngineTime,
    pub(super) origin: Origin,
}
pub(super) enum Origin {
    Engine {
        producer: NodeWaker,
        guards: [RevisionGuard; 3],
        _permit: RetirementPermit,
    },
    #[cfg(test)]
    Fixture,
}
impl Source {
    pub(in super::super) fn live(&self) -> bool {
        match &self.origin {
            Origin::Engine { producer, guards, .. } => {
                !producer.is_closed() && guards.iter().all(RevisionGuard::is_current)
            }
            #[cfg(test)]
            Origin::Fixture => true,
        }
    }
    pub(in super::super) fn weight(&self) -> usize {
        match &self.input {
            Input::Command(value) => 64 * 1024 + 3 * value.override_weight,
            Input::Script { .. } => 64 * 1024,
            #[cfg(test)]
            Input::Prepared(_) => COMMAND_WEIGHT,
        }
    }
    pub(super) fn prepare(&self) -> Result<ButtplugWorkerCommand, String> {
        if !self.live() {
            return Err("Buttplug source retired; request was not sent".into());
        }
        let (module, snapshot, mut request) = match &self.input {
            Input::Command(invocation) => (invocation.module, &*invocation.snapshot, command(invocation)?),
            Input::Script {
                module,
                method,
                args,
                snapshot,
            } => {
                if args.len() > 4 || args.retained_bytes() > 64 * 1024 {
                    return Err("Buttplug scripts accept four arguments / 64 KiB".into());
                }
                (*module, &**snapshot, script(method, args)?)
            }
            #[cfg(test)]
            Input::Prepared(request) => return Ok(request.clone()),
        };
        let target = match &mut request {
            ButtplugWorkerCommand::StopDevice { target } => Some(target),
            ButtplugWorkerCommand::SetOutput(request) => Some(&mut request.target),
            _ => None,
        };
        if let Some(target) = target {
            if target.trim().is_empty() || target.eq_ignore_ascii_case(BUTTPLUG_TARGET_SELECTED) {
                let connection = snapshot
                    .find_child_by_decl_id(module, "connection")
                    .ok_or("Missing Buttplug connection")?;
                *target = text(child(snapshot, connection, "device"), 1024, "selected device")?.to_owned();
            }
        }
        if let ButtplugWorkerCommand::SetOutput(request) = &mut request {
            let parameters = snapshot
                .find_child_by_decl_id(module, "parameters")
                .ok_or("Missing Buttplug parameters")?;
            let max = number(child(snapshot, parameters, "max_output"), 1.0)?.clamp(0.0, 1.0);
            if !request.value.is_finite() || !max.is_finite() {
                return Err("Buttplug output must be finite".into());
            }
            request.value = request.value.clamp(0.0, max);
        }
        admission::command(&request)?;
        Ok(request)
    }
}
fn child<'a>(snapshot: &'a ProcessTreeSnapshot, parent: NodeId, name: &str) -> Option<&'a ParamValue> {
    snapshot
        .find_child_by_decl_id(parent, name)
        .and_then(|id| snapshot.node(id)?.param_value.as_ref())
}
fn text<'a>(value: Option<&'a ParamValue>, limit: usize, name: &str) -> Result<&'a str, String> {
    match value {
        Some(ParamValue::Str(value) | ParamValue::Enum(value) | ParamValue::File(value)) if value.len() <= limit => {
            Ok(value)
        }
        _ => Err(format!("Buttplug {name} must be text within {limit} bytes")),
    }
}
fn number(value: Option<&ParamValue>, default: f64) -> Result<f64, String> {
    let Some(value) = value else { return Ok(default) };
    if matches!(value,ParamValue::Str(value)|ParamValue::Enum(value)|ParamValue::File(value) if value.len()>32) {
        return Err("Buttplug numeric text exceeds 32 bytes".into());
    }
    value
        .as_float()
        .or_else(|| value.as_int().map(f64::from))
        .ok_or("Buttplug value must be numeric".into())
}
fn duration(value: Option<&ParamValue>) -> Result<u32, String> {
    if matches!(value,Some(ParamValue::Str(value)|ParamValue::Enum(value)|ParamValue::File(value)) if value.len()>32) {
        return Err("Buttplug duration text exceeds 32 bytes".into());
    }
    Ok(value
        .and_then(ParamValue::as_int)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(1000))
}
fn command(invocation: &CapturedInvocation) -> Result<ButtplugWorkerCommand, String> {
    let execution = invocation
        .execution
        .as_ref()
        .map(|value| {
            serde_json::from_value::<ModuleCommandExecuteEvent>((**value).clone()).map_err(|error| error.to_string())
        })
        .transpose()?;
    if execution
        .as_ref()
        .is_some_and(|value| value.command_id != invocation.command)
    {
        return Err("Buttplug invocation targets another command".into());
    }
    let value = |name| {
        let id = invocation.snapshot.find_child_by_decl_id(invocation.command, name)?;
        execution
            .as_ref()
            .and_then(|value| value.param_overrides.iter().rev().find(|entry| entry.param_id == id))
            .map(|entry| &entry.value)
            .or_else(|| invocation.snapshot.node(id)?.param_value.as_ref())
    };
    let target = || text(value("target"), 1024, "target").map(str::to_owned);
    match invocation
        .snapshot
        .node(invocation.command)
        .map(|node| node.node_type.as_str())
    {
        Some(BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE) => Ok(ButtplugWorkerCommand::StartScanning),
        Some(BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE) => Ok(ButtplugWorkerCommand::StopScanning),
        Some(BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE) => Ok(ButtplugWorkerCommand::StopAllDevices),
        Some(BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE) => Ok(ButtplugWorkerCommand::StopDevice { target: target()? }),
        Some(BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE) => Ok(ButtplugWorkerCommand::SetOutput(ButtplugSetOutputRequest {
            target: target()?,
            output: text(value("output"), 64, "output")?.to_owned(),
            value: number(value("value"), 0.0)?,
            duration_ms: duration(value("duration_ms"))?,
            description: "set output".into(),
        })),
        _ => Err("Unsupported Buttplug command type".into()),
    }
}
fn script(method: &str, args: &[ParamValue]) -> Result<ButtplugWorkerCommand, String> {
    let target = |index| {
        args.get(index)
            .map(|value| text(Some(value), 1024, "target"))
            .transpose()
            .map(|value| value.unwrap_or(BUTTPLUG_TARGET_SELECTED).trim().to_owned())
    };
    let duration = |index| duration(args.get(index));
    let (output, value, target, duration_ms) = match method {
        "startScanning" => return Ok(ButtplugWorkerCommand::StartScanning),
        "stopScanning" => return Ok(ButtplugWorkerCommand::StopScanning),
        "stopAllDevices" | "stopAll" => return Ok(ButtplugWorkerCommand::StopAllDevices),
        "stopDevice" => return Ok(ButtplugWorkerCommand::StopDevice { target: target(0)? }),
        "setOutput" => (
            text(args.first(), 64, "output")?,
            number(args.get(1), 0.0)?,
            target(2)?,
            duration(3)?,
        ),
        "positionWithDuration" => (
            BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION,
            number(args.first(), 0.0)?,
            target(2)?,
            duration(1)?,
        ),
        "vibrate" | "rotate" | "oscillate" | "position" => {
            (method, number(args.first(), 0.0)?, target(1)?, duration(2)?)
        }
        _ => return Err("Unknown Buttplug method".into()),
    };
    if args.is_empty() || method == "setOutput" && args.len() < 2 {
        return Err("Buttplug output method requires a value".into());
    }
    Ok(ButtplugWorkerCommand::SetOutput(ButtplugSetOutputRequest {
        target,
        output: output.into(),
        value,
        duration_ms,
        description: "set output".into(),
    }))
}
