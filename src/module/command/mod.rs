use golden_core::{
    events::CustomEvent,
    node,
    node::{Node, NodeData, NodeId, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

pub const MODULE_COMMAND_ITEM_KIND: &str = "module_command";
pub const MODULE_COMMAND_REQUEST_TOPIC: &str = "chataigne.module.command.request";
const MODULE_COMMAND_EXECUTE_PATH: &str = "command/execute";
const MODULE_COMMAND_LAST_RESULT_PATH: &str = "command/last_result";

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

pub(crate) fn create_command_folder() -> golden_core::node::Folder {
    let mut folder = golden_core::node::Folder::new("Folder");
    enable_module_command_authoring(folder.node_data_mut());
    folder.node_data_mut().meta.presentation.show_in_nested_inspector = true;
    folder
}

pub(crate) fn module_command_execute_triggered(
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    changed_param: NodeId,
) -> bool {
    snapshot
        .resolve_path_from(command_id, MODULE_COMMAND_EXECUTE_PATH)
        .is_some_and(|execute_id| execute_id == changed_param)
}

pub(crate) fn set_module_command_last_result(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    result: impl Into<String>,
) {
    let Some(last_result_id) = snapshot.resolve_path_from(command_id, MODULE_COMMAND_LAST_RESULT_PATH) else {
        return;
    };
    ctx.set_param(last_result_id, ParamValue::from(result.into()));
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

#[node("module_commands_container", label = "Commands")]
pub struct ModuleCommandsContainer {}

#[node("module_commands_container", from_struct)]
impl Node for ModuleCommandsContainer {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_module_command_authoring(self.node_data_mut());
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[MODULE_COMMAND_ITEM_KIND, "folder"]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new("folder", "folder", "Folder").with_select_when_created(false)]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            "folder" => Some(Box::new(create_command_folder())),
            _ => None,
        }
    }
}

#[node("module_command_base", label = "Command")]
#[children(
    folder(command, label = "Command") {
        execute: ParamValue = ParamValue::Trigger() (
            label = "Execute",
            description = "Fire this trigger to execute the command."
        );
        last_result: String = "".to_string() (
            label = "Last Result",
            description = "Summary of the most recent command execution.",
            read_only = true
        );
    }
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
