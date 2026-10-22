mod app_control_runtime;
mod catalog;
mod script_requests;
mod service;
mod watch_processing;
mod watch_structure;
pub(crate) use service::dispatch as queue_app_control_command;
#[cfg(test)]
mod tests;

use watch_structure::*;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use golden_core::{
    edit::NodeTree,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event, EventKind},
    node,
    node::{
        DeclId, Folder, Node, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor, NodeUuid, UserContainerRules,
        UserCreatableItem,
    },
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use self::app_control_runtime::FolderWatchUpdate;
use crate::app::module::common::app_control::{
    sync_command_watched_app_options, APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE,
    APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE, APP_CONTROL_MODULE_COMMAND_TYPES,
    APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE,
};
use crate::app::module::common::app_control::{
    CommandTargetSource, KillProcessRequest, LaunchMode, LaunchProcessRequest, ProcessMatchMode, WindowAction,
    WindowControlRequest, APP_CONTROL_MATCH_MODE_EXACT,
};
use crate::app::module::common::system_metrics;

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
    uuid: NodeUuid,
    target_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WatchedFolderEntry {
    label: String,
    item_id: NodeId,
    uuid: NodeUuid,
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
    service: service::Service,
    watched_app_value_folders: HashMap<NodeId, NodeId>,
    watched_folder_value_folders: HashMap<NodeId, NodeId>,
    watched_app_auto_labels: HashMap<NodeId, String>,
    watch_config_dirty: bool,
}

impl AppControlModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            service::Service::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            true,
        )
    }

    fn is_watch_configuration_param(&self, snapshot: &ProcessTreeSnapshot, param: NodeId) -> bool {
        let Some(parameters_id) = self.base.parameters_id() else {
            return false;
        };
        let Some(node) = snapshot.node(param) else {
            return false;
        };
        let Some(parent_id) = node.parent else {
            return false;
        };

        if snapshot.find_child_by_decl_id(parameters_id, "watched_apps_targets") == Some(parent_id) {
            return true;
        }

        let Some(watched_folders_root_id) = snapshot.find_child_by_decl_id(parameters_id, "watched_folders_targets")
        else {
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

    fn watch_structure_changed(&self, snapshot: &ProcessTreeSnapshot, parent: NodeId) -> bool {
        if self.is_watch_item_root(snapshot, parent)
            || snapshot
                .node(parent)
                .and_then(|node| node.parent)
                .is_some_and(|root| self.is_watch_item_root(snapshot, root))
        {
            return true;
        }
        self.base.values_id().is_some_and(|values| {
            ["watched_apps_values", "watched_folders_values"]
                .into_iter()
                .any(|key| snapshot.find_child_by_decl_id(values, key) == Some(parent))
        })
    }

    fn sync_runtime_state(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if let Err(error) = self.validate_watch_shape(snapshot) {
            self.warning(ctx, &error);
            self.watch_config_dirty = false;
            return;
        }
        let watched_apps = self.auto_rename_watched_apps(ctx, self.collect_watched_apps(snapshot));
        let watched_folders = self.collect_watched_folders(snapshot);
        self.apply_duplicate_label_warnings(ctx, &watched_apps, &watched_folders);
        self.sync_value_structure(ctx, snapshot, &watched_apps, &watched_folders);
        self.sync_command_target_options(ctx, snapshot, &watched_apps);
        let config = app_control_runtime::Config {
            apps: watched_apps,
            folders: watched_folders,
        };
        if let Some(runtime) = &mut self.service.runtime {
            match runtime.configure(config) {
                Ok(config) => self.service.config = Some(config),
                Err(error) => {
                    self.warning(ctx, &error);
                    self.service.config_retry_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
                    if let Some(wake) = &self.service.wake {
                        wake.later(std::time::Duration::from_millis(250));
                    }
                    return;
                }
            }
        }
        self.watch_config_dirty = false;
    }

    fn validate_watch_shape(&self, snapshot: &ProcessTreeSnapshot) -> Result<(), String> {
        let Some(parameters) = self.base.parameters_id() else {
            return Ok(());
        };
        let mut total = 0;
        for key in ["watched_apps_targets", "watched_folders_targets"] {
            let Some(root) = snapshot.find_child_by_decl_id(parameters, key) else {
                continue;
            };
            total += snapshot.node(root).map_or(0, |node| node.child_count);
            if total > 64 {
                return Err("App Control supports 64 combined watch targets".into());
            }
            for id in snapshot.child_ids(root) {
                let Some(node) = snapshot.node(id) else {
                    continue;
                };
                if node.label.len() > 128 || node.child_count > 16 {
                    return Err("App Control watch label/structure exceeds capacity".into());
                }
                let target = if key == "watched_apps_targets" {
                    Some(node)
                } else {
                    snapshot
                        .find_child_by_decl_id(id, "target")
                        .and_then(|id| snapshot.node(id))
                };
                if matches!(target.and_then(|node|node.param_value.as_ref()),Some(ParamValue::File(text)|ParamValue::Str(text)) if text.len()>4096)
                {
                    return Err("App Control watch path exceeds 4096 bytes".into());
                }
            }
        }
        if let Some(values) = self.base.values_id() {
            for key in ["watched_apps_values", "watched_folders_values"] {
                if let Some(root) = snapshot.find_child_by_decl_id(values, key) {
                    if snapshot.node(root).is_some_and(|node| node.child_count > 64) {
                        return Err("App Control value roots exceed 64 folders".into());
                    }
                    for id in snapshot.child_ids(root) {
                        if snapshot
                            .node(id)
                            .is_some_and(|node| node.label.len() > 128 || node.child_count > 16)
                        {
                            return Err("App Control value folder label/structure exceeds capacity".into());
                        }
                    }
                }
            }
        }
        Ok(())
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
                    uuid: item.uuid,
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
                    uuid: item.uuid,
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

    fn sync_command_target_options(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        watched_apps: &[WatchedAppEntry],
    ) {
        let version = self.service.catalog_generation.advance();

        let guard = self.service.catalog_generation.reader().guard_at(version);
        self.service.catalog = Some(catalog::Cursor::new(
            snapshot,
            self.id(),
            watched_app_command_enum_options(watched_apps),
            guard,
        ));
        self.service_catalog(ctx, snapshot);
    }

    fn sync_command_target_options_for_watched_app_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
    ) {
        let Some(parameters_id) = self.base.parameters_id() else {
            return;
        };
        let Some(watched_apps_root_id) = snapshot.find_child_by_decl_id(parameters_id, "watched_apps_targets") else {
            return;
        };
        if snapshot.node(param).and_then(|node| node.parent) != Some(watched_apps_root_id) {
            return;
        }

        let Some(new_target_path) = ctx.events.iter().rev().find_map(|event| match &event.kind {
            EventKind::ParamChanged {
                param: changed_param,
                new_value: ParamValue::File(value) | ParamValue::Str(value) | ParamValue::Enum(value),
                ..
            } if *changed_param == param => Some(value.as_str()),
            _ => None,
        }) else {
            return;
        };

        if new_target_path.len() > 4096 || self.validate_watch_shape(snapshot).is_err() {
            return;
        }
        let previous_auto_label = self
            .watched_app_auto_labels
            .get(&param)
            .cloned()
            .unwrap_or_else(|| WATCHED_APP_DEFAULT_LABEL.to_string());
        let mut watched_apps = self.collect_watched_apps(snapshot);
        let next_auto_label = watched_app_label_from_target_path(new_target_path);

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
        let live_ids = watched_apps.iter().map(|entry| entry.item_id).collect::<Vec<_>>();

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

                self.watched_app_auto_labels.insert(entry.item_id, next_auto_label);
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

        sync_values_root(
            ctx,
            snapshot,
            app_values_id,
            watched_apps.iter().map(|entry| (entry.item_id, entry.label.as_str())),
            &mut self.watched_app_value_folders,
            watched_app_values_tree,
        );
        sync_values_root(
            ctx,
            snapshot,
            folder_values_id,
            watched_folders
                .iter()
                .map(|entry| (entry.item_id, entry.label.as_str())),
            &mut self.watched_folder_value_folders,
            watched_folder_values_tree,
        );
        self.service.structure_pending = self.watched_app_value_folders.len() < watched_apps.len()
            || self.watched_folder_value_folders.len() < watched_folders.len();
        if self.service.structure_pending {
            self.request_service();
        }
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
    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base
            .configure_command_tester(ctx, APP_CONTROL_MODULE_COMMAND_TYPES);
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
        self.base.set_connected(ctx, false);
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: golden_core::node::NodeCreationContext) {
        if let Err(error) = self.register_service(ctx) {
            self.warning(ctx, &error);
        }
        self.request_service();
    }
    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.service(ctx);
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service(ctx);
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn worker_wake_when_disabled(&self) -> bool {
        !self.service.pending.is_empty()
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.app-control")
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
        if let Some(method) = APP_CONTROL_SCRIPT_METHODS
            .iter()
            .copied()
            .find(|value| *value == method)
        {
            self.capture_script(ctx, method, args)?;
            return Ok(true);
        }
        self.base.engine_call_script_method(ctx, method, args)
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            if self.is_watch_configuration_param(&snapshot, param) {
                self.watch_config_dirty = true;
                self.sync_command_target_options_for_watched_app_param_change(ctx, &snapshot, param);
                self.request_service();
            }
            self.capture_running(ctx, &snapshot, param);
            self.base.emit_script_param_callback(ctx, &snapshot, param, &old_value);
        }
    }
    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        if self.watch_structure_changed(&snapshot, parent) {
            self.watch_config_dirty = true;
            self.request_service();
        }
        if (self.is_watch_item_root(&snapshot, parent)
            || snapshot
                .node(child)
                .is_some_and(|node| is_app_control_command_type(&node.node_type)))
            && self.validate_watch_shape(&snapshot).is_ok()
        {
            let apps = self.collect_watched_apps(&snapshot);
            self.sync_command_target_options(ctx, &snapshot, &apps);
        }
    }
    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        if self.watch_structure_changed(&snapshot, parent) {
            self.watch_config_dirty = true;
            self.request_service();
            if self.validate_watch_shape(&snapshot).is_ok() {
                let apps = self
                    .collect_watched_apps(&snapshot)
                    .into_iter()
                    .filter(|entry| entry.item_id != child)
                    .collect::<Vec<_>>();
                self.sync_command_target_options(ctx, &snapshot, &apps);
            }
        }
    }
    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if patch.label.is_none() && patch.short_name.is_none() && patch.enabled.is_none() {
            return;
        }
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            if snapshot
                .node(node)
                .and_then(|node| node.parent)
                .is_some_and(|parent| self.is_watch_item_root(&snapshot, parent))
            {
                self.watch_config_dirty = true;
                self.request_service();
            }
        }
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.accept_output(ctx, event);
    }
    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, _enabled: bool) {
        self.service.retire();
        self.watch_config_dirty = true;
        self.base.set_connected(ctx, false);
        self.request_service();
    }
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.service.retire();
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
        (node_type == WATCHED_APP_NODE_TYPE).then(|| Box::new(create_watched_app_parameter()) as Box<dyn Node>)
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
        vec![UserCreatableItem::new(
            AppControlWatchedFolder::NODE_TYPE,
            WATCHED_FOLDER_ITEM_KIND,
            "Watch Folder",
        )
        .with_select_when_created(false)]
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
