use super::*;
use crate::app::module_command::{CapturedInvocation, ModuleCommandExecuteEvent};
use golden_core::{
    engine::{EngineTime, NodeWaker},
    node::NodeId,
    parameter::ParamValue,
    process_ctx::ProcessTreeSnapshot,
    script::ScriptArguments,
};

pub(in super::super) enum Input {
    Command(CapturedInvocation),
    Script {
        module: NodeId,
        method: &'static str,
        args: ScriptArguments,
        snapshot: RetainedPayload<ProcessTreeSnapshot>,
    },
    Running {
        snapshot: RetainedPayload<ProcessTreeSnapshot>,
        item: NodeId,
        desired: bool,
    },
}
pub(in super::super) struct Source {
    pub(in super::super) input: Input,
    pub(in super::super) producer: NodeWaker,
    pub(in super::super) time: EngineTime,
    pub(in super::super) fence: Fence,
    pub(super) _permit: golden_io::RetirementPermit,
}
#[derive(Clone, Debug)]
pub(in super::super) enum Request {
    Launch(LaunchProcessRequest, Option<String>),
    Kill(KillProcessRequest, Option<String>),
    Window(WindowControlRequest, Option<String>),
    Running(String, bool),
}
impl Source {
    pub(in super::super) fn live(&self) -> bool {
        !self.producer.is_closed() && self.fence.current()
    }
    pub(super) fn weight(&self) -> usize {
        COMMAND_WEIGHT
            + match &self.input {
                Input::Command(value) => 3 * value.override_weight,
                _ => 0,
            }
    }
    pub(in super::super) fn method(&self) -> &'static str {
        match &self.input {
            Input::Script { method, .. } => match *method {
                "killProcess" => "kill",
                "controlWindow" => "controlWindow",
                _ => "launch",
            },
            Input::Running { .. } => "setRunning",
            Input::Command(value) => match value.snapshot.node(value.command).map(|node| node.node_type.as_str()) {
                Some("app_control_kill_process_command") => "kill",
                Some("app_control_window_control_command") => "controlWindow",
                _ => "launch",
            },
        }
    }
    pub(super) fn prepare(&self) -> Result<Request, String> {
        if !self.live() {
            return Err("App Control source retired; request was not sent".into());
        }
        let (module, snapshot, request) = match &self.input {
            Input::Running {
                snapshot,
                item,
                desired,
            } => {
                let path = text(
                    snapshot.node(*item).and_then(|node| node.param_value.as_ref()),
                    4096,
                    "watched target",
                )?;
                return Ok(Request::Running(path.to_owned(), *desired));
            }
            Input::Script {
                module,
                method,
                args,
                snapshot,
            } => {
                if args.len() > 9 || args.retained_bytes() > 64 * 1024 {
                    return Err("App Control script input exceeds 9 arguments / 64 KiB".into());
                }
                for value in args.iter() {
                    if matches!(value,ParamValue::Str(text)|ParamValue::Enum(text)|ParamValue::File(text) if text.len()>8192)
                    {
                        return Err("App Control text argument exceeds 8192 bytes".into());
                    }
                }
                use super::super::script_requests::*;
                let request = match *method {
                    "launchWatchedApp" => Raw::Launch(script_launch_watched_app_request(args)?),
                    "launchApp" => Raw::Launch(script_launch_app_request(args)?),
                    "launchCommandLine" => Raw::Launch(script_launch_command_line_request(args)?),
                    "killProcess" => Raw::Kill(script_kill_request(args)?),
                    "controlWindow" => Raw::Window(script_window_control_request(args)?),
                    _ => return Err("Unknown App Control method".into()),
                };
                (*module, &**snapshot, request)
            }
            Input::Command(value) => (value.module, &*value.snapshot, command(value)?),
        };
        match request {
            Raw::Launch(value) => {
                let target = if value.mode == LaunchMode::WatchedApp {
                    Some(resolve(snapshot, module, &value.watched_app)?)
                } else {
                    None
                };
                Ok(Request::Launch(value, target))
            }
            Raw::Kill(value) => {
                let target = if value.target_source == CommandTargetSource::WatchedApp {
                    Some(resolve(snapshot, module, &value.target)?)
                } else {
                    None
                };
                Ok(Request::Kill(value, target))
            }
            Raw::Window(value) => {
                let target = if value.target_source == CommandTargetSource::WatchedApp {
                    Some(resolve(snapshot, module, &value.target)?)
                } else {
                    None
                };
                Ok(Request::Window(value, target))
            }
        }
    }
}
enum Raw {
    Launch(LaunchProcessRequest),
    Kill(KillProcessRequest),
    Window(WindowControlRequest),
}
fn command(invocation: &CapturedInvocation) -> Result<Raw, String> {
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
        return Err("App Control invocation targets another command".into());
    }
    let value = |name| {
        let id = invocation.snapshot.find_child_by_decl_id(invocation.command, name)?;
        execution
            .as_ref()
            .and_then(|value| value.param_overrides.iter().rev().find(|entry| entry.param_id == id))
            .map(|entry| &entry.value)
            .or_else(|| invocation.snapshot.node(id)?.param_value.as_ref())
    };
    let string = |name| text(value(name), 8192, name).map(str::to_owned);
    let number = |name, default| -> Result<i32, String> {
        let Some(value) = value(name) else {
            return Ok(default);
        };
        if matches!(value,ParamValue::Str(text)|ParamValue::Enum(text)|ParamValue::File(text) if text.len()>32) {
            return Err("App Control numeric text exceeds 32 bytes".into());
        }
        value.as_int().ok_or_else(|| format!("Invalid App Control {name}"))
    };
    let boolean = |name, default| value(name).and_then(ParamValue::as_bool).unwrap_or(default);
    let kind = invocation
        .snapshot
        .node(invocation.command)
        .map(|node| node.node_type.as_str());
    if kind == Some("app_control_launch_process_command") {
        let mode = LaunchMode::from_variant(&string("mode")?).ok_or("Invalid launch mode")?;
        // Params DSL dependencies physically omit inactive fields. Consume the
        // selected branch rather than requiring dormant controls to exist.
        return Ok(Raw::Launch(LaunchProcessRequest {
            mode,
            watched_app: if mode == LaunchMode::WatchedApp {
                string("watched_app")?
            } else {
                String::new()
            },
            executable_path: if mode == LaunchMode::Executable {
                string("executable_path")?
            } else {
                String::new()
            },
            arguments: if mode != LaunchMode::CommandLine {
                string("arguments")?
            } else {
                String::new()
            },
            working_directory: string("working_directory")?,
            command_line: if mode == LaunchMode::CommandLine {
                string("command_line")?
            } else {
                String::new()
            },
        }));
    }
    let source = CommandTargetSource::from_variant(&string("target_source")?).ok_or("Invalid target source")?;
    let target = string(if source == CommandTargetSource::WatchedApp {
        "watched_app"
    } else {
        "target"
    })?;
    let mode = if source == CommandTargetSource::FreeProcess {
        ProcessMatchMode::from_variant(&string("match_mode")?).ok_or("Invalid process match mode")?
    } else {
        ProcessMatchMode::Exact
    };
    match kind {
        Some("app_control_kill_process_command") => Ok(Raw::Kill(KillProcessRequest {
            target_source: source,
            target,
            match_mode: mode,
            hard_kill: boolean("hard_kill", false),
        })),
        Some("app_control_window_control_command") => Ok(Raw::Window(WindowControlRequest {
            target_source: source,
            target,
            match_mode: mode,
            action: crate::app::module::common::app_control::WindowAction::from_variant(&string("action")?)
                .ok_or("Invalid window action")?,
            x: number("x", 0)?,
            y: number("y", 0)?,
            width: number("width", 1280)?,
            height: number("height", 720)?,
            always_on_top: boolean("always_on_top", true),
        })),
        _ => Err("Unknown App Control command".into()),
    }
}
fn text<'a>(value: Option<&'a ParamValue>, max: usize, name: &str) -> Result<&'a str, String> {
    match value {
        Some(ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value)) if value.len() <= max => {
            Ok(value)
        }
        _ => Err(format!("App Control {name} must be text within {max} bytes")),
    }
}
fn resolve(snapshot: &ProcessTreeSnapshot, module: NodeId, label: &str) -> Result<String, String> {
    let parameters = snapshot
        .find_child_by_decl_id(module, "parameters")
        .ok_or("Missing App Control parameters")?;
    let root = snapshot
        .find_child_by_decl_id(parameters, "watched_apps_targets")
        .ok_or("Missing watched apps")?;
    if snapshot.node(root).is_none_or(|node| node.child_count > 64) {
        return Err("App Control supports at most 64 watch targets".into());
    }
    for id in snapshot.child_ids(root) {
        let Some(node) = snapshot.node(id) else {
            continue;
        };
        let path = text(node.param_value.as_ref(), 4096, "watched path")?;
        if node.label.eq_ignore_ascii_case(label) || path.eq_ignore_ascii_case(label) {
            return Ok(path.to_owned());
        }
    }
    Err(format!(
        "No watched app matches '{}'",
        label.chars().take(128).collect::<String>()
    ))
}
pub(super) fn validate_config(config: &Config) -> Result<(), String> {
    if config.apps.len() + config.folders.len() > 64 {
        return Err("App Control supports 64 combined watch targets".into());
    }
    if config
        .apps
        .iter()
        .any(|entry| entry.label.len() > 128 || entry.target_path.len() > 4096)
        || config
            .folders
            .iter()
            .any(|entry| entry.label.len() > 128 || entry.target_path.len() > 4096)
    {
        return Err("App Control watch labels/paths exceed 128 / 4096 bytes".into());
    }
    Ok(())
}
