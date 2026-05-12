mod app_control_runtime;
mod commands;
#[cfg(test)]
mod app_control_tests;

use std::{collections::HashMap, path::Path};

use golden_core::{
    edit::NodeTree,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event, EventKind},
    logerror, node,
    node::{
        DeclId, Folder, Node, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor,
        UserContainerRules, UserCreatableItem,
    },
    parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption,
        ParameterEventBehaviour,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use self::app_control_runtime::{AppControlRuntime, FolderWatchUpdate};
use crate::app::module::common::app_control::{
    CommandTargetSource, KillProcessRequest, LaunchMode, LaunchProcessRequest,
    ProcessMatchMode, WindowAction, WindowControlRequest, APP_CONTROL_MATCH_MODE_EXACT,
};
use crate::app::module::common::system_metrics;
use self::commands::{
    APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE, APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE,
    APP_CONTROL_MODULE_COMMAND_TYPES, APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE,
};

const APP_CONTROL_MODULE_UPDATE_RATE_HZ: u32 = 2;
const APP_CONTROL_SCRIPT_METHODS: &[&str] = &[
    "launchWatchedApp",
    "launchApp",
    "launchCommandLine",
    "killProcess",
    "controlWindow",
];

const WATCH_FOLDER_CHANGED_CALLBACK: &str = "watchFolderChanged";
const APP_CONTROL_COMMAND_REQUESTED_CALLBACK: &str = "appControlCommandRequested";
const APP_CONTROL_COMMAND_FAILED_CALLBACK: &str = "appControlCommandFailed";

const WATCHED_APP_NODE_TYPE: &str = "file";
const WATCHED_APP_DEFAULT_LABEL: &str = "Watched App";
const WATCHED_FOLDER_ITEM_KIND: &str = "app_control_watched_folder";
const DUPLICATE_LABEL_WARNING_ID: &str = "app_control_duplicate_label";
const INVALID_TARGET_WARNING_ID: &str = "app_control_invalid_target";

#[derive(Clone, Debug, Eq, PartialEq)]
struct WatchedAppEntry {
    label: String,
    item_id: NodeId,
    target_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WatchedFolderEntry {
    label: String,
    item_id: NodeId,
    target_id: Option<NodeId>,
    changed_id: Option<NodeId>,
    target_path: String,
}

#[node("app_control_module", label = "App Control")]
#[children(
    folder(connection) {
        [base_children];
    }
    folder(parameters) {
        node watched_apps_targets: AppControlWatchedApps = AppControlWatchedApps::new() (
            label = "Watched Apps",
            description = "Add applications to monitor and reuse from App Control commands."
        );
        node watched_folders_targets: AppControlWatchFolders = AppControlWatchFolders::new() (
            label = "Watch Folders",
            description = "Add folders to watch for created, modified, and removed entries."
        );
        [base_children];
    }
    folder(values) {
        folder(watched_apps_values, label = "Watched Apps", collapsed = true) {}
        folder(watched_folders_values, label = "Watch Folders", collapsed = true) {}
        [base_children];
    }
)]
pub struct AppControlModule {
    base: crate::app::ModuleBase,
    runtime: AppControlRuntime,
    watched_app_value_labels: Vec<String>,
    watched_folder_value_labels: Vec<String>,
    watched_app_auto_labels: HashMap<NodeId, String>,
    ignored_running_value_updates: HashMap<NodeId, bool>,
    pending_running_requests: HashMap<NodeId, bool>,
}

impl AppControlModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            AppControlRuntime::create(),
            Vec::new(),
            Vec::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        )
    }

    fn sync_runtime_state(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let watched_apps = self.auto_rename_watched_apps(ctx, self.collect_watched_apps(snapshot));
        let watched_folders = self.collect_watched_folders(snapshot);

        self.apply_duplicate_label_warnings(ctx, &watched_apps, &watched_folders);
        self.apply_target_warnings(ctx, &watched_apps, &watched_folders);
        self.sync_command_target_options(ctx, snapshot, &watched_apps);
        self.runtime
            .sync_folder_keys(watched_folders.iter().map(|entry| entry.label.as_str()));

        if self.sync_value_structure(ctx, snapshot, &watched_apps, &watched_folders) {
            return;
        }

        self.runtime.refresh_processes();
        self.update_watched_app_values(ctx, snapshot, &watched_apps);
        self.update_watched_folder_values(ctx, snapshot, &watched_folders);

        if !watched_apps.is_empty() || !watched_folders.is_empty() {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn collect_watched_apps(&self, snapshot: &ProcessTreeSnapshot) -> Vec<WatchedAppEntry> {
        let Some(parameters_id) = self.base.parameters_id() else {
            return Vec::new();
        };
        let Some(parent_id) = snapshot.find_child_by_decl_id(parameters_id, "watched_apps_targets") else {
            return Vec::new();
        };

        snapshot
            .child_ids(parent_id)
            .into_iter()
            .filter_map(|item_id| {
                let item = snapshot.node(item_id)?;
                if !matches!(item.param_value.as_ref(), Some(ParamValue::File(_))) {
                    return None;
                }
                let target_path = item
                    .param_value
                    .as_ref()
                    .and_then(ParamValue::as_str)
                    .unwrap_or_default()
                    .to_string();
                Some(WatchedAppEntry {
                    label: item.label.clone(),
                    item_id,
                    target_path,
                })
            })
            .collect()
    }

    fn collect_watched_folders(&self, snapshot: &ProcessTreeSnapshot) -> Vec<WatchedFolderEntry> {
        let Some(parameters_id) = self.base.parameters_id() else {
            return Vec::new();
        };
        let Some(parent_id) = snapshot.find_child_by_decl_id(parameters_id, "watched_folders_targets") else {
            return Vec::new();
        };

        snapshot
            .child_ids(parent_id)
            .into_iter()
            .filter_map(|item_id| {
                let item = snapshot.node(item_id)?;
                if item.node_type != AppControlWatchedFolder::NODE_TYPE {
                    return None;
                }
                let target_id = snapshot.find_child(item_id, "target");
                let changed_id = snapshot.find_child(item_id, "changed");
                let target_path = target_id
                    .and_then(|id| snapshot.node(id))
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_str)
                    .unwrap_or_default()
                    .to_string();
                Some(WatchedFolderEntry {
                    label: item.label.clone(),
                    item_id,
                    target_id,
                    changed_id,
                    target_path,
                })
            })
            .collect()
    }

    fn apply_duplicate_label_warnings(
        &self,
        ctx: &mut ProcessCtx,
        watched_apps: &[WatchedAppEntry],
        watched_folders: &[WatchedFolderEntry],
    ) {
        apply_duplicate_label_warning_set(
            ctx,
            watched_apps.iter().map(|entry| (entry.item_id, entry.label.as_str())),
            "Watched app labels must be unique for command targeting and Values mirroring.",
        );
        apply_duplicate_label_warning_set(
            ctx,
            watched_folders
                .iter()
                .map(|entry| (entry.item_id, entry.label.as_str())),
            "Watch folder labels must be unique for callbacks and Values mirroring.",
        );
    }

    fn apply_target_warnings(
        &self,
        ctx: &mut ProcessCtx,
        watched_apps: &[WatchedAppEntry],
        watched_folders: &[WatchedFolderEntry],
    ) {
        for entry in watched_apps {
            let target_path = entry.target_path.trim();
            if target_path.is_empty() {
                ctx.clear_node_warning(entry.item_id, Some(INVALID_TARGET_WARNING_ID));
                continue;
            }

            let path = Path::new(target_path);
            let warning = if !path.exists() {
                Some((
                    "Application path does not exist.",
                    Some(target_path),
                ))
            } else if path.is_dir() {
                Some((
                    "Application target must point to an executable file, not a folder.",
                    Some(target_path),
                ))
            } else {
                None
            };

            apply_optional_warning(ctx, entry.item_id, INVALID_TARGET_WARNING_ID, warning);
        }

        for entry in watched_folders {
            let Some(target_id) = entry.target_id else {
                continue;
            };
            let target_path = entry.target_path.trim();
            if target_path.is_empty() {
                ctx.clear_node_warning(target_id, Some(INVALID_TARGET_WARNING_ID));
                continue;
            }

            let path = Path::new(target_path);
            let warning = if !path.exists() {
                Some(("Watch folder does not exist.", Some(target_path)))
            } else if !path.is_dir() {
                Some((
                    "Watch folder target must point to a folder.",
                    Some(target_path),
                ))
            } else {
                None
            };

            apply_optional_warning(ctx, target_id, INVALID_TARGET_WARNING_ID, warning);
        }
    }

    fn sync_command_target_options(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        watched_apps: &[WatchedAppEntry],
    ) {
        let Some(command_tester_id) = self
            .base
            .command_tester_id()
            .or_else(|| snapshot.find_child_by_decl_id(self.id(), "command_tester"))
        else {
            return;
        };

        let watched_app_options = watched_app_command_enum_options(watched_apps);
        for command_id in snapshot.child_ids(command_tester_id) {
            let Some(command) = snapshot.node(command_id) else {
                continue;
            };

            match command.node_type.as_str() {
                APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE
                | APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE
                | APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE => {
                    sync_command_enum_param_options(
                        ctx,
                        snapshot,
                        command_id,
                        "watched_app",
                        watched_app_options.as_slice(),
                    );
                }
                _ => {}
            }
        }
    }

    fn auto_rename_watched_apps(
        &mut self,
        ctx: &mut ProcessCtx,
        watched_apps: Vec<WatchedAppEntry>,
    ) -> Vec<WatchedAppEntry> {
        let live_ids = watched_apps
            .iter()
            .map(|entry| entry.item_id)
            .collect::<Vec<_>>();

        let watched_apps = watched_apps
            .into_iter()
            .map(|mut entry| {
            let next_auto_label = watched_app_label_from_target_path(entry.target_path.as_str());
            let previous_auto_label = self
                .watched_app_auto_labels
                .get(&entry.item_id)
                .cloned()
                .unwrap_or_else(|| WATCHED_APP_DEFAULT_LABEL.to_string());

            if should_auto_rename_watched_app(entry.label.as_str(), previous_auto_label.as_str())
                && entry.label != next_auto_label
            {
                ctx.patch_node_meta(
                    entry.item_id,
                    NodeMetaPatch {
                        label: Some(next_auto_label.clone()),
                        ..Default::default()
                    },
                );
                entry.label = next_auto_label.clone();
            }

            self.watched_app_auto_labels
                .insert(entry.item_id, next_auto_label);
            entry
        })
        .collect();

        self.watched_app_auto_labels
            .retain(|node_id, _| live_ids.contains(node_id));

        watched_apps
    }

    fn sync_value_structure(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        watched_apps: &[WatchedAppEntry],
        watched_folders: &[WatchedFolderEntry],
    ) -> bool {
        let next_app_labels = unique_labels_in_order(watched_apps.iter().map(|entry| entry.label.as_str()));
        let next_folder_labels =
            unique_labels_in_order(watched_folders.iter().map(|entry| entry.label.as_str()));

        if self.watched_app_value_labels == next_app_labels
            && self.watched_folder_value_labels == next_folder_labels
        {
            return false;
        }

        let Some(values_id) = self.base.values_id() else {
            return false;
        };
        let Some(app_values_id) = snapshot.find_child_by_decl_id(values_id, "watched_apps_values") else {
            return false;
        };
        let Some(folder_values_id) = snapshot.find_child_by_decl_id(values_id, "watched_folders_values") else {
            return false;
        };

        rebuild_values_root(
            ctx,
            snapshot,
            app_values_id,
            next_app_labels
                .iter()
                .map(|label| watched_app_values_tree(label.as_str()))
                .collect(),
        );
        rebuild_values_root(
            ctx,
            snapshot,
            folder_values_id,
            next_folder_labels
                .iter()
                .map(|label| watched_folder_values_tree(label.as_str()))
                .collect(),
        );

        self.watched_app_value_labels = next_app_labels;
        self.watched_folder_value_labels = next_folder_labels;
        self.pending_running_requests.clear();
        self.ignored_running_value_updates.clear();
        true
    }

    fn update_watched_app_values(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        watched_apps: &[WatchedAppEntry],
    ) {
        let Some(values_id) = self.base.values_id() else {
            return;
        };
        let Some(root_id) = snapshot.find_child_by_decl_id(values_id, "watched_apps_values") else {
            return;
        };
        let value_folders = child_ids_by_label(snapshot, root_id);

        for entry in watched_apps {
            let Some(folder_id) = value_folders.get(entry.label.as_str()).copied() else {
                continue;
            };

            let metrics = self.runtime.watched_app_metrics(entry.target_path.as_str());
            self.sync_watched_app_value_constraints(ctx, snapshot, folder_id, &metrics);
            self.handle_running_control(ctx, snapshot, entry, folder_id, metrics.running);
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "target_path",
                ParamValue::Str(metrics.target_path),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "name",
                ParamValue::Str(metrics.target_name),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "exists",
                ParamValue::Bool(metrics.exists),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "uptime_seconds",
                ParamValue::Float(metrics.uptime_seconds),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "process_count",
                ParamValue::Int(metrics.process_count),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "main_pid",
                ParamValue::Int(metrics.main_pid),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "window_opened",
                ParamValue::Bool(metrics.window_opened),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "window_count",
                ParamValue::Int(metrics.window_count),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "cpu_ratio",
                ParamValue::Float(metrics.cpu_ratio),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "memory_mb",
                ParamValue::Float(metrics.memory_mb),
            );
            set_value_param(
                snapshot,
                ctx,
                folder_id,
                "virtual_memory_mb",
                ParamValue::Float(metrics.virtual_memory_mb),
            );
        }
    }

    fn sync_watched_app_value_constraints(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        value_folder_id: NodeId,
        metrics: &app_control_runtime::WatchedAppMetrics,
    ) {
        let Some(cpu_id) = find_child_by_key(snapshot, value_folder_id, "cpu_ratio") else {
            return;
        };
        system_metrics::sync_float_constraints(ctx, snapshot, cpu_id, Some(0.0), Some(1.0));

        if let Some(memory_id) = find_child_by_key(snapshot, value_folder_id, "memory_mb") {
            system_metrics::sync_float_constraints(
                ctx,
                snapshot,
                memory_id,
                Some(0.0),
                Some(metrics.memory_max_mb.max(metrics.memory_mb)),
            );
        }

        if let Some(uptime_id) = find_child_by_key(snapshot, value_folder_id, "uptime_seconds") {
            system_metrics::sync_float_constraints(ctx, snapshot, uptime_id, Some(0.0), None);
        }

        if let Some(virtual_memory_id) = find_child_by_key(snapshot, value_folder_id, "virtual_memory_mb") {
            system_metrics::sync_float_constraints(
                ctx,
                snapshot,
                virtual_memory_id,
                Some(0.0),
                None,
            );
        }
    }

    fn sync_running_value(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        ctx: &mut ProcessCtx,
        value_folder_id: NodeId,
        running: bool,
    ) {
        let Some(running_id) = find_child_by_key(snapshot, value_folder_id, "running") else {
            return;
        };
        let Some(current_running) = snapshot
            .node(running_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_bool)
        else {
            return;
        };
        if current_running == running {
            self.ignored_running_value_updates.remove(&running_id);
            return;
        }

        self.ignored_running_value_updates.insert(running_id, running);
        ctx.set_param_with_behaviour(
            running_id,
            ParamValue::Bool(running),
            ParameterEventBehaviour::Coalesce,
        );
    }

    fn handle_running_control(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        entry: &WatchedAppEntry,
        value_folder_id: NodeId,
        actual_running: bool,
    ) {
        let Some(running_id) = find_child_by_key(snapshot, value_folder_id, "running") else {
            return;
        };
        let Some(desired_running) = snapshot
            .node(running_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_bool)
        else {
            return;
        };

        let ignored_sync = self
            .ignored_running_value_updates
            .remove(&running_id)
            .is_some_and(|expected| expected == desired_running);

        if desired_running == actual_running || ignored_sync {
            self.pending_running_requests.remove(&running_id);
            self.sync_running_value(snapshot, ctx, value_folder_id, actual_running);
            return;
        }

        if self
            .pending_running_requests
            .get(&running_id)
            .is_some_and(|pending| *pending == desired_running)
        {
            return;
        }

        let request_result = if desired_running {
            self.execute_launch_request(
                ctx,
                std::slice::from_ref(entry),
                LaunchProcessRequest {
                    watched_app: entry.label.clone(),
                    executable_path: String::new(),
                    arguments: String::new(),
                    working_directory: String::new(),
                    command_line: String::new(),
                    mode: LaunchMode::WatchedApp,
                },
            )
        } else {
            self.execute_kill_request(
                ctx,
                std::slice::from_ref(entry),
                KillProcessRequest {
                    target_source: CommandTargetSource::WatchedApp,
                    target: entry.label.clone(),
                    match_mode: ProcessMatchMode::Exact,
                    hard_kill: false,
                },
            )
        };

        match request_result {
            Ok(()) => {
                self.pending_running_requests.insert(running_id, desired_running);
            }
            Err(error) => {
                self.pending_running_requests.remove(&running_id);
                logerror!(format!(
                    "Failed to apply App Control running toggle for '{}': {error}",
                    entry.label,
                ));
                self.sync_running_value(snapshot, ctx, value_folder_id, actual_running);
            }
        }
    }

    fn update_watched_folder_values(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        watched_folders: &[WatchedFolderEntry],
    ) {
        let Some(values_id) = self.base.values_id() else {
            return;
        };
        let Some(root_id) = snapshot.find_child_by_decl_id(values_id, "watched_folders_values") else {
            return;
        };
        let value_folders = child_ids_by_label(snapshot, root_id);

        for entry in watched_folders {
            let Some(folder_id) = value_folders.get(entry.label.as_str()).copied() else {
                continue;
            };

            let update = self
                .runtime
                .poll_folder(entry.label.as_str(), entry.target_path.as_str());
            self.apply_folder_watch_update(ctx, snapshot, folder_id, entry, &update);
        }
    }

    fn apply_folder_watch_update(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        value_folder_id: NodeId,
        entry: &WatchedFolderEntry,
        update: &FolderWatchUpdate,
    ) {
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "path",
            ParamValue::Str(update.path.clone()),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "exists",
            ParamValue::Bool(update.exists),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "entry_count",
            ParamValue::Int(update.entry_count),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "last_event_kind",
            ParamValue::Str(update.last_event_kind()),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "last_event_path",
            ParamValue::Str(update.last_event_path()),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "created_count",
            ParamValue::Int(saturating_i32(update.created.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "modified_count",
            ParamValue::Int(saturating_i32(update.modified.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "removed_count",
            ParamValue::Int(saturating_i32(update.removed.len())),
        );
        set_value_param(
            snapshot,
            ctx,
            value_folder_id,
            "last_changed_ago_seconds",
            ParamValue::Float(last_changed_ago_seconds(update)),
        );

        if !update.has_changes() {
            return;
        }

        if let Some(changed_id) = entry.changed_id {
            ctx.set_param_with_behaviour(
                changed_id,
                ParamValue::Trigger(),
                ParameterEventBehaviour::Coalesce,
            );
        }

        if self.base.log_incoming_enabled() {
            golden_core::log!(origin = self.id(); format!(
                "Watch folder '{}' changed: +{} ~{} -{}.",
                entry.label,
                update.created.len(),
                update.modified.len(),
                update.removed.len(),
            ));
        }

        self.emit_watch_folder_changed(ctx, entry, update);
    }

    fn emit_watch_folder_changed(
        &self,
        ctx: &mut ProcessCtx,
        entry: &WatchedFolderEntry,
        update: &FolderWatchUpdate,
    ) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            WATCH_FOLDER_CHANGED_CALLBACK,
            vec![
                crate::app::module::script_api::node_arg(entry.item_id),
                serde_json::json!({
                    "path": update.path,
                    "exists": update.exists,
                    "entryCount": update.entry_count,
                    "created": update.created,
                    "modified": update.modified,
                    "removed": update.removed,
                    "timestampMs": update.timestamp_ms,
                }),
            ],
        );
    }

    fn emit_command_requested(
        &self,
        ctx: &mut ProcessCtx,
        command: &str,
        details: serde_json::Value,
    ) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            APP_CONTROL_COMMAND_REQUESTED_CALLBACK,
            vec![serde_json::json!(command), details],
        );
    }

    fn emit_command_failed(&self, ctx: &mut ProcessCtx, command: &str, error: &str) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            APP_CONTROL_COMMAND_FAILED_CALLBACK,
            vec![serde_json::json!(command), serde_json::json!(error)],
        );
    }

    fn execute_launch_request(
        &mut self,
        ctx: &mut ProcessCtx,
        watched_apps: &[WatchedAppEntry],
        request: LaunchProcessRequest,
    ) -> Result<(), String> {
        self.base.emit_outgoing_traffic(ctx);
        let watched_target = if request.mode == LaunchMode::WatchedApp {
            Some(resolve_watched_app_target(watched_apps, request.watched_app.as_str())?)
        } else {
            None
        };

        match self.runtime.execute_launch(&request, watched_target) {
            Ok(outcome) => {
                self.emit_command_requested(
                    ctx,
                    "launch",
                    serde_json::json!({
                        "mode": request.mode.as_str(),
                        "watchedTarget": request.watched_app,
                        "effectiveProgram": outcome.effective_program,
                        "arguments": request.arguments,
                        "workingDirectory": request.working_directory,
                        "commandLine": request.command_line,
                    }),
                );
                if self.base.log_outgoing_enabled() {
                    golden_core::log!(origin = self.id(); format!(
                        "App Control launch: mode='{}', watchedApp='{}', executable='{}', args='{}', cwd='{}', commandLine='{}', effectiveProgram='{}'.",
                        request.mode.as_str(),
                        request.watched_app,
                        request.executable_path,
                        request.arguments,
                        request.working_directory,
                        request.command_line,
                        outcome.effective_program,
                    ));
                }
                Ok(())
            }
            Err(error) => {
                self.emit_command_failed(ctx, "launch", error.as_str());
                Err(error)
            }
        }
    }

    fn execute_kill_request(
        &mut self,
        ctx: &mut ProcessCtx,
        watched_apps: &[WatchedAppEntry],
        request: KillProcessRequest,
    ) -> Result<(), String> {
        self.base.emit_outgoing_traffic(ctx);
        let watched_target = if request.target_source == CommandTargetSource::WatchedApp {
            Some(resolve_watched_app_target(watched_apps, request.target.as_str())?)
        } else {
            None
        };

        match self.runtime.execute_kill(&request, watched_target) {
            Ok(process_count) => {
                self.emit_command_requested(
                    ctx,
                    "kill",
                    serde_json::json!({
                        "target": request.target,
                        "targetSource": request.target_source.as_str(),
                        "matchMode": request.match_mode.as_str(),
                        "hardKill": request.hard_kill,
                        "processCount": process_count,
                    }),
                );
                if self.base.log_outgoing_enabled() {
                    golden_core::log!(origin = self.id(); format!(
                        "App Control kill: source='{}', target='{}', matchMode='{}', hardKill={}, processCount={}",
                        request.target_source.as_str(),
                        request.target,
                        request.match_mode.as_str(),
                        request.hard_kill,
                        process_count,
                    ));
                }
                Ok(())
            }
            Err(error) => {
                self.emit_command_failed(ctx, "kill", error.as_str());
                Err(error)
            }
        }
    }

    fn execute_window_control_request(
        &mut self,
        ctx: &mut ProcessCtx,
        watched_apps: &[WatchedAppEntry],
        request: WindowControlRequest,
    ) -> Result<(), String> {
        self.base.emit_outgoing_traffic(ctx);
        let watched_target = if request.target_source == CommandTargetSource::WatchedApp {
            Some(resolve_watched_app_target(watched_apps, request.target.as_str())?)
        } else {
            None
        };

        match self.runtime.execute_window_control(&request, watched_target) {
            Ok(window_count) => {
                self.emit_command_requested(
                    ctx,
                    "controlWindow",
                    serde_json::json!({
                        "target": request.target,
                        "targetSource": request.target_source.as_str(),
                        "matchMode": request.match_mode.as_str(),
                        "action": request.action.as_str(),
                        "windowCount": window_count,
                        "x": request.x,
                        "y": request.y,
                        "width": request.width,
                        "height": request.height,
                        "alwaysOnTop": request.always_on_top,
                    }),
                );
                if self.base.log_outgoing_enabled() {
                    golden_core::log!(origin = self.id(); format!(
                        "App Control window: source='{}', target='{}', matchMode='{}', action='{}', x={}, y={}, width={}, height={}, alwaysOnTop={}, windowCount={}",
                        request.target_source.as_str(),
                        request.target,
                        request.match_mode.as_str(),
                        request.action.as_str(),
                        request.x,
                        request.y,
                        request.width,
                        request.height,
                        request.always_on_top,
                        window_count,
                    ));
                }
                Ok(())
            }
            Err(error) => {
                self.emit_command_failed(ctx, "controlWindow", error.as_str());
                Err(error)
            }
        }
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id()
            || !APP_CONTROL_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str())
        {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        let watched_apps = self.collect_watched_apps(snapshot);

        let result = match request.command_type.as_str() {
            APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE => {
                serde_json::from_value::<LaunchProcessRequest>(request.payload)
                    .map_err(|error| format!("invalid App Control launch payload: {error}"))
                    .and_then(|payload| self.execute_launch_request(ctx, &watched_apps, payload))
            }
            APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE => {
                serde_json::from_value::<KillProcessRequest>(request.payload)
                    .map_err(|error| format!("invalid App Control kill payload: {error}"))
                    .and_then(|payload| self.execute_kill_request(ctx, &watched_apps, payload))
            }
            APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE => {
                serde_json::from_value::<WindowControlRequest>(request.payload)
                    .map_err(|error| format!("invalid App Control window payload: {error}"))
                    .and_then(|payload| {
                        self.execute_window_control_request(ctx, &watched_apps, payload)
                    })
            }
            _ => Ok(()),
        };

        if let Err(error) = result {
            logerror!(format!(
                "Failed to handle App Control command {:?}: {error}",
                request.command_id,
            ));
        }
    }

    fn handle_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let watched_apps = self.collect_watched_apps(snapshot);

        let result = match method {
            "launchWatchedApp" => {
                script_launch_watched_app_request(args).and_then(|request| {
                    self.execute_launch_request(ctx, &watched_apps, request)
                })
            }
            "launchApp" => script_launch_app_request(args)
                .and_then(|request| self.execute_launch_request(ctx, &watched_apps, request)),
            "launchCommandLine" => script_launch_command_line_request(args)
                .and_then(|request| self.execute_launch_request(ctx, &watched_apps, request)),
            "killProcess" => script_kill_request(args)
                .and_then(|request| self.execute_kill_request(ctx, &watched_apps, request)),
            "controlWindow" => script_window_control_request(args).and_then(|request| {
                self.execute_window_control_request(ctx, &watched_apps, request)
            }),
            _ => return None,
        };

        Some(result)
    }

}

#[golden_core::item(
    "module",
    node = "app_control_module",
    via = base,
    from_struct,
    menu_path = ["System"]
)]
impl Node for AppControlModule {
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ParamChanged { .. } => u32::MAX,
            _ => 1,
        }
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base
            .configure_command_tester(ctx, APP_CONTROL_MODULE_COMMAND_TYPES);
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
        self.base.set_connected(ctx, true);
        crate::app::module::enable_module_authoring(self.node_data_mut());

        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.sync_runtime_state(ctx, snapshot_arc.as_ref());
        }
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        if !self.node_data().effective_enabled {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        self.sync_runtime_state(ctx, snapshot_arc.as_ref());
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(APP_CONTROL_MODULE_UPDATE_RATE_HZ)
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            APP_CONTROL_SCRIPT_METHODS,
        )
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return self.base.engine_call_script_method(ctx, method, args);
        };

        if let Some(result) = self.handle_script_method(ctx, snapshot_arc.as_ref(), method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &old_value);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, _enabled: bool) {
        self.runtime.reset();
        self.watched_app_auto_labels.clear();
        self.ignored_running_value_updates.clear();
        self.pending_running_requests.clear();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("app_control_watched_apps", label = "Watched Apps")]
pub struct AppControlWatchedApps {}

#[node("app_control_watched_apps", from_struct)]
impl Node for AppControlWatchedApps {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[WATCHED_APP_NODE_TYPE]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(WATCHED_APP_NODE_TYPE, WATCHED_APP_NODE_TYPE, WATCHED_APP_DEFAULT_LABEL)
                .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == WATCHED_APP_NODE_TYPE)
            .then(|| Box::new(create_watched_app_parameter()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("app_control_watch_folders", label = "Watch Folders")]
pub struct AppControlWatchFolders {}

#[node("app_control_watch_folders", from_struct)]
impl Node for AppControlWatchFolders {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[WATCHED_FOLDER_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                AppControlWatchedFolder::NODE_TYPE,
                WATCHED_FOLDER_ITEM_KIND,
                "Watch Folder",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == AppControlWatchedFolder::NODE_TYPE)
            .then(|| Box::new(AppControlWatchedFolder::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("app_control_watched_folder", label = "Watch Folder")]
#[children(
    target: golden_core::parameter::File = golden_core::parameter::File::default() (
        label = "Folder",
        description = "Folder path watched for created, modified, and removed entries."
    );
    changed: ParamValue = ParamValue::Trigger() (
        label = "Changed",
        description = "Fires when the watched folder contents change.",
        read_only = true
    );
)]
pub struct AppControlWatchedFolder {}

#[node("app_control_watched_folder", from_struct)]
impl Node for AppControlWatchedFolder {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn user_item_kind(&self) -> &str {
        WATCHED_FOLDER_ITEM_KIND
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

fn apply_duplicate_label_warning_set<'a, I>(ctx: &mut ProcessCtx, items: I, message: &str)
where
    I: IntoIterator<Item = (NodeId, &'a str)>,
{
    let mut ids_by_label: HashMap<String, Vec<NodeId>> = HashMap::new();
    for (node_id, label) in items {
        ids_by_label
            .entry(label.trim().to_ascii_lowercase())
            .or_default()
            .push(node_id);
    }

    for ids in ids_by_label.into_values() {
        if ids.len() > 1 {
            for id in ids {
                ctx.set_node_warning_with(id, Some(DUPLICATE_LABEL_WARNING_ID), message, None);
            }
        } else if let Some(id) = ids.first().copied() {
            ctx.clear_node_warning(id, Some(DUPLICATE_LABEL_WARNING_ID));
        }
    }
}

fn apply_optional_warning(
    ctx: &mut ProcessCtx,
    node_id: NodeId,
    warning_id: &str,
    warning: Option<(&str, Option<&str>)>,
) {
    if let Some((message, detail)) = warning {
        ctx.set_node_warning_with(node_id, Some(warning_id), message, detail);
    } else {
        ctx.clear_node_warning(node_id, Some(warning_id));
    }
}

fn unique_labels_in_order<'a, I>(labels: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen = Vec::<String>::new();
    for label in labels {
        if seen.iter().any(|existing| existing == label) {
            continue;
        }
        seen.push(label.to_string());
    }
    seen
}

fn rebuild_values_root(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
    children: Vec<NodeTree>,
) {
    for child_id in snapshot.child_ids(root_id) {
        NodeHandle::new(child_id).remove(ctx);
    }
    for tree in children {
        ctx.add_child_tree(root_id, tree, None);
    }
}

fn watched_app_values_tree(label: &str) -> NodeTree {
    let mut tree = NodeTree::new(create_values_folder(label));
    tree.push_child(NodeTree::new(create_running_control_param()));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Target Path",
        "target_path",
        ParamValue::Str(String::new()),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Name",
        "name",
        ParamValue::Str(String::new()),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Exists",
        "exists",
        ParamValue::Bool(false),
    )));
    tree.push_child(NodeTree::new(create_read_only_time_param(
        "App Uptime",
        "uptime_seconds",
        0.0,
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Process Count",
        "process_count",
        ParamValue::Int(0),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Main PID",
        "main_pid",
        ParamValue::Int(0),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Window Opened",
        "window_opened",
        ParamValue::Bool(false),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Window Count",
        "window_count",
        ParamValue::Int(0),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "CPU Usage",
        "cpu_ratio",
        ParamValue::Float(0.0),
    )));
    tree.push_child(NodeTree::new(create_read_only_float_param(
        "Memory MB",
        "memory_mb",
        0.0,
        Some(0.0),
        None,
    )));
    tree.push_child(NodeTree::new(create_read_only_float_param(
        "Virtual Memory MB",
        "virtual_memory_mb",
        0.0,
        Some(0.0),
        None,
    )));
    tree
}

fn watched_folder_values_tree(label: &str) -> NodeTree {
    let mut tree = NodeTree::new(create_values_folder(label));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Path",
        "path",
        ParamValue::Str(String::new()),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Exists",
        "exists",
        ParamValue::Bool(false),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Entry Count",
        "entry_count",
        ParamValue::Int(0),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Last Event Kind",
        "last_event_kind",
        ParamValue::Str(String::new()),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Last Event Path",
        "last_event_path",
        ParamValue::Str(String::new()),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Created Count",
        "created_count",
        ParamValue::Int(0),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Modified Count",
        "modified_count",
        ParamValue::Int(0),
    )));
    tree.push_child(NodeTree::new(create_read_only_param(
        "Removed Count",
        "removed_count",
        ParamValue::Int(0),
    )));
    tree.push_child(NodeTree::new(create_read_only_time_param(
        "Last Changed Ago",
        "last_changed_ago_seconds",
        0.0,
    )));
    tree
}

fn create_values_folder(label: &str) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}

fn create_watched_app_parameter() -> Parameter {
    let mut parameter = Parameter::new(
        WATCHED_APP_DEFAULT_LABEL,
        ParamValue::File(String::new()),
        ParameterChangeCheck::ValueChange,
    );
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter.node_data_mut().meta.description = Some(
        "Executable path used for monitoring, watched-app commands, and the Values Running toggle."
            .to_string(),
    );
    parameter
}

fn create_running_control_param() -> Parameter {
    let mut parameter = Parameter::new(
        "Running",
        ParamValue::Bool(false),
        ParameterChangeCheck::ValueChange,
    );
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter.node_data_mut().meta.description = Some(
        "Toggle to launch the watched application or request a normal close by terminating its running process."
            .to_string(),
    );
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId("running".to_string());
    meta.short_name = "running".to_string();
    parameter
}

fn create_read_only_param(label: &str, decl_id: &str, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.short_name = decl_id.to_string();
    parameter
}

fn create_read_only_float_param(
    label: &str,
    decl_id: &str,
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
) -> Parameter {
    let mut parameter = create_read_only_param(label, decl_id, ParamValue::Float(value));
    parameter.constraints = system_metrics::float_constraints(min, max);
    parameter
}

fn create_read_only_time_param(label: &str, decl_id: &str, value: f64) -> Parameter {
    let mut parameter = create_read_only_float_param(label, decl_id, value, Some(0.0), None);
    parameter.ui_hints.widget = Some("time".to_string());
    parameter
}

fn last_changed_ago_seconds(update: &FolderWatchUpdate) -> f64 {
    if update.last_change_ms == 0 {
        return 0.0;
    }

    update
        .timestamp_ms
        .saturating_sub(update.last_change_ms) as f64
        / 1000.0
}

fn child_ids_by_label(snapshot: &ProcessTreeSnapshot, parent: NodeId) -> HashMap<String, NodeId> {
    let mut by_label = HashMap::new();
    for child_id in snapshot.child_ids(parent) {
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        by_label.entry(child.label.clone()).or_insert(child_id);
    }
    by_label
}

fn set_value_param(
    snapshot: &ProcessTreeSnapshot,
    ctx: &mut ProcessCtx,
    parent: NodeId,
    key: &str,
    value: ParamValue,
) {
    let Some(param_id) = find_child_by_key(snapshot, parent, key) else {
        return;
    };
    let Some(current) = snapshot.node(param_id).and_then(|node| node.param_value.as_ref()) else {
        return;
    };
    if current == &value {
        return;
    }

    ctx.set_param_with_behaviour(param_id, value, ParameterEventBehaviour::Coalesce);
}

fn find_child_by_key(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<NodeId> {
    snapshot.child_ids(parent).into_iter().find(|child_id| {
        snapshot.node(*child_id).is_some_and(|child| {
            child.decl_id == key
                || child.decl_id.rsplit('/').next() == Some(key)
                || child.short_name == key
                || child.label == key
        })
    })
}

fn resolve_watched_app_target<'a>(
    watched_apps: &'a [WatchedAppEntry],
    target: &str,
) -> Result<&'a str, String> {
    let normalized_target = target.trim();
    if normalized_target.is_empty() {
        return Err("watched app target cannot be empty".to_string());
    }

    watched_apps
        .iter()
        .find(|entry| {
            entry.label.eq_ignore_ascii_case(normalized_target)
                || entry.target_path.eq_ignore_ascii_case(normalized_target)
        })
        .map(|entry| entry.target_path.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("No watched app matches '{normalized_target}'"))
}

fn watched_app_label_from_target_path(target_path: &str) -> String {
    let trimmed = target_path.trim();
    if trimmed.is_empty() {
        return WATCHED_APP_DEFAULT_LABEL.to_string();
    }

    Path::new(trimmed)
        .file_stem()
        .or_else(|| Path::new(trimmed).file_name())
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| WATCHED_APP_DEFAULT_LABEL.to_string())
}

fn should_auto_rename_watched_app(current_label: &str, previous_auto_label: &str) -> bool {
    let trimmed_label = current_label.trim();
    trimmed_label.is_empty()
        || trimmed_label.eq_ignore_ascii_case(WATCHED_APP_DEFAULT_LABEL)
    || trimmed_label == previous_auto_label
}

fn watched_app_command_enum_options(
    watched_apps: &[WatchedAppEntry],
) -> Vec<ParameterEnumOption> {
    unique_labels_in_order(watched_apps.iter().map(|entry| entry.label.as_str()))
        .into_iter()
        .filter(|label| !label.trim().is_empty())
        .map(|label| ParameterEnumOption {
            variant_id: label.clone(),
            value: ParamValue::Enum(label.clone()),
            label,
            tags: Vec::new(),
            ordering: None,
        })
        .collect()
}

fn sync_command_enum_param_options(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    path: &str,
    options: &[ParameterEnumOption],
) {
    let Some(param_id) = crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
    else {
        return;
    };
    let Some(node) = snapshot.node(param_id) else {
        return;
    };

    let mut constraints = node.param_constraints.clone().unwrap_or_default();
    let current_value = node
        .param_value
        .as_ref()
        .and_then(ParamValue::as_str)
        .unwrap_or_default();
    let desired_value = desired_enum_value(current_value.as_str(), options);
    let current_is_enum = node
        .param_value
        .as_ref()
        .is_some_and(|value| matches!(value, ParamValue::Enum(_)));

    if constraints.enum_options == options
        && current_is_enum
        && current_value == desired_value
    {
        return;
    }

    constraints.enum_options = options.to_vec();
    ctx.call_node_mutation(param_id, move |node, _| {
        node.engine_restore_param_state(ParamValue::Enum(desired_value), constraints)
    });
}

fn desired_enum_value(current_value: &str, options: &[ParameterEnumOption]) -> String {
    if options
        .iter()
        .any(|option| option.variant_id == current_value)
    {
        return current_value.to_string();
    }

    options
        .first()
        .map(|option| option.variant_id.clone())
        .unwrap_or_default()
}


fn script_launch_watched_app_request(args: &[ParamValue]) -> Result<LaunchProcessRequest, String> {
    Ok(LaunchProcessRequest {
        mode: LaunchMode::WatchedApp,
        watched_app: required_script_string(args, 0, "watched app target")?,
        executable_path: String::new(),
        arguments: optional_script_string(args, 1),
        working_directory: optional_script_string(args, 2),
        command_line: String::new(),
    })
}

fn script_launch_app_request(args: &[ParamValue]) -> Result<LaunchProcessRequest, String> {
    Ok(LaunchProcessRequest {
        mode: LaunchMode::Executable,
        watched_app: String::new(),
        executable_path: required_script_string(args, 0, "application path")?,
        arguments: optional_script_string(args, 1),
        working_directory: optional_script_string(args, 2),
        command_line: String::new(),
    })
}

fn script_launch_command_line_request(args: &[ParamValue]) -> Result<LaunchProcessRequest, String> {
    Ok(LaunchProcessRequest {
        mode: LaunchMode::CommandLine,
        watched_app: String::new(),
        executable_path: String::new(),
        arguments: String::new(),
        working_directory: optional_script_string(args, 1),
        command_line: required_script_string(args, 0, "command line")?,
    })
}

fn script_kill_request(args: &[ParamValue]) -> Result<KillProcessRequest, String> {
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

fn script_window_control_request(args: &[ParamValue]) -> Result<WindowControlRequest, String> {
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

fn parse_script_match_mode(value: Option<&ParamValue>) -> Result<ProcessMatchMode, String> {
    let variant = value
        .and_then(ParamValue::as_str)
        .unwrap_or_else(|| APP_CONTROL_MATCH_MODE_EXACT.to_string());
    ProcessMatchMode::from_variant(variant.as_str())
        .ok_or_else(|| format!("unsupported process match mode '{variant}'"))
}

fn required_script_string(args: &[ParamValue], index: usize, label: &str) -> Result<String, String> {
    args.get(index)
        .and_then(ParamValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("App Control script method expects a non-empty {label}"))
}

fn optional_script_string(args: &[ParamValue], index: usize) -> String {
    args.get(index).and_then(ParamValue::as_str).unwrap_or_default()
}

fn optional_script_bool(args: &[ParamValue], index: usize, default: bool) -> bool {
    args.get(index)
        .and_then(ParamValue::as_bool)
        .unwrap_or(default)
}

fn optional_script_int(args: &[ParamValue], index: usize, default: i32) -> i32 {
    args.get(index)
        .and_then(ParamValue::as_int)
        .unwrap_or(default)
}

fn saturating_i32(value: usize) -> i32 {
    value.min(i32::MAX as usize) as i32
}