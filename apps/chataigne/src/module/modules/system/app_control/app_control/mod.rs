mod app_control_runtime;
mod script_requests;
mod watch_structure;
mod watch_processing;
#[cfg(test)]
mod tests;

use script_requests::*;
use watch_structure::*;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

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
use crate::app::module::common::app_control::{
    sync_command_watched_app_options, APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE,
    APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE, APP_CONTROL_MODULE_COMMAND_TYPES,
    APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE,
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
    watched_app_value_folders: HashMap<NodeId, NodeId>,
    watched_folder_value_folders: HashMap<NodeId, NodeId>,
    watched_app_auto_labels: HashMap<NodeId, String>,
    ignored_running_value_updates: HashMap<NodeId, bool>,
    requested_running_control_changes: HashSet<NodeId>,
    pending_running_requests: HashMap<NodeId, bool>,
    has_active_watch_targets: bool,
    watch_config_dirty: bool,
}

impl AppControlModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            AppControlRuntime::create(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashSet::new(),
            HashMap::new(),
            false,
            false,
        )
    }

    fn sync_watch_state(
        &mut self,
        watched_apps: &[WatchedAppEntry],
        watched_folders: &[WatchedFolderEntry],
    ) {
        self.has_active_watch_targets = Self::has_active_watched_apps(watched_apps)
            || Self::has_active_watched_folders(watched_folders);
        self.watch_config_dirty = false;
    }

    fn has_active_watched_apps(watched_apps: &[WatchedAppEntry]) -> bool {
        watched_apps
            .iter()
            .any(|entry| !entry.target_path.trim().is_empty())
    }

    fn has_active_watched_folders(watched_folders: &[WatchedFolderEntry]) -> bool {
        watched_folders
            .iter()
            .any(|entry| !entry.target_path.trim().is_empty())
    }

    fn is_watch_configuration_param(
        &self,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
    ) -> bool {
        let Some(parameters_id) = self.base.parameters_id() else {
            return false;
        };
        let Some(node) = snapshot.node(param) else {
            return false;
        };
        let Some(parent_id) = node.parent else {
            return false;
        };

        if snapshot.find_child_by_decl_id(parameters_id, "watched_apps_targets") == Some(parent_id)
        {
            return true;
        }

        let Some(watched_folders_root_id) = snapshot.find_child_by_decl_id(
            parameters_id,
            "watched_folders_targets",
        ) else {
            return false;
        };
        if snapshot.node(parent_id).and_then(|parent| parent.parent) != Some(watched_folders_root_id) {
            return false;
        }

        node.decl_id.rsplit('/').next() == Some("target") || node.short_name == "target"
    }

    fn is_watch_item_root(&self, snapshot: &ProcessTreeSnapshot, parent: NodeId) -> bool {
        let Some(parameters_id) = self.base.parameters_id() else {
            return false;
        };

        snapshot.find_child_by_decl_id(parameters_id, "watched_apps_targets") == Some(parent)
            || snapshot.find_child_by_decl_id(parameters_id, "watched_folders_targets") == Some(parent)
    }

    fn sync_runtime_state(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let watched_apps = self.auto_rename_watched_apps(ctx, self.collect_watched_apps(snapshot));
        let watched_folders = self.collect_watched_folders(snapshot);
        let has_active_watched_apps = Self::has_active_watched_apps(&watched_apps);
        let has_active_watched_folders = Self::has_active_watched_folders(&watched_folders);

        self.sync_watch_state(&watched_apps, &watched_folders);

        self.apply_duplicate_label_warnings(ctx, &watched_apps, &watched_folders);
        self.apply_target_warnings(ctx, &watched_apps, &watched_folders);
        self.sync_command_target_options(ctx, snapshot, &watched_apps);
        self.runtime
            .sync_folder_keys(watched_folders.iter().map(|entry| entry.label.as_str()));
        self.runtime
            .sync_watched_app_targets(watched_apps.iter().map(|entry| entry.target_path.as_str()));

        self.sync_value_structure(ctx, snapshot, &watched_apps, &watched_folders);
        self.update_watched_app_values(ctx, snapshot, &watched_apps);
        self.update_watched_folder_values(ctx, snapshot, &watched_folders);

        if has_active_watched_apps || has_active_watched_folders {
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
        let watched_app_options = watched_app_command_enum_options(watched_apps);
        for command_id in descendant_ids_matching_command_types(snapshot, self.id()) {
            let Some(command) = snapshot.node(command_id) else {
                continue;
            };

            match command.node_type.as_str() {
                APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE
                | APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE
                | APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE => {
                    sync_command_watched_app_options(
                        ctx,
                        snapshot,
                        command_id,
                        watched_app_options.as_slice(),
                    );
                }
                _ => {}
            }
        }
    }

    fn sync_command_target_options_for_watched_app_param_change(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
    ) {
        let Some(parameters_id) = self.base.parameters_id() else {
            return;
        };
        let Some(watched_apps_root_id) = snapshot.find_child_by_decl_id(parameters_id, "watched_apps_targets")
        else {
            return;
        };
        if snapshot.node(param).and_then(|node| node.parent) != Some(watched_apps_root_id) {
            return;
        }

        let Some(new_target_path) = ctx.events.iter().rev().find_map(|event| match &event.kind {
            EventKind::ParamChanged {
                param: changed_param,
                new_value,
                ..
            } if *changed_param == param => new_value.as_str(),
            _ => None,
        }) else {
            return;
        };

        let previous_auto_label = self
            .watched_app_auto_labels
            .get(&param)
            .cloned()
            .unwrap_or_else(|| WATCHED_APP_DEFAULT_LABEL.to_string());
        let mut watched_apps = self.collect_watched_apps(snapshot);
        let next_auto_label = watched_app_label_from_target_path(new_target_path.as_str());

        for entry in &mut watched_apps {
            if entry.item_id != param {
                continue;
            }

            entry.target_path = new_target_path.to_string();
            if should_auto_rename_watched_app(entry.label.as_str(), previous_auto_label.as_str()) {
                entry.label = next_auto_label.clone();
            }
            break;
        }

        self.sync_command_target_options(ctx, snapshot, &watched_apps);
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
    ) {
        let Some(values_id) = self.base.values_id() else {
            return;
        };
        let Some(app_values_id) = snapshot.find_child_by_decl_id(values_id, "watched_apps_values") else {
            return;
        };
        let Some(folder_values_id) = snapshot.find_child_by_decl_id(values_id, "watched_folders_values") else {
            return;
        };

        let app_sync = sync_values_root(
            ctx,
            snapshot,
            app_values_id,
            watched_apps
                .iter()
                .map(|entry| (entry.item_id, entry.label.as_str())),
            &mut self.watched_app_value_folders,
            watched_app_values_tree,
        );
        for removed_folder_id in app_sync.removed_folder_ids {
            self.clear_watched_app_value_state(snapshot, removed_folder_id);
        }

        let _folder_sync = sync_values_root(
            ctx,
            snapshot,
            folder_values_id,
            watched_folders
                .iter()
                .map(|entry| (entry.item_id, entry.label.as_str())),
            &mut self.watched_folder_value_folders,
            watched_folder_values_tree,
        );
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
            EventKind::ChildAdded { .. } | EventKind::ChildRemoved { .. } => u32::MAX,
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

    fn needs_update(&self) -> bool {
        self.watch_config_dirty || self.has_active_watch_targets
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(APP_CONTROL_MODULE_UPDATE_RATE_HZ)
            .with_compiled_kernel("chataigne.runtime.app-control")
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
            let snapshot = snapshot_arc.as_ref();
            let watch_configuration_changed = self.is_watch_configuration_param(snapshot, param);
            self.sync_command_target_options_for_watched_app_param_change(
                ctx,
                snapshot,
                param,
            );
            if watch_configuration_changed {
                self.watch_config_dirty = true;
            }
            if self.is_running_control_param(snapshot, param)
                && !self.is_ignored_running_sync(snapshot, param)
            {
                self.requested_running_control_changes.insert(param);
            }
            self.base
                .emit_script_param_callback(ctx, snapshot, param, &old_value);
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if self.is_watch_item_root(snapshot, parent) {
            self.watch_config_dirty = true;
            let watched_apps = self.collect_watched_apps(snapshot);
            self.sync_command_target_options(ctx, snapshot, &watched_apps);
            return;
        }

        let Some(command_tester_id) = self
            .base
            .command_tester_id()
            .or_else(|| snapshot.find_child_by_decl_id(self.id(), "command_tester"))
        else {
            return;
        };
        if !node_is_within_subtree(snapshot, child, command_tester_id) {
            return;
        }
        if !snapshot.node(child).is_some_and(|node| is_app_control_command_type(node.node_type.as_str())) {
            return;
        }

        let watched_apps = self.collect_watched_apps(snapshot);
        self.sync_command_target_options(ctx, snapshot, &watched_apps);
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if !self.is_watch_item_root(snapshot, parent) {
            return;
        }

        self.watch_config_dirty = true;
        let watched_apps = self
            .collect_watched_apps(snapshot)
            .into_iter()
            .filter(|entry| entry.item_id != child)
            .collect::<Vec<_>>();
        self.sync_command_target_options(ctx, snapshot, &watched_apps);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, _enabled: bool) {
        self.runtime.reset();
        self.watched_app_value_folders.clear();
        self.watched_folder_value_folders.clear();
        self.watched_app_auto_labels.clear();
        self.ignored_running_value_updates.clear();
        self.requested_running_control_changes.clear();
        self.pending_running_requests.clear();
        self.has_active_watch_targets = false;
        self.watch_config_dirty = false;
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
