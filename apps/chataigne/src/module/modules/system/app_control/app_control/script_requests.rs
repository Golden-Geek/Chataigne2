use super::*;

pub(super) fn script_launch_watched_app_request(args: &[ParamValue]) -> Result<LaunchProcessRequest, String> {
    Ok(LaunchProcessRequest {
        mode: LaunchMode::WatchedApp,
        watched_app: required_script_string(args, 0, "watched app target")?,
        executable_path: String::new(),
        arguments: optional_script_string(args, 1),
        working_directory: optional_script_string(args, 2),
        command_line: String::new(),
    })
}

pub(super) fn script_launch_app_request(args: &[ParamValue]) -> Result<LaunchProcessRequest, String> {
    Ok(LaunchProcessRequest {
        mode: LaunchMode::Executable,
        watched_app: String::new(),
        executable_path: required_script_string(args, 0, "application path")?,
        arguments: optional_script_string(args, 1),
        working_directory: optional_script_string(args, 2),
        command_line: String::new(),
    })
}

pub(super) fn script_launch_command_line_request(args: &[ParamValue]) -> Result<LaunchProcessRequest, String> {
    Ok(LaunchProcessRequest {
        mode: LaunchMode::CommandLine,
        watched_app: String::new(),
        executable_path: String::new(),
        arguments: String::new(),
        working_directory: optional_script_string(args, 1),
        command_line: required_script_string(args, 0, "command line")?,
    })
}

pub(super) fn script_kill_request(args: &[ParamValue]) -> Result<KillProcessRequest, String> {
    let match_mode = parse_script_match_mode(args.get(1))?;
    Ok(KillProcessRequest {
        target_source: if optional_script_bool(args, 3, false) {
            CommandTargetSource::WatchedApp
        } else {
            CommandTargetSource::FreeProcess
        },
        target: required_script_string(args, 0, "process target")?,
        match_mode,
        hard_kill: optional_script_bool(args, 2, false),
    })
}

pub(super) fn script_window_control_request(args: &[ParamValue]) -> Result<WindowControlRequest, String> {
    let action_name = required_script_string(args, 1, "window action")?;
    let Some(action) = WindowAction::from_variant(action_name.as_str()) else {
        return Err(format!("unsupported window action '{action_name}'"));
    };

    Ok(WindowControlRequest {
        target_source: if optional_script_bool(args, 3, false) {
            CommandTargetSource::WatchedApp
        } else {
            CommandTargetSource::FreeProcess
        },
        target: required_script_string(args, 0, "window target")?,
        match_mode: parse_script_match_mode(args.get(2))?,
        action,
        x: optional_script_int(args, 4, 0),
        y: optional_script_int(args, 5, 0),
        width: optional_script_int(args, 6, 1280),
        height: optional_script_int(args, 7, 720),
        always_on_top: optional_script_bool(args, 8, true),
    })
}

pub(super) fn parse_script_match_mode(value: Option<&ParamValue>) -> Result<ProcessMatchMode, String> {
    let variant = value
        .and_then(ParamValue::as_str)
        .unwrap_or_else(|| APP_CONTROL_MATCH_MODE_EXACT.to_string());
    ProcessMatchMode::from_variant(variant.as_str())
        .ok_or_else(|| format!("unsupported process match mode '{variant}'"))
}

pub(super) fn required_script_string(args: &[ParamValue], index: usize, label: &str) -> Result<String, String> {
    args.get(index)
        .and_then(ParamValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("App Control script method expects a non-empty {label}"))
}

pub(super) fn optional_script_string(args: &[ParamValue], index: usize) -> String {
    args.get(index).and_then(ParamValue::as_str).unwrap_or_default()
}

pub(super) fn optional_script_bool(args: &[ParamValue], index: usize, default: bool) -> bool {
    args.get(index)
        .and_then(ParamValue::as_bool)
        .unwrap_or(default)
}

pub(super) fn optional_script_int(args: &[ParamValue], index: usize, default: i32) -> i32 {
    args.get(index)
        .and_then(ParamValue::as_int)
        .unwrap_or(default)
}
