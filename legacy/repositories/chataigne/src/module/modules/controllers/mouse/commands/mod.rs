use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::mouse::{
        MouseButtonAction, MouseButtonKind, MouseButtonRequest, MouseMoveCoordinate,
        MouseMoveRequest, MouseMoveUnits, MouseScrollRequest, MOUSE_ACTION_CLICK,
        MOUSE_BUTTON_COMMAND_NODE_TYPE, MOUSE_BUTTON_LEFT, MOUSE_MOVE_COMMAND_NODE_TYPE,
        MOUSE_COORDINATE_ABSOLUTE, MOUSE_SCROLL_COMMAND_NODE_TYPE, MOUSE_UNITS_PIXELS,
    },
    module_command,
};

macro_rules! mouse_command_node_impl {
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
            let snapshot = module_command::command_execute_snapshot(
                &event,
                snapshot_arc.as_ref(),
                self.id(),
            );
            let snapshot = snapshot.as_ref();
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

#[node("mouse_move_command", label = "Move Mouse")]
#[children(
    coordinate: Enum = MOUSE_COORDINATE_ABSOLUTE (
        label = "Coordinate",
        description = "Absolute moves go to a point. Relative moves apply X/Y deltas.",
        enum_options = ["absolute (Absolute)", "relative (Relative)"]
    );
    units: Enum = MOUSE_UNITS_PIXELS (
        label = "Units",
        description = "Normalized coordinates use 0.0-1.0 across the main display and only support Absolute moves.",
        enum_options = ["pixels (Pixels)", "normalized (Normalized 0-1)"]
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MouseMoveRequest, String> {
        let coordinate = command_enum_param(snapshot, self.id(), "coordinate")
            .map(|value| MouseMoveCoordinate::parse(value.as_str()))
            .transpose()?
            .unwrap_or(MouseMoveCoordinate::Absolute);
        let units = command_enum_param(snapshot, self.id(), "units")
            .map(|value| MouseMoveUnits::parse(value.as_str()))
            .transpose()?
            .unwrap_or(MouseMoveUnits::Pixels);
        if coordinate == MouseMoveCoordinate::Relative && units == MouseMoveUnits::Normalized {
            return Err(
                "normalized mouse movement only supports absolute coordinates on the main display"
                    .to_string(),
            );
        }

        Ok(MouseMoveRequest {
            x: command_float_param(snapshot, self.id(), "x").unwrap_or(0.0),
            y: command_float_param(snapshot, self.id(), "y").unwrap_or(0.0),
            coordinate,
            units,
            description: "move mouse".to_string(),
        })
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
        enum_options = ["left (Left)", "middle (Middle)", "right (Right)"]
    );
    action: Enum = MOUSE_ACTION_CLICK (
        label = "Action",
        description = "Button action to send.",
        enum_options = ["click (Click)", "press (Press)", "release (Release)"]
    );
)]
pub struct MouseButtonCommand {
    base: crate::app::ModuleCommandBase,
}

impl MouseButtonCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MouseButtonRequest, String> {
        let button = command_enum_param(snapshot, self.id(), "button")
            .map(|value| MouseButtonKind::parse(value.as_str()))
            .transpose()?
            .unwrap_or(MouseButtonKind::Left);
        let action = command_enum_param(snapshot, self.id(), "action")
            .map(|value| MouseButtonAction::parse(value.as_str()))
            .transpose()?
            .unwrap_or(MouseButtonAction::Click);

        Ok(MouseButtonRequest {
            button,
            action,
            description: format!("{} {} mouse button", action.as_str(), button.as_str()),
        })
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MouseScrollRequest, String> {
        Ok(MouseScrollRequest {
            vertical: command_int_param(snapshot, self.id(), "vertical").unwrap_or(0),
            horizontal: command_int_param(snapshot, self.id(), "horizontal").unwrap_or(0),
            description: "scroll mouse".to_string(),
        })
    }
}

#[golden_core::item("module_command", node = "mouse_scroll_command", via = base, from_struct)]
impl Node for MouseScrollCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == MOUSE_SCROLL_COMMAND_NODE_TYPE).then(Self::create)
    }

    mouse_command_node_impl!("mouse scroll command");
}

fn command_enum_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
    })
}

fn command_float_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<f64> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_float)
    })
}

fn command_int_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<i32> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_int)
    })
}
