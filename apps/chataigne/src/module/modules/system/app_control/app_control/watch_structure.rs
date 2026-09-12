use super::*;

pub(super) fn apply_duplicate_label_warning_set<'a, I>(ctx: &mut ProcessCtx, items: I, message: &str)
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

pub(super) fn apply_optional_warning(
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

pub(super) fn unique_labels_in_order<'a, I>(labels: I) -> Vec<String>
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

pub(super) struct ValueRootSyncResult {
    pub(super) removed_folder_ids: Vec<NodeId>,
}

pub(super) fn watched_app_values_tree(label: &str) -> NodeTree {
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

pub(super) fn watched_folder_values_tree(label: &str) -> NodeTree {
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

pub(super) fn create_values_folder(label: &str) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}

pub(super) fn create_watched_app_parameter() -> Parameter {
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

pub(super) fn create_running_control_param() -> Parameter {
    let mut parameter = Parameter::new(
        "Running",
        ParamValue::Bool(false),
        ParameterChangeCheck::ValueChange,
    );
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter.node_data_mut().meta.description = Some(
        "Indicates if the watched application is currently running and can launch or stop it."
            .to_string(),
    );
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId("running".to_string());
    meta.short_name = "running".to_string();
    parameter
}

pub(super) fn create_read_only_param(label: &str, decl_id: &str, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.short_name = decl_id.to_string();
    parameter
}

pub(super) fn create_read_only_float_param(
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

pub(super) fn create_read_only_time_param(label: &str, decl_id: &str, value: f64) -> Parameter {
    let mut parameter = create_read_only_float_param(label, decl_id, value, Some(0.0), None);
    parameter.ui_hints.widget = Some("time".to_string());
    parameter
}

pub(super) fn last_changed_ago_seconds(update: &FolderWatchUpdate) -> f64 {
    if update.last_change_ms == 0 {
        return 0.0;
    }

    update
        .timestamp_ms
        .saturating_sub(update.last_change_ms) as f64
        / 1000.0
}

pub(super) fn child_ids_by_label(snapshot: &ProcessTreeSnapshot, parent: NodeId) -> HashMap<String, NodeId> {
    let mut by_label = HashMap::new();
    for child_id in snapshot.child_ids(parent) {
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        by_label.entry(child.label.clone()).or_insert(child_id);
    }
    by_label
}

pub(super) fn sync_values_root<'a, I>(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
    items: I,
    value_folders_by_item: &mut HashMap<NodeId, NodeId>,
    build_tree: fn(&str) -> NodeTree,
) -> ValueRootSyncResult
where
    I: IntoIterator<Item = (NodeId, &'a str)>,
{
    let existing_by_label = child_ids_by_label(snapshot, root_id);
    let mut used_folder_ids = HashSet::new();
    let mut next_value_folders_by_item = HashMap::new();

    for (item_id, label) in items {
        let existing_folder_id = value_folders_by_item
            .get(&item_id)
            .copied()
            .filter(|folder_id| snapshot.node(*folder_id).is_some())
            .or_else(|| {
                existing_by_label
                    .get(label)
                    .copied()
                    .filter(|folder_id| !used_folder_ids.contains(folder_id))
            });

        match existing_folder_id {
            Some(folder_id) => {
                used_folder_ids.insert(folder_id);
                next_value_folders_by_item.insert(item_id, folder_id);

                if snapshot.node(folder_id).is_some_and(|node| node.label != label) {
                    ctx.patch_node_meta(
                        folder_id,
                        NodeMetaPatch {
                            label: Some(label.to_string()),
                            ..Default::default()
                        },
                    );
                }
            }
            None => {
                ctx.add_child_tree(root_id, build_tree(label), None);
            }
        }
    }

    let mut removed_folder_ids = Vec::new();
    for child_id in snapshot.child_ids(root_id) {
        if used_folder_ids.contains(&child_id) {
            continue;
        }

        removed_folder_ids.push(child_id);
        NodeHandle::new(child_id).remove(ctx);
    }

    *value_folders_by_item = next_value_folders_by_item;

    ValueRootSyncResult {
        removed_folder_ids,
    }
}

pub(super) fn descendant_ids_matching_command_types(snapshot: &ProcessTreeSnapshot, root_id: NodeId) -> Vec<NodeId> {
    let mut pending = snapshot.child_ids(root_id);
    let mut matching = Vec::new();

    while let Some(node_id) = pending.pop() {
        let Some(node) = snapshot.node(node_id) else {
            continue;
        };

        if is_app_control_command_type(node.node_type.as_str()) {
            matching.push(node_id);
        }

        pending.extend(snapshot.child_ids(node_id));
    }

    matching
}

pub(super) fn node_is_within_subtree(snapshot: &ProcessTreeSnapshot, node_id: NodeId, root_id: NodeId) -> bool {
    let mut current = Some(node_id);
    while let Some(id) = current {
        if id == root_id {
            return true;
        }
        current = snapshot.node(id).and_then(|node| node.parent);
    }

    false
}

pub(super) fn is_app_control_command_type(node_type: &str) -> bool {
    matches!(
        node_type,
        APP_CONTROL_LAUNCH_PROCESS_COMMAND_NODE_TYPE
            | APP_CONTROL_KILL_PROCESS_COMMAND_NODE_TYPE
            | APP_CONTROL_WINDOW_CONTROL_COMMAND_NODE_TYPE
    )
}

pub(super) fn set_value_param(
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

pub(super) fn find_child_by_key(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<NodeId> {
    snapshot.child_ids(parent).into_iter().find(|child_id| {
        snapshot.node(*child_id).is_some_and(|child| {
            child.decl_id == key
                || child.decl_id.rsplit('/').next() == Some(key)
                || child.short_name == key
                || child.label == key
        })
    })
}

pub(super) fn resolve_watched_app_target<'a>(
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

pub(super) fn watched_app_label_from_target_path(target_path: &str) -> String {
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

pub(super) fn should_auto_rename_watched_app(current_label: &str, previous_auto_label: &str) -> bool {
    let trimmed_label = current_label.trim();
    trimmed_label.is_empty()
        || trimmed_label.eq_ignore_ascii_case(WATCHED_APP_DEFAULT_LABEL)
    || trimmed_label == previous_auto_label
}

pub(super) fn watched_app_command_enum_options(
    watched_apps: &[WatchedAppEntry],
) -> Vec<ParameterEnumOption> {
    unique_labels_in_order(
        watched_apps
            .iter()
            .filter(|entry| !entry.target_path.trim().is_empty())
            .map(|entry| entry.label.as_str())
            .filter(|label| {
                let trimmed = label.trim();
                !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case(WATCHED_APP_DEFAULT_LABEL)
            }),
    )
        .into_iter()
        .map(|label| ParameterEnumOption {
            variant_id: label.clone(),
            value: ParamValue::Enum(label.clone()),
            label,
            tags: Vec::new(),
            ordering: None,
        })
        .collect()
}

pub(super) fn saturating_i32(value: usize) -> i32 {
    value.min(i32::MAX as usize) as i32
}
