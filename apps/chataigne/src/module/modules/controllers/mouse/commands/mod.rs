use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::ProcessCtx,
};

use crate::app::{
    module::common::mouse::{
        MOUSE_ACTION_CLICK, MOUSE_BUTTON_COMMAND_NODE_TYPE, MOUSE_BUTTON_LEFT, MOUSE_COORDINATE_ABSOLUTE,
        MOUSE_MOVE_COMMAND_NODE_TYPE, MOUSE_SCROLL_COMMAND_NODE_TYPE, MOUSE_UNITS_PIXELS,
    },
    module_command,
};
macro_rules! mouse_command_node_impl {
    ($context:literal) => {
        fn child_event_interest_depth(&self, event: &Event) -> u32 {
            if matches!(event.kind, EventKind::ParamChanged { .. }) {
                u32::MAX
            } else {
                0
            }
        }
        fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old: ParamValue) {
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
    if let Err(error) = module_command::CapturedInvocation::capture(&snapshot, command, event, &["mouse_module"])
        .and_then(|invocation| crate::app::queue_mouse_command(ctx, invocation))
    {
        golden_core::logerror!(format!("Mouse command was not admitted: {error}"));
    }
}

#[node("mouse_move_command", label = "Move Mouse")]
#[children(
    coordinate: Enum = MOUSE_COORDINATE_ABSOLUTE (
        label = "Coordinate",
        description = "Absolute moves go to a point. Relative moves apply X/Y deltas.",
        enum_options = ["absolute", "relative"]
    );
    units: Enum = MOUSE_UNITS_PIXELS (
        label = "Units",
        description = "Normalized coordinates use 0.0-1.0 across the main display and only support Absolute moves.",
        enum_options = ["pixels", "normalized"]
    );
    x: f64 = 0.0 (
        label = "X",
        description = "Absolute X position or relative horizontal delta.",
        widget = "text"
    );
    y: f64 = 0.0 (
        label = "Y",
        description = "Absolute Y position or relative vertical delta.",
        widget = "text"
    );
)]
pub struct MouseMoveCommand {
    base: crate::app::ModuleCommandBase,
}

impl MouseMoveCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "mouse_move_command", via = base, from_struct)]
impl Node for MouseMoveCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == MOUSE_MOVE_COMMAND_NODE_TYPE).then(Self::create)
    }

    mouse_command_node_impl!("mouse move command");
}

#[node("mouse_button_command", label = "Mouse Button")]
#[children(
    button: Enum = MOUSE_BUTTON_LEFT (
        label = "Button",
        description = "Mouse button to control.",
        enum_options = ["left", "middle", "right"]
    );
    action: Enum = MOUSE_ACTION_CLICK (
        label = "Action",
        description = "Button action to send.",
        enum_options = ["click", "press", "release"]
    );
)]
pub struct MouseButtonCommand {
    base: crate::app::ModuleCommandBase,
}

impl MouseButtonCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "mouse_button_command", via = base, from_struct)]
impl Node for MouseButtonCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == MOUSE_BUTTON_COMMAND_NODE_TYPE).then(Self::create)
    }

    mouse_command_node_impl!("mouse button command");
}

#[node("mouse_scroll_command", label = "Scroll Mouse")]
#[children(
    vertical: i32 = 0 (
        label = "Vertical",
        description = "Vertical wheel clicks. Positive values scroll down, negative values scroll up.",
        widget = "text"
    );
    horizontal: i32 = 0 (
        label = "Horizontal",
        description = "Horizontal wheel clicks. Positive values scroll right, negative values scroll left.",
        widget = "text"
    );
)]
pub struct MouseScrollCommand {
    base: crate::app::ModuleCommandBase,
}

impl MouseScrollCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "mouse_scroll_command", via = base, from_struct)]
impl Node for MouseScrollCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == MOUSE_SCROLL_COMMAND_NODE_TYPE).then(Self::create)
    }

    mouse_command_node_impl!("mouse scroll command");
}
