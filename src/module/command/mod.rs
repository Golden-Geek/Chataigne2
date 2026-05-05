use std::collections::HashMap;

use golden_core::{
    events::CustomEvent,
    node,
    node::{DeclId, Node, NodeData, NodeId, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

pub const MODULE_COMMAND_ITEM_KIND: &str = "module_command";
pub const MODULE_COMMAND_REQUEST_TOPIC: &str = "chataigne.module.command.request";
const MODULE_COMMAND_TRIGGER_PATH: &str = "trigger";
const MODULE_COMMAND_AUTO_TRIGGER_PATH: &str = "auto_trigger";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct ModuleCommandRequestEvent {
    pub module_id: NodeId,
    pub module_type: String,
    pub module_label: String,
    pub command_id: NodeId,
    pub command_type: String,
    pub command_label: String,
    pub payload: serde_json::Value,
}

pub fn enable_module_command_authoring(node_data: &mut NodeData) {
    node_data.meta.user_permissions = NodeUserPermissions::all();
}

pub(crate) fn module_command_triggered(
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    changed_param: NodeId,
) -> bool {
    module_command_manual_triggered(snapshot, command_id, changed_param)
        || module_command_auto_triggered(snapshot, command_id, changed_param)
}

fn module_command_manual_triggered(
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    changed_param: NodeId,
) -> bool {
    resolve_module_command_control(snapshot, command_id, MODULE_COMMAND_TRIGGER_PATH)
        .is_some_and(|trigger_id| trigger_id == changed_param)
}

fn module_command_auto_triggered(
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    changed_param: NodeId,
) -> bool {
    let Some(auto_trigger_id) = resolve_module_command_control(
        snapshot,
        command_id,
        MODULE_COMMAND_AUTO_TRIGGER_PATH,
    ) else {
        return false;
    };

    if changed_param == auto_trigger_id {
        return false;
    }
    if resolve_module_command_control(snapshot, command_id, MODULE_COMMAND_TRIGGER_PATH)
        .is_some_and(|trigger_id| trigger_id == changed_param)
    {
        return false;
    }
    if !node_is_descendant_of(snapshot, changed_param, command_id) {
        return false;
    }
    if !snapshot
        .node(changed_param)
        .is_some_and(|node| node.param_value.is_some())
    {
        return false;
    }

    bool_param(snapshot, auto_trigger_id)
}

fn bool_param(snapshot: &ProcessTreeSnapshot, param_id: NodeId) -> bool {
    snapshot
        .node(param_id)
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_bool)
        .unwrap_or(false)
}

fn node_is_descendant_of(
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    ancestor_id: NodeId,
) -> bool {
    let mut current = snapshot.node(node_id).and_then(|node| node.parent);
    while let Some(current_id) = current {
        if current_id == ancestor_id {
            return true;
        }
        current = snapshot.node(current_id).and_then(|node| node.parent);
    }

    false
}

pub(crate) fn resolve_module_command_child(
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    path: &str,
) -> Option<NodeId> {
    // Macro-generated reused children may expose the full path as decl_id, so
    // prefer that stable declaration before falling back to segment traversal.
    find_direct_child_by_decl_id(snapshot, command_id, path)
        .or_else(|| find_descendant_by_decl_id(snapshot, command_id, path))
        .or_else(|| snapshot.resolve_path_from(command_id, path))
}

fn resolve_module_command_control(
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    path: &str,
) -> Option<NodeId> {
    find_direct_child_by_decl_id(snapshot, command_id, path)
        .or_else(|| snapshot.find_child(command_id, path))
}

fn find_direct_child_by_decl_id(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    snapshot
        .child_ids(parent)
        .into_iter()
        .find(|child_id| snapshot.node(*child_id).is_some_and(|child| child.decl_id == decl_id))
}

fn find_descendant_by_decl_id(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    for child_id in snapshot.child_ids(parent) {
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if child.decl_id == decl_id {
            return Some(child_id);
        }
        if let Some(found) = find_descendant_by_decl_id(snapshot, child_id, decl_id) {
            return Some(found);
        }
    }
    None
}

pub(crate) fn emit_module_command_request<T: Serialize>(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    command_type: &str,
    payload: &T,
) -> Result<(), String> {
    let module_id = crate::app::module::resolve_enclosing_module_root(snapshot, command_id)
        .ok_or_else(|| "command is not attached under a module root".to_string())?;
    let module_snapshot = snapshot
        .node(module_id)
        .ok_or_else(|| "module root disappeared while executing command".to_string())?;
    let command_snapshot = snapshot
        .node(command_id)
        .ok_or_else(|| "command disappeared while executing".to_string())?;

    let event = ModuleCommandRequestEvent {
        module_id,
        module_type: module_snapshot.node_type.clone(),
        module_label: module_snapshot.label.clone(),
        command_id,
        command_type: command_type.to_string(),
        command_label: command_snapshot.label.clone(),
        payload: serde_json::to_value(payload)
            .map_err(|error| format!("failed to serialize module command payload: {error}"))?,
    };

    ctx.emit_custom_payload(MODULE_COMMAND_REQUEST_TOPIC, Some(command_id), &event)
        .map_err(|error| format!("failed to emit module command request: {error}"))
}

pub(crate) fn decode_module_command_request(event: &CustomEvent) -> Option<ModuleCommandRequestEvent> {
    (event.topic == MODULE_COMMAND_REQUEST_TOPIC)
        .then(|| event.payload_as::<ModuleCommandRequestEvent>().ok())
        .flatten()
}

#[node("module_command_manager_base", label = "Commands")]
pub struct ModuleCommandManagerBase {}

#[node("module_command_manager_base", from_struct)]
impl Node for ModuleCommandManagerBase {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_module_command_authoring(self.node_data_mut());
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[MODULE_COMMAND_ITEM_KIND]))
    }
}

impl ModuleCommandManagerBase {
    pub(crate) fn ensure_command_tester_controls(&self, ctx: &mut ProcessCtx, command_id: NodeId) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        if snapshot.node(command_id).is_none() {
            return;
        }
        if resolve_module_command_control(snapshot, command_id, MODULE_COMMAND_AUTO_TRIGGER_PATH).is_some() {
            return;
        }

        ctx.add_child_boxed(command_id, Box::new(create_auto_trigger_parameter()), None);
    }
}

#[node("module_command_tester", label = "Command Tester")]
pub struct ModuleCommandTester {
    available_command_types: Option<&'static [&'static str]>,
    manager: ModuleCommandManagerBase,
}

impl ModuleCommandTester {
    pub fn create(available_command_types: &'static [&'static str]) -> Self {
        Self::create_with_available_command_types(Some(available_command_types))
    }

    fn create_for_project_decode() -> Self {
        Self::create_with_available_command_types(None)
    }

    fn create_with_available_command_types(available_command_types: Option<&'static [&'static str]>) -> Self {
        let mut tester = Self::new(available_command_types, ModuleCommandManagerBase::new());
        tester.node_data_mut().meta.description =
            Some("Create and trigger ad-hoc commands through this module.".to_string());
        tester
    }

    pub(crate) fn set_available_command_types(&mut self, available_command_types: &'static [&'static str]) {
        self.available_command_types = Some(available_command_types);
    }

    fn command_type_available(&self, node_type: &str) -> bool {
        self.available_command_types
            .map(|available_command_types| available_command_types.contains(&node_type))
            .unwrap_or_else(|| crate::app::declared_user_item_type_matches(node_type, MODULE_COMMAND_ITEM_KIND))
    }

    fn available_command_items(&self) -> Vec<UserCreatableItem> {
        let declared_items = crate::app::declared_user_creatable_items(MODULE_COMMAND_ITEM_KIND);

        match self.available_command_types {
            Some(available_command_types) => {
                let mut items_by_type = declared_items
                    .into_iter()
                    .map(|item| (item.node_type.clone(), item))
                    .collect::<HashMap<_, _>>();

                available_command_types
                    .iter()
                    .filter_map(|node_type| items_by_type.remove(*node_type))
                    .map(|item| item.with_select_when_created(false))
                    .collect()
            }
            None => declared_items
                .into_iter()
                .filter(|item| self.command_type_available(item.node_type.as_str()))
                .map(|item| item.with_select_when_created(false))
                .collect(),
        }
    }
}

#[node("module_command_tester", via = manager, from_struct)]
impl Node for ModuleCommandTester {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "module_command_tester").then(Self::create_for_project_decode)
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        self.manager.user_container_accepts_item(item_type, item_kind)
            && self.command_type_available(item_type)
            && crate::app::declared_user_item_type_matches(item_type, MODULE_COMMAND_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        self.available_command_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        self.command_type_available(node_type)
            .then(|| crate::app::create_declared_user_item(node_type, MODULE_COMMAND_ITEM_KIND))
            .flatten()
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        if parent == self.id() {
            self.manager.ensure_command_tester_controls(ctx, child);
        }
    }
}

#[node("module_command_base", label = "Command")]
#[children(
    trigger: ParamValue = ParamValue::Trigger() (
        label = "Trigger",
        description = "Fire this trigger to run the command.",
        show_in_inspector_content = false
    );
)]
pub struct ModuleCommandBase {}

#[node("module_command_base", from_struct)]
impl Node for ModuleCommandBase {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_module_command_authoring(self.node_data_mut());
    }

    fn user_item_kind(&self) -> &str {
        MODULE_COMMAND_ITEM_KIND
    }
}

fn create_auto_trigger_parameter() -> Parameter {
    let mut parameter = Parameter::new(
        "Auto Trigger",
        ParamValue::Bool(false),
        ParameterChangeCheck::ValueChange,
    );
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(MODULE_COMMAND_AUTO_TRIGGER_PATH.to_string());
    meta.short_name = MODULE_COMMAND_AUTO_TRIGGER_PATH.to_string();
    meta.description =
        Some("Run this command automatically when one of its command parameters changes.".to_string());
    meta.presentation.show_in_inspector_content = false;
    parameter
}

#[cfg(test)]
mod tests;
