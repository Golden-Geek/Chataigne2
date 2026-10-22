use std::collections::HashSet;
use std::path::Path;

use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue, ParameterEnumOption},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::app_control::{
    launch_mode_enum_options, match_mode_enum_options, sync_watched_app_enum_options, target_source_enum_options,
    window_action_enum_options, APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE, APP_CONTROL_LAUNCH_MODE_COMMAND_LINE,
    APP_CONTROL_LAUNCH_MODE_EXECUTABLE, APP_CONTROL_LAUNCH_MODE_WATCHED_APP,
    APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE, APP_CONTROL_MATCH_MODE_EXACT, APP_CONTROL_TARGET_SOURCE_FREE_PROCESS,
    APP_CONTROL_TARGET_SOURCE_WATCHED_APP, APP_CONTROL_WINDOW_ACTION_ALWAYS_ON_TOP, APP_CONTROL_WINDOW_ACTION_BOUNDS,
    APP_CONTROL_WINDOW_ACTION_MINIMIZE, APP_CONTROL_WINDOW_ACTION_MOVE, APP_CONTROL_WINDOW_ACTION_RESIZE,
    APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE,
};
use crate::app::module_command;

const WATCHED_APP_DEFAULT_LABEL: &str = "Watched App";

macro_rules! app_control_command_node_impl {
    () => {
        fn child_event_interest_depth(&self, event: &Event) -> u32 {
            if matches!(event.kind, EventKind::ParamChanged { .. }) {
                u32::MAX
            } else {
                0
            }
        }
        fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
            if ctx
                .tree_snapshot()
                .is_some_and(|snapshot| module_command::module_command_triggered(snapshot, self.id(), param))
            {
                run(ctx, self.id(), None);
            }
        }
        fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
            if event.topic == module_command::MODULE_COMMAND_EXECUTE_TOPIC
                && event
                    .payload
                    .get("command_id")
                    .and_then(serde_json::Value::as_u64)
                    == Some(self.id().0)
            {
                run(ctx, self.id(), Some(event));
            }
        }
    };
}
fn run(ctx: &mut ProcessCtx, command: NodeId, event: Option<golden_core::events::CustomEvent>) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    if let Err(error) = module_command::CapturedInvocation::capture(&snapshot, command, event, &["app_control_module"])
        .and_then(|invocation| crate::app::queue_app_control_command(ctx, invocation))
    {
        golden_core::logerror!(format!("App Control command was not admitted: {error}"));
    }
}

#[node("app_control_launch_process_command", label = "Launch")]
#[children(
    mode: Enum = APP_CONTROL_LAUNCH_MODE_WATCHED_APP (
        label = "Mode",
        description = "Choose whether to launch one watched app, a direct executable path, or a shell command line.",
        enum_options = launch_mode_enum_options()
    );
    watched_app: Enum = String::new() (
        label = "Watched App",
        description = "Watched app label to launch when Mode is watched_app.",
        enum_options = Vec::<golden_core::parameter::ParameterEnumOption>::new(),
        dependency = mode == APP_CONTROL_LAUNCH_MODE_WATCHED_APP
    );
    executable_path: golden_core::parameter::File = golden_core::parameter::File::default() (
        label = "Application",
        description = "Executable path to launch when Mode is executable.",
        dependency = mode == APP_CONTROL_LAUNCH_MODE_EXECUTABLE
    );
    arguments: String = String::new() (
        label = "Arguments",
        description = "Raw command-line arguments appended to the launch target.",
        dependency = mode != APP_CONTROL_LAUNCH_MODE_COMMAND_LINE
    );
    working_directory: golden_core::parameter::File = golden_core::parameter::File::default() (
        label = "Working Directory",
        description = "Optional working directory used when launching the process. Enter a folder path manually when needed."
    );
    command_line: String = String::new() (
        label = "Command Line",
        description = "Raw shell command line launched when Mode is command_line.",
        widget = "textarea",
        dependency = mode == APP_CONTROL_LAUNCH_MODE_COMMAND_LINE
    );
)]
pub struct AppControlLaunchProcessCommand {
    base: crate::app::ModuleCommandBase,
}

impl AppControlLaunchProcessCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "app_control_launch_process_command",
    via = base,
    from_struct
)]
impl Node for AppControlLaunchProcessCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE).then(Self::create)
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        sync_watched_app_options_from_enclosing_module(
            ctx,
            snapshot_arc.as_ref(),
            self.node_data().parent,
            self.watched_app.is_bound().then_some(self.watched_app.id()),
        );
    }

    app_control_command_node_impl!();
}

#[node("app_control_kill_process_command", label = "Kill")]
#[children(
    target_source: Enum = APP_CONTROL_TARGET_SOURCE_WATCHED_APP (
        label = "Target Source",
        description = "Choose whether the target string resolves one watched app or matches a free process.",
        enum_options = target_source_enum_options()
    );
    watched_app: Enum = String::new() (
        label = "Watched App",
        description = "Watched app label to resolve when Target Source is watched_app.",
        enum_options = Vec::<golden_core::parameter::ParameterEnumOption>::new(),
        dependency = target_source == APP_CONTROL_TARGET_SOURCE_WATCHED_APP
    );
    target: String = String::new() (
        label = "Process",
        description = "Free-process name/path pattern.",
        dependency = target_source == APP_CONTROL_TARGET_SOURCE_FREE_PROCESS
    );
    match_mode: Enum = APP_CONTROL_MATCH_MODE_EXACT (
        label = "Match Mode",
        description = "How the Target string matches free-process names or executable paths.",
        enum_options = match_mode_enum_options(),
        dependency = target_source == APP_CONTROL_TARGET_SOURCE_FREE_PROCESS
    );
    hard_kill: bool = false (
        label = "Hard Kill",
        description = "Force termination instead of a softer OS shutdown request."
    );
)]
pub struct AppControlKillProcessCommand {
    base: crate::app::ModuleCommandBase,
}

impl AppControlKillProcessCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "app_control_kill_process_command",
    via = base,
    from_struct
)]
impl Node for AppControlKillProcessCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE).then(Self::create)
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        sync_watched_app_options_from_enclosing_module(
            ctx,
            snapshot_arc.as_ref(),
            self.node_data().parent,
            self.watched_app.is_bound().then_some(self.watched_app.id()),
        );
    }

    app_control_command_node_impl!();
}

#[node("app_control_window_control_command", label = "Window Control")]
#[children(
    target_source: Enum = APP_CONTROL_TARGET_SOURCE_WATCHED_APP (
        label = "Target Source",
        description = "Choose whether the target string resolves one watched app or matches a free process.",
        enum_options = target_source_enum_options()
    );
    watched_app: Enum = String::new() (
        label = "Watched App",
        description = "Watched app label to resolve when Target Source is watched_app.",
        enum_options = Vec::<golden_core::parameter::ParameterEnumOption>::new(),
        dependency = target_source == APP_CONTROL_TARGET_SOURCE_WATCHED_APP
    );
    target: String = String::new() (
        label = "Process",
        description = "Free-process name/path pattern.",
        dependency = target_source == APP_CONTROL_TARGET_SOURCE_FREE_PROCESS
    );
    match_mode: Enum = APP_CONTROL_MATCH_MODE_EXACT (
        label = "Match Mode",
        description = "How the Target string matches free-process names or executable paths.",
        enum_options = match_mode_enum_options(),
        dependency = target_source == APP_CONTROL_TARGET_SOURCE_FREE_PROCESS
    );
    action: Enum = APP_CONTROL_WINDOW_ACTION_MINIMIZE (
        label = "Action",
        description = "Window operation applied to every matched top-level application window.",
        enum_options = window_action_enum_options()
    );
    x: i32 = 0 [-32768..32767] (
        label = "X",
        description = "Window X position used by move and bounds actions.",
        dependency = action == APP_CONTROL_WINDOW_ACTION_MOVE || action == APP_CONTROL_WINDOW_ACTION_BOUNDS
    );
    y: i32 = 0 [-32768..32767] (
        label = "Y",
        description = "Window Y position used by move and bounds actions.",
        dependency = action == APP_CONTROL_WINDOW_ACTION_MOVE || action == APP_CONTROL_WINDOW_ACTION_BOUNDS
    );
    width: i32 = 1280 [1..32767] (
        label = "Width",
        description = "Window width used by resize and bounds actions.",
        dependency = action == APP_CONTROL_WINDOW_ACTION_RESIZE || action == APP_CONTROL_WINDOW_ACTION_BOUNDS
    );
    height: i32 = 720 [1..32767] (
        label = "Height",
        description = "Window height used by resize and bounds actions.",
        dependency = action == APP_CONTROL_WINDOW_ACTION_RESIZE || action == APP_CONTROL_WINDOW_ACTION_BOUNDS
    );
    always_on_top: bool = true (
        label = "Always On Top",
        description = "Whether the window should be pinned above others when Action is always_on_top.",
        dependency = action == APP_CONTROL_WINDOW_ACTION_ALWAYS_ON_TOP
    );
)]
pub struct AppControlWindowControlCommand {
    base: crate::app::ModuleCommandBase,
}

impl AppControlWindowControlCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "app_control_window_control_command",
    via = base,
    from_struct
)]
impl Node for AppControlWindowControlCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE).then(Self::create)
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        sync_watched_app_options_from_enclosing_module(
            ctx,
            snapshot_arc.as_ref(),
            self.node_data().parent,
            self.watched_app.is_bound().then_some(self.watched_app.id()),
        );
    }

    app_control_command_node_impl!();
}

fn sync_watched_app_options_from_enclosing_module(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    command_parent: Option<NodeId>,
    watched_app_param_id: Option<NodeId>,
) {
    let (Some(parent_id), Some(param_id)) = (command_parent, watched_app_param_id) else {
        return;
    };
    let Some(module_id) = crate::app::module::resolve_enclosing_module_root(snapshot, parent_id) else {
        return;
    };

    sync_watched_app_enum_options(
        ctx,
        snapshot,
        param_id,
        watched_app_enum_options_for_module(snapshot, module_id),
    );
}

fn watched_app_enum_options_for_module(snapshot: &ProcessTreeSnapshot, module_id: NodeId) -> Vec<ParameterEnumOption> {
    let Some(parameters_id) = snapshot.find_child_by_decl_id(module_id, "parameters") else {
        return Vec::new();
    };
    let Some(watched_apps_root_id) = snapshot.find_child_by_decl_id(parameters_id, "watched_apps_targets") else {
        return Vec::new();
    };

    if snapshot
        .node(watched_apps_root_id)
        .is_none_or(|node| node.child_count > 64)
    {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut options = Vec::new();
    for item_id in snapshot.child_ids(watched_apps_root_id) {
        let Some(item) = snapshot.node(item_id) else {
            continue;
        };
        if !matches!(item.param_value.as_ref(), Some(ParamValue::File(_))) {
            continue;
        }

        if item.label.len() > 128 || matches!(item.param_value.as_ref(),Some(ParamValue::File(text)) if text.len()>4096)
        {
            return Vec::new();
        }
        let target_path = item
            .param_value
            .as_ref()
            .and_then(ParamValue::as_str)
            .unwrap_or_default();
        if target_path.trim().is_empty() {
            continue;
        }
        let Some(label) = watched_app_option_label(item.label.as_str(), target_path.as_str()) else {
            continue;
        };
        if !seen.insert(label.clone()) {
            continue;
        }

        options.push(ParameterEnumOption {
            variant_id: label.clone(),
            value: ParamValue::Enum(label.clone()),
            label,
            tags: Vec::new(),
            ordering: None,
        });
    }

    options
}

fn watched_app_option_label(current_label: &str, target_path: &str) -> Option<String> {
    let trimmed_label = current_label.trim();
    if !trimmed_label.is_empty() && !trimmed_label.eq_ignore_ascii_case(WATCHED_APP_DEFAULT_LABEL) {
        return Some(trimmed_label.to_string());
    }

    let trimmed_target = target_path.trim();
    if trimmed_target.is_empty() {
        return None;
    }

    Path::new(trimmed_target)
        .file_stem()
        .or_else(|| Path::new(trimmed_target).file_name())
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
}
