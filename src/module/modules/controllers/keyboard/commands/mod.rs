use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::keyboard::{
        keyboard_key_action_enum_options, keyboard_key_enum_options, KeyboardKey,
        KeyboardKeyAction, KeyboardKeyRequest, KEYBOARD_ACTION_TAP,
        KEYBOARD_KEY_COMMAND_NODE_TYPE, KEYBOARD_KEY_SPACE,
    },
    module_command,
};

macro_rules! keyboard_command_node_impl {
    ($context:literal) => {
        fn child_event_interest_depth(&self, event: &Event) -> u32 {
            match event.kind {
                EventKind::ParamChanged { .. } => u32::MAX,
                _ => 0,
            }
        }

        fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return;
            };
            let snapshot = snapshot_arc.as_ref();
            if !module_command::module_command_triggered(snapshot, self.id(), param) {
                return;
            }

            if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
                module_command::emit_module_command_request(
                    ctx,
                    snapshot,
                    self.id(),
                    self.get_type(),
                    &payload,
                )
            }) {
                golden_core::logerror!(format!("Failed to trigger {}: {error}", $context));
            }
        }

        fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
            if !module_command::is_command_execute_request(&event, self.id()) {
                return;
            }
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return;
            };
            let snapshot = snapshot_arc.as_ref();
            if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
                module_command::emit_module_command_request(
                    ctx,
                    snapshot,
                    self.id(),
                    self.get_type(),
                    &payload,
                )
            }) {
                golden_core::logerror!(format!("Failed to execute {}: {error}", $context));
            }
        }
    };
}

#[node("keyboard_key_command", label = "Keyboard Key")]
#[children(
    key: Enum = KEYBOARD_KEY_SPACE (
        label = "Key",
        description = "Physical key to control.",
        enum_options = keyboard_key_enum_options()
    );
    action: Enum = KEYBOARD_ACTION_TAP (
        label = "Action",
        description = "Key action to send.",
        enum_options = keyboard_key_action_enum_options()
    );
)]
pub struct KeyboardKeyCommand {
    base: crate::app::ModuleCommandBase,
}

impl KeyboardKeyCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<KeyboardKeyRequest, String> {
        let key = command_enum_param(snapshot, self.id(), "key")
            .map(|value| KeyboardKey::parse(value.as_str()))
            .transpose()?
            .unwrap_or(KeyboardKey::Space);
        let action = command_enum_param(snapshot, self.id(), "action")
            .map(|value| KeyboardKeyAction::parse(value.as_str()))
            .transpose()?
            .unwrap_or(KeyboardKeyAction::Tap);

        Ok(KeyboardKeyRequest {
            key: key.as_str().to_string(),
            action,
            description: format!("{} {} key", action.as_str(), key.as_str()),
        })
    }
}

#[golden_core::item("module_command", node = "keyboard_key_command", via = base, from_struct)]
impl Node for KeyboardKeyCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == KEYBOARD_KEY_COMMAND_NODE_TYPE).then(Self::create)
    }

    keyboard_command_node_impl!("keyboard key command");
}

fn command_enum_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
    })
}