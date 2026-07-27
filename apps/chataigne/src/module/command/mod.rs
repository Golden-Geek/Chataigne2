use std::{borrow::Cow, collections::HashMap};

use golden_core::{
    events::CustomEvent,
    node,
    node::{DeclId, Node, NodeData, NodeId, NodeReference, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

pub const MODULE_COMMAND_ITEM_KIND: &str = "module_command";
pub const MODULE_COMMAND_REQUEST_TOPIC: &str = "chataigne.module.command.request";
/// Topic for asking a specific command node to run once (state-machine outputs
/// fire one event per lane, which the command turns into a module request).
pub const MODULE_COMMAND_EXECUTE_TOPIC: &str = "chataigne.module.command.execute";
pub const MODULE_COMMAND_TESTER_LABEL: &str = "Command Tester";
pub const MODULE_COMMAND_TESTER_DESCRIPTION: &str = "Create and trigger ad-hoc commands through this module.";
pub const MODULE_COMMAND_TARGET_MODULE_PATH: &str = "target_module";
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

fn module_command_manual_triggered(snapshot: &ProcessTreeSnapshot, command_id: NodeId, changed_param: NodeId) -> bool {
    resolve_module_command_control(snapshot, command_id, MODULE_COMMAND_TRIGGER_PATH)
        .is_some_and(|trigger_id| trigger_id == changed_param)
}

fn module_command_auto_triggered(snapshot: &ProcessTreeSnapshot, command_id: NodeId, changed_param: NodeId) -> bool {
    let Some(auto_trigger_id) = resolve_module_command_control(snapshot, command_id, MODULE_COMMAND_AUTO_TRIGGER_PATH)
    else {
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
    if snapshot
        .node(changed_param)
        .is_none_or(|node| node.param_value.is_none())
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

fn node_is_descendant_of(snapshot: &ProcessTreeSnapshot, node_id: NodeId, ancestor_id: NodeId) -> bool {
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

fn resolve_module_command_control(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<NodeId> {
    find_direct_child_by_decl_id(snapshot, command_id, path).or_else(|| snapshot.find_child(command_id, path))
}

fn find_direct_child_by_decl_id(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    snapshot
        .child_ids(parent)
        .into_iter()
        .find(|child_id| snapshot.node(*child_id).is_some_and(|child| child.decl_id == decl_id))
}

fn find_descendant_by_decl_id(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<NodeId> {
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

fn resolve_linked_module_root(snapshot: &ProcessTreeSnapshot, command_id: NodeId) -> Option<NodeId> {
    let target_module_param = resolve_module_command_child(snapshot, command_id, MODULE_COMMAND_TARGET_MODULE_PATH)?;
    let reference = snapshot
        .node(target_module_param)
        .and_then(|node| node.param_value.as_ref())
        .and_then(|value| match value {
            ParamValue::Reference(reference) => Some(reference),
            _ => None,
        })?;
    let module_id = reference
        .cached_id()
        .filter(|node_id| snapshot.node(*node_id).is_some())
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))?;
    snapshot
        .node(module_id)
        .filter(|node| {
            crate::app::declared_user_item_type_matches(&node.node_type, crate::app::module::MODULE_ITEM_KIND)
        })
        .map(|_| module_id)
}

pub(crate) fn emit_module_command_request<T: Serialize>(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    command_type: &str,
    payload: &T,
) -> Result<(), String> {
    let module_id = resolve_linked_module_root(snapshot, command_id)
        .or_else(|| crate::app::module::resolve_enclosing_module_root(snapshot, command_id))
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

    ctx.emit_custom_payload(MODULE_COMMAND_REQUEST_TOPIC, Some(module_id), &event)
        .map_err(|error| format!("failed to emit module command request: {error}"))
}

pub(crate) fn decode_module_command_request(event: &CustomEvent) -> Option<ModuleCommandRequestEvent> {
    (event.topic == MODULE_COMMAND_REQUEST_TOPIC)
        .then(|| event.payload_as::<ModuleCommandRequestEvent>().ok())
        .flatten()
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub(crate) struct ModuleCommandInvocationId {
    /// Node that owns the stream interner.
    pub emitter: NodeId,
    /// Collision-free stream allocated monotonically by that emitter.
    pub stream: u64,
}

impl ModuleCommandInvocationId {
    pub(crate) const fn new(emitter: NodeId, stream: u64) -> Self {
        Self { emitter, stream }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModuleCommandDeliveryPolicy {
    #[default]
    Standard,
    ChangeAwareLogAdmitted,
}

impl ModuleCommandDeliveryPolicy {
    fn is_standard(value: &Self) -> bool {
        *value == Self::Standard
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct ModuleCommandExecuteEvent {
    pub command_id: NodeId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_overrides: ModuleCommandParamOverrides,
    /// Runtime producer identity used by idempotent sinks. Manual/UI invocations omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_id: Option<ModuleCommandInvocationId>,
    #[serde(default, skip_serializing_if = "ModuleCommandDeliveryPolicy::is_standard")]
    pub delivery_policy: ModuleCommandDeliveryPolicy,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct ModuleCommandParamOverride {
    pub param_id: NodeId,
    pub value: ParamValue,
}

pub(crate) type ModuleCommandParamOverrides = Vec<ModuleCommandParamOverride>;

pub(crate) fn emit_command_execute_with_invocation(
    ctx: &mut ProcessCtx,
    command_id: NodeId,
    param_overrides: ModuleCommandParamOverrides,
    invocation_id: Option<ModuleCommandInvocationId>,
    delivery_policy: ModuleCommandDeliveryPolicy,
) -> Result<(), String> {
    let event = ModuleCommandExecuteEvent {
        command_id,
        param_overrides,
        invocation_id,
        delivery_policy,
    };
    ctx.emit_transient_custom_payload(MODULE_COMMAND_EXECUTE_TOPIC, Some(command_id), &event)
        .map_err(|error| format!("failed to emit module command execute: {error}"))
}

/// Returns `true` when `event` asks `command_id` to run.
pub(crate) fn is_command_execute_request(event: &CustomEvent, command_id: NodeId) -> bool {
    command_execute_request(event, command_id).is_some()
}

pub(crate) fn command_execute_request(event: &CustomEvent, command_id: NodeId) -> Option<ModuleCommandExecuteEvent> {
    (event.topic == MODULE_COMMAND_EXECUTE_TOPIC)
        .then(|| event.payload_as::<ModuleCommandExecuteEvent>().ok())
        .flatten()
        .filter(|decoded| decoded.command_id == command_id)
}

pub(crate) fn command_execute_param_overrides(
    event: &CustomEvent,
    command_id: NodeId,
) -> Option<ModuleCommandParamOverrides> {
    command_execute_request(event, command_id).map(|decoded| decoded.param_overrides)
}

pub(crate) fn command_execute_snapshot<'a>(
    event: &CustomEvent,
    snapshot: &'a ProcessTreeSnapshot,
    command_id: NodeId,
) -> Cow<'a, ProcessTreeSnapshot> {
    let Some(overrides) = command_execute_param_overrides(event, command_id) else {
        return Cow::Borrowed(snapshot);
    };
    if overrides.is_empty() {
        return Cow::Borrowed(snapshot);
    }
    Cow::Owned(
        snapshot.with_param_values(
            overrides
                .into_iter()
                .map(|override_value| (override_value.param_id, override_value.value)),
        ),
    )
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

#[node("module_command_tester", label = MODULE_COMMAND_TESTER_LABEL)]
pub struct ModuleCommandTester {
    #[state(default = None::<Vec<String>>, persist)]
    available_command_types: Option<Vec<String>>,
    manager: ModuleCommandManagerBase,
}

impl ModuleCommandTester {
    pub fn create(available_command_types: &'static [&'static str]) -> Self {
        Self::create_owned(command_type_names(available_command_types))
    }

    pub fn create_owned(available_command_types: Vec<String>) -> Self {
        Self::create_with_available_command_types(Some(available_command_types))
    }

    fn create_for_project_decode() -> Self {
        Self::create_with_available_command_types(None)
    }

    fn create_with_available_command_types(available_command_types: Option<Vec<String>>) -> Self {
        let mut tester = Self::new(ModuleCommandManagerBase::new());
        tester.available_command_types = available_command_types;
        tester.node_data_mut().meta.description = Some(MODULE_COMMAND_TESTER_DESCRIPTION.to_string());
        tester
    }

    pub(crate) fn set_available_command_types(&mut self, available_command_types: &'static [&'static str]) {
        self.available_command_types = Some(command_type_names(available_command_types));
    }

    fn command_type_available(&self, node_type: &str) -> bool {
        self.available_command_types
            .as_ref()
            .map(|available_command_types| available_command_types.iter().any(|available| available == node_type))
            .unwrap_or_else(|| crate::app::declared_user_item_type_matches(node_type, MODULE_COMMAND_ITEM_KIND))
    }

    fn available_command_items(&self) -> Vec<UserCreatableItem> {
        let declared_items = crate::app::declared_user_creatable_items(MODULE_COMMAND_ITEM_KIND);

        match self.available_command_types.as_ref() {
            Some(available_command_types) => {
                let mut items_by_type = declared_items
                    .into_iter()
                    .map(|item| (item.node_type.clone(), item))
                    .collect::<HashMap<_, _>>();

                available_command_types
                    .iter()
                    .filter_map(|node_type| items_by_type.remove(node_type))
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
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

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

fn command_type_names(available_command_types: &[&str]) -> Vec<String> {
    available_command_types
        .iter()
        .map(|node_type| (*node_type).to_string())
        .collect()
}

#[node("module_command_base", label = "Command")]
#[children(
    target_module: NodeReference = NodeReference::default() (
        label = "Module",
        description = "Module instance that will execute this command when it is used outside the module tree.",
        reference_target_kind = golden_core::parameter::ReferenceTargetKind::AnyNode,
        show_in_inspector_content = false
    );
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
    meta.description = Some("Run this command automatically when one of its command parameters changes.".to_string());
    meta.presentation.show_in_inspector_content = false;
    parameter
}

#[cfg(test)]
mod tests;
