use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeData, NodeId, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    parameter::{Enum, ParamValue},
    process_ctx::ProcessCtx,
};
use serde::{Deserialize, Serialize};

pub const MODULE_COMMAND_ITEM_KIND: &str = "module_command";
pub const MODULE_COMMAND_MODE_DIRECT: &str = "direct";
pub const MODULE_COMMAND_MODE_SYNC_VALUES: &str = "sync_values";
pub const MODULE_COMMAND_EXECUTED_TOPIC: &str = "chataigne.module.command.executed";
pub const MODULE_COMMAND_DIRECT_REQUEST_TOPIC: &str = "chataigne.module.command.direct_request";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct ModuleCommandEvent {
    pub module_id: NodeId,
    pub module_type: String,
    pub module_label: String,
    pub command_id: NodeId,
    pub command_type: String,
    pub command_label: String,
    pub dispatch_mode: String,
    pub payload: serde_json::Value,
    pub target_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ModuleValueWrite {
    pub path: String,
    pub value: ParamValue,
}

pub fn enable_module_command_authoring(node_data: &mut NodeData) {
    node_data.meta.user_permissions = NodeUserPermissions::all();
}

pub fn execute_module_command(
    ctx: &mut ProcessCtx,
    command_id: NodeId,
    command_type: &str,
    dispatch_mode: &Enum,
    payload: serde_json::Value,
    value_writes: Vec<ModuleValueWrite>,
) -> Result<String, String> {
    let snapshot = ctx
        .tree_snapshot_arc()
        .ok_or_else(|| "module command execution requires a tree snapshot".to_string())?;
    let module_id = crate::app::module::resolve_enclosing_module_root(snapshot.as_ref(), command_id)
        .ok_or_else(|| "command is not attached under a module root".to_string())?;
    let module_snapshot = snapshot
        .node(module_id)
        .ok_or_else(|| "module root disappeared while executing command".to_string())?;
    let command_snapshot = snapshot
        .node(command_id)
        .ok_or_else(|| "command disappeared while executing".to_string())?;

    let event = ModuleCommandEvent {
        module_id,
        module_type: module_snapshot.node_type.clone(),
        module_label: module_snapshot.label.clone(),
        command_id,
        command_type: command_type.to_string(),
        command_label: command_snapshot.label.clone(),
        dispatch_mode: dispatch_mode.as_str().to_string(),
        payload,
        target_paths: value_writes.iter().map(|write| write.path.clone()).collect(),
    };

    match dispatch_mode.as_str() {
        MODULE_COMMAND_MODE_DIRECT => {
            ctx.emit_custom_payload(MODULE_COMMAND_DIRECT_REQUEST_TOPIC, Some(command_id), &event)
                .map_err(|error| format!("failed to serialize direct command payload: {error}"))?;
            ctx.emit_custom_payload(MODULE_COMMAND_EXECUTED_TOPIC, Some(command_id), &event)
                .map_err(|error| format!("failed to serialize command execution payload: {error}"))?;

            Ok(format!("Sent '{}' directly to the interface", command_snapshot.label))
        }
        MODULE_COMMAND_MODE_SYNC_VALUES => {
            if value_writes.is_empty() {
                return Err(format!(
                    "command '{}' does not define any module value targets for sync mode",
                    command_snapshot.label
                ));
            }

            for write in &value_writes {
                let target_id = snapshot
                    .resolve_path_from(module_id, write.path.as_str())
                    .ok_or_else(|| format!("module value path '{}' does not exist", write.path))?;
                let target_snapshot = snapshot
                    .node(target_id)
                    .ok_or_else(|| format!("module value path '{}' no longer exists", write.path))?;
                if target_snapshot.param_value.is_none() {
                    return Err(format!("module value path '{}' is not a parameter", write.path));
                }
                ctx.set_param(target_id, write.value.clone());
            }

            ctx.emit_custom_payload(MODULE_COMMAND_EXECUTED_TOPIC, Some(command_id), &event)
                .map_err(|error| format!("failed to serialize command execution payload: {error}"))?;

            Ok(format!("Updated {} module value(s)", value_writes.len()))
        }
        other => Err(format!("unsupported module command dispatch mode '{other}'")),
    }
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

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        (item_type == "folder" && item_kind == "folder") || item_kind == MODULE_COMMAND_ITEM_KIND
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        Vec::new()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            "module_command_base" => Some(Box::new(crate::app::ModuleCommandBase::new())),
            "folder" => Some(Box::new(golden_core::node::Folder::new("Folder"))),
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
        dispatch_mode: Enum = "direct" (
            label = "Dispatch Mode",
            description = "Either send directly to the remote interface or update module values first.",
            enum_options = ["direct", "sync_values"],
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

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ParamChanged { .. } => u32::MAX,
            _ => 0,
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(execute_id) = snapshot.resolve_path_from(self.id(), "command/execute") else {
            return;
        };
        if param != execute_id {
            return;
        }

        let Some(dispatch_mode_id) = snapshot.resolve_path_from(self.id(), "command/dispatch_mode") else {
            return;
        };
        let Some(dispatch_mode_snapshot) = snapshot.node(dispatch_mode_id) else {
            return;
        };
        let Some(dispatch_mode) = dispatch_mode_snapshot
            .param_value
            .as_ref()
            .and_then(ParamValue::as_enum)
            .map(Enum::new)
        else {
            return;
        };

        let result = execute_module_command(
            ctx,
            self.id(),
            self.get_type(),
            &dispatch_mode,
            serde_json::Value::Null,
            Vec::new(),
        )
        .unwrap_or_else(|error| error);

        let Some(last_result_id) = snapshot.resolve_path_from(self.id(), "command/last_result") else {
            return;
        };
        ctx.set_param(last_result_id, ParamValue::from(result));
    }

    fn user_item_kind(&self) -> &str {
        MODULE_COMMAND_ITEM_KIND
    }
}
