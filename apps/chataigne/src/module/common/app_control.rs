use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterEnumOption},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

pub(crate) const APP_CONTROL_TARGET_SOURCE_WATCHED_APP: &str = "watched_app";
pub(crate) const APP_CONTROL_TARGET_SOURCE_FREE_PROCESS: &str = "free_process";

pub(crate) const APP_CONTROL_LAUNCH_MODE_WATCHED_APP: &str = "watched_app";
pub(crate) const APP_CONTROL_LAUNCH_MODE_EXECUTABLE: &str = "executable";
pub(crate) const APP_CONTROL_LAUNCH_MODE_COMMAND_LINE: &str = "command_line";

pub(crate) const APP_CONTROL_MATCH_MODE_EXACT: &str = "exact";
pub(crate) const APP_CONTROL_MATCH_MODE_CONTAINS: &str = "contains";
pub(crate) const APP_CONTROL_MATCH_MODE_STARTS_WITH: &str = "starts_with";
pub(crate) const APP_CONTROL_MATCH_MODE_ENDS_WITH: &str = "ends_with";

pub(crate) const APP_CONTROL_WINDOW_ACTION_MOVE: &str = "move";
pub(crate) const APP_CONTROL_WINDOW_ACTION_RESIZE: &str = "resize";
pub(crate) const APP_CONTROL_WINDOW_ACTION_BOUNDS: &str = "bounds";
pub(crate) const APP_CONTROL_WINDOW_ACTION_MINIMIZE: &str = "minimize";
pub(crate) const APP_CONTROL_WINDOW_ACTION_MAXIMIZE: &str = "maximize";
pub(crate) const APP_CONTROL_WINDOW_ACTION_RESTORE: &str = "restore";
pub(crate) const APP_CONTROL_WINDOW_ACTION_TRAY: &str = "tray";
pub(crate) const APP_CONTROL_WINDOW_ACTION_SHOW: &str = "show";
pub(crate) const APP_CONTROL_WINDOW_ACTION_ALWAYS_ON_TOP: &str = "always_on_top";

pub(crate) const APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE: &str = "app_control_launch_process_command";
pub(crate) const APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE: &str = "app_control_kill_process_command";
pub(crate) const APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE: &str = "app_control_window_control_command";
pub(crate) const APP_CONTROL_MODULE_COMMAND_TYPES: &[&str] = &[
    APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE,
    APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE,
    APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE,
];
pub(crate) const MISSING_WATCHED_APP_WARNING_ID: &str = "app_control_missing_watched_app";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommandTargetSource {
    #[default]
    WatchedApp,
    FreeProcess,
}

impl CommandTargetSource {
    pub(crate) fn from_variant(variant: &str) -> Option<Self> {
        match normalized_variant(variant).as_str() {
            APP_CONTROL_TARGET_SOURCE_WATCHED_APP => Some(Self::WatchedApp),
            APP_CONTROL_TARGET_SOURCE_FREE_PROCESS => Some(Self::FreeProcess),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WatchedApp => APP_CONTROL_TARGET_SOURCE_WATCHED_APP,
            Self::FreeProcess => APP_CONTROL_TARGET_SOURCE_FREE_PROCESS,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LaunchMode {
    #[default]
    WatchedApp,
    Executable,
    CommandLine,
}

impl LaunchMode {
    pub(crate) fn from_variant(variant: &str) -> Option<Self> {
        match normalized_variant(variant).as_str() {
            APP_CONTROL_LAUNCH_MODE_WATCHED_APP => Some(Self::WatchedApp),
            APP_CONTROL_LAUNCH_MODE_EXECUTABLE => Some(Self::Executable),
            APP_CONTROL_LAUNCH_MODE_COMMAND_LINE => Some(Self::CommandLine),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WatchedApp => APP_CONTROL_LAUNCH_MODE_WATCHED_APP,
            Self::Executable => APP_CONTROL_LAUNCH_MODE_EXECUTABLE,
            Self::CommandLine => APP_CONTROL_LAUNCH_MODE_COMMAND_LINE,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProcessMatchMode {
    #[default]
    Exact,
    Contains,
    StartsWith,
    EndsWith,
}

impl ProcessMatchMode {
    pub(crate) fn from_variant(variant: &str) -> Option<Self> {
        match normalized_variant(variant).as_str() {
            APP_CONTROL_MATCH_MODE_EXACT => Some(Self::Exact),
            APP_CONTROL_MATCH_MODE_CONTAINS => Some(Self::Contains),
            APP_CONTROL_MATCH_MODE_STARTS_WITH | "startwith" | "startswith" => Some(Self::StartsWith),
            APP_CONTROL_MATCH_MODE_ENDS_WITH | "endwith" | "endswith" | "endwidth" => Some(Self::EndsWith),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Exact => APP_CONTROL_MATCH_MODE_EXACT,
            Self::Contains => APP_CONTROL_MATCH_MODE_CONTAINS,
            Self::StartsWith => APP_CONTROL_MATCH_MODE_STARTS_WITH,
            Self::EndsWith => APP_CONTROL_MATCH_MODE_ENDS_WITH,
        }
    }

    pub(crate) fn matches(self, haystack: &str, needle: &str) -> bool {
        let haystack = haystack.trim().to_ascii_lowercase();
        let needle = needle.trim().to_ascii_lowercase();
        if haystack.is_empty() || needle.is_empty() {
            return false;
        }

        match self {
            Self::Exact => haystack == needle,
            Self::Contains => haystack.contains(needle.as_str()),
            Self::StartsWith => haystack.starts_with(needle.as_str()),
            Self::EndsWith => haystack.ends_with(needle.as_str()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WindowAction {
    #[default]
    Move,
    Resize,
    Bounds,
    Minimize,
    Maximize,
    Restore,
    Tray,
    Show,
    AlwaysOnTop,
}

impl WindowAction {
    pub(crate) fn from_variant(variant: &str) -> Option<Self> {
        match normalized_variant(variant).as_str() {
            APP_CONTROL_WINDOW_ACTION_MOVE => Some(Self::Move),
            APP_CONTROL_WINDOW_ACTION_RESIZE => Some(Self::Resize),
            APP_CONTROL_WINDOW_ACTION_BOUNDS => Some(Self::Bounds),
            APP_CONTROL_WINDOW_ACTION_MINIMIZE => Some(Self::Minimize),
            APP_CONTROL_WINDOW_ACTION_MAXIMIZE => Some(Self::Maximize),
            APP_CONTROL_WINDOW_ACTION_RESTORE => Some(Self::Restore),
            APP_CONTROL_WINDOW_ACTION_TRAY | "hide" => Some(Self::Tray),
            APP_CONTROL_WINDOW_ACTION_SHOW => Some(Self::Show),
            APP_CONTROL_WINDOW_ACTION_ALWAYS_ON_TOP | "topmost" => Some(Self::AlwaysOnTop),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Move => APP_CONTROL_WINDOW_ACTION_MOVE,
            Self::Resize => APP_CONTROL_WINDOW_ACTION_RESIZE,
            Self::Bounds => APP_CONTROL_WINDOW_ACTION_BOUNDS,
            Self::Minimize => APP_CONTROL_WINDOW_ACTION_MINIMIZE,
            Self::Maximize => APP_CONTROL_WINDOW_ACTION_MAXIMIZE,
            Self::Restore => APP_CONTROL_WINDOW_ACTION_RESTORE,
            Self::Tray => APP_CONTROL_WINDOW_ACTION_TRAY,
            Self::Show => APP_CONTROL_WINDOW_ACTION_SHOW,
            Self::AlwaysOnTop => APP_CONTROL_WINDOW_ACTION_ALWAYS_ON_TOP,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct LaunchProcessRequest {
    pub watched_app: String,
    pub executable_path: String,
    pub arguments: String,
    pub working_directory: String,
    pub command_line: String,
    pub mode: LaunchMode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct KillProcessRequest {
    pub target_source: CommandTargetSource,
    pub target: String,
    pub match_mode: ProcessMatchMode,
    pub hard_kill: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct WindowControlRequest {
    pub target_source: CommandTargetSource,
    pub target: String,
    pub match_mode: ProcessMatchMode,
    pub action: WindowAction,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub always_on_top: bool,
}

pub(crate) fn target_source_enum_options() -> Vec<ParameterEnumOption> {
    enum_options(&[
        (APP_CONTROL_TARGET_SOURCE_WATCHED_APP, "Watched App"),
        (APP_CONTROL_TARGET_SOURCE_FREE_PROCESS, "Free Process"),
    ])
}

pub(crate) fn launch_mode_enum_options() -> Vec<ParameterEnumOption> {
    enum_options(&[
        (APP_CONTROL_LAUNCH_MODE_WATCHED_APP, "Watched App"),
        (APP_CONTROL_LAUNCH_MODE_EXECUTABLE, "Executable"),
        (APP_CONTROL_LAUNCH_MODE_COMMAND_LINE, "Command Line"),
    ])
}

pub(crate) fn match_mode_enum_options() -> Vec<ParameterEnumOption> {
    enum_options(&[
        (APP_CONTROL_MATCH_MODE_EXACT, "Exact"),
        (APP_CONTROL_MATCH_MODE_CONTAINS, "Contains"),
        (APP_CONTROL_MATCH_MODE_STARTS_WITH, "Starts With"),
        (APP_CONTROL_MATCH_MODE_ENDS_WITH, "Ends With"),
    ])
}

pub(crate) fn window_action_enum_options() -> Vec<ParameterEnumOption> {
    enum_options(&[
        (APP_CONTROL_WINDOW_ACTION_MOVE, "Move"),
        (APP_CONTROL_WINDOW_ACTION_RESIZE, "Resize"),
        (APP_CONTROL_WINDOW_ACTION_BOUNDS, "Move + Resize"),
        (APP_CONTROL_WINDOW_ACTION_MINIMIZE, "Minimize"),
        (APP_CONTROL_WINDOW_ACTION_MAXIMIZE, "Maximize"),
        (APP_CONTROL_WINDOW_ACTION_RESTORE, "Restore"),
        (APP_CONTROL_WINDOW_ACTION_TRAY, "To Tray / Hide"),
        (APP_CONTROL_WINDOW_ACTION_SHOW, "Show"),
        (APP_CONTROL_WINDOW_ACTION_ALWAYS_ON_TOP, "Always On Top"),
    ])
}

fn enum_options(options: &[(&str, &str)]) -> Vec<ParameterEnumOption> {
    options
        .iter()
        .enumerate()
        .map(|(ordering, (variant_id, label))| ParameterEnumOption {
            variant_id: (*variant_id).to_string(),
            value: ParamValue::Enum((*variant_id).to_string()),
            label: (*label).to_string(),
            tags: Vec::new(),
            ordering: Some(ordering as i32),
        })
        .collect()
}

fn normalized_variant(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

pub(crate) fn sync_command_watched_app_options(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    options: &[ParameterEnumOption],
    guard: golden_io::RevisionGuard,
) {
    let Some(param_id) = crate::app::module_command::resolve_module_command_child(snapshot, command_id, "watched_app")
    else {
        return;
    };
    sync_enum_options(ctx, snapshot, param_id, options.to_vec(), Some(guard));
}

pub(crate) fn sync_watched_app_enum_options(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    param_id: NodeId,
    options: Vec<ParameterEnumOption>,
) {
    sync_enum_options(ctx, snapshot, param_id, options, None);
}

fn sync_enum_options(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    param_id: NodeId,
    options: Vec<ParameterEnumOption>,
    guard: Option<golden_io::RevisionGuard>,
) {
    let Some(original) = snapshot.node(param_id) else {
        return;
    };
    let (uuid, command_id) = (original.uuid, original.parent);
    if let Some(command) = command_id.and_then(|id| snapshot.node(id)) {
        let controller = match command.node_type.as_str() {
            APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE => Some("mode"),
            APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE | APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE => {
                Some("target_source")
            }
            _ => None,
        };
        // Dependency reconciliation can already have queued this selector's
        // removal. The shared dispatch snapshot still contains the old child.
        if controller
            .and_then(|key| snapshot.find_child_by_decl_id(command.id, key))
            .and_then(|id| snapshot.node(id))
            .and_then(|node| node.param_value.as_ref())
            .is_some_and(|value| matches!(value,ParamValue::Enum(value) if value!="watched_app"))
        {
            return;
        }
    }
    if options.len() > 64
        || options
            .iter()
            .any(|option| option.label.len() > 128 || option.variant_id.len() > 128)
    {
        return;
    }
    // Stale catalog jobs are superseded metadata work. Skipping them must not
    // reject unrelated edits in the same engine drain. Read the live selection
    // here so a queued catalog cannot restore an older authored selection.
    ctx.call_node_mutation_without_snapshot(param_id, move |node, inner_ctx| {
        if guard.as_ref().is_some_and(|guard| !guard.is_current()) {
            return Ok(());
        }
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Ok(());
        };
        if parameter.node_data().meta.uuid != uuid || parameter.node_data().parent != command_id {
            return Ok(());
        }
        let current = match &parameter.value {
            ParamValue::Enum(value) | ParamValue::Str(value) if value.len() <= 128 => value.as_str(),
            _ => {
                if let Some(command) = command_id {
                    inner_ctx.set_node_warning_with(
                        command,
                        Some(MISSING_WATCHED_APP_WARNING_ID),
                        "Watched app selection must be text within 128 bytes",
                        None,
                    );
                }
                return Ok(());
            }
        };
        let (next_options, next_variant, missing_value) = enum_options_with_missing_current(current, &options);
        if let Some(command) = command_id {
            if let Some(missing) = missing_value {
                inner_ctx.set_node_warning_with(
                    command,
                    Some(MISSING_WATCHED_APP_WARNING_ID),
                    format!("Missing app: {missing}"),
                    None,
                );
            } else {
                inner_ctx.clear_node_warning(command, Some(MISSING_WATCHED_APP_WARNING_ID));
            }
        }
        let next_value = ParamValue::Enum(next_variant.clone());
        if parameter.constraints.enum_options.as_slice() == next_options.as_slice() && parameter.value == next_value {
            return Ok(());
        }

        parameter.constraints.enum_options = next_options.into();
        parameter.value = next_value;
        Ok(())
    });
}

fn enum_options_with_missing_current(
    current_value: &str,
    options: &[ParameterEnumOption],
) -> (Vec<ParameterEnumOption>, String, Option<String>) {
    let trimmed_value = current_value.trim();
    let mut next_options = options.to_vec();

    if trimmed_value.is_empty() {
        let desired_value = next_options
            .first()
            .map(|option| option.variant_id.clone())
            .unwrap_or_default();
        return (next_options, desired_value, None);
    }

    if next_options.iter().any(|option| option.variant_id == trimmed_value) {
        return (next_options, trimmed_value.to_string(), None);
    }

    next_options.insert(
        0,
        ParameterEnumOption {
            variant_id: trimmed_value.to_string(),
            value: ParamValue::Enum(trimmed_value.to_string()),
            label: trimmed_value.to_string(),
            tags: Vec::new(),
            ordering: None,
        },
    );

    (next_options, trimmed_value.to_string(), Some(trimmed_value.to_string()))
}
