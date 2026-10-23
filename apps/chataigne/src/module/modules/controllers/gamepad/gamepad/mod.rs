mod devices;
mod gamepad_runtime;
mod projection;
mod service;
use devices::*;
#[cfg(test)]
use gamepad_runtime::process_axis_value;
use gamepad_runtime::{AxisProcessing, DiscoveredGamepad, GamepadAxis, GamepadRuntime};
use golden_core::{
    engine::NodeExecutionRule,
    node,
    node::{Node, NodeCreationContext, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, ParameterEnumOption, Vec2},
    process_ctx::ProcessCtx,
};
const GAMEPAD_RUNTIME_WARNING_ID: &str = "gamepad_runtime";
const GAMEPAD_SELECTION_WARNING_ID: &str = "gamepad_selection";
const GAMEPAD_UDEV_HELP_URL: &str = "https://docs.rs/gilrs/latest/gilrs/#linuxbsd-evdev";
const NO_GAMEPAD_VARIANT: &str = "none";
const AUTO_GAMEPAD_VARIANT: &str = "auto";
const GAMEPAD_CONNECTED_CALLBACK: &str = "gamepadConnected";
const GAMEPAD_DISCONNECTED_CALLBACK: &str = "gamepadDisconnected";
const GAMEPAD_AXIS_CHANGED_CALLBACK: &str = "gamepadAxisChanged";
const GAMEPAD_BUTTON_PRESSED_CALLBACK: &str = "gamepadButtonPressed";
const GAMEPAD_BUTTON_RELEASED_CALLBACK: &str = "gamepadButtonReleased";
const GAMEPAD_BUTTON_CHANGED_CALLBACK: &str = "gamepadButtonChanged";
const GAMEPAD_SCRIPT_METHODS: &[&str] = &[];
const GAMEPAD_MODULE_COMMAND_TYPES: &[&str] = &[];
#[node("gamepad_module", label = "Gamepad")]
#[children(
    folder(connection) {
        device: Enum = AUTO_GAMEPAD_VARIANT (
            label = "Device",
            description = "Gamepad device to read. Auto uses the first connected gamepad.",
            enum_options = gamepad_device_options(&[])
        );
        [base_children];
    }
    folder(parameters) {
        folder(axis_processing, label = "Axis Processing", collapsed = true) {
            folder(left_stick_x, label = "Left Stick X") {
                left_stick_x_dead_zone: f64 = 0.0 [0.0..1.0] (
                    label = "Dead Zone",
                    description = "Centered range ignored for Left Stick X after offset is applied."
                );
                left_stick_x_offset: f64 = 0.0 [-1.0..1.0] (
                    label = "Offset",
                    description = "Calibration offset added to Left Stick X before dead-zone processing."
                );
            }
            folder(left_stick_y, label = "Left Stick Y") {
                left_stick_y_dead_zone: f64 = 0.0 [0.0..1.0] (
                    label = "Dead Zone",
                    description = "Centered range ignored for Left Stick Y after offset is applied."
                );
                left_stick_y_offset: f64 = 0.0 [-1.0..1.0] (
                    label = "Offset",
                    description = "Calibration offset added to Left Stick Y before dead-zone processing."
                );
            }
            folder(left_z, label = "Left Z") {
                left_z_dead_zone: f64 = 0.0 [0.0..1.0] (
                    label = "Dead Zone",
                    description = "Centered range ignored for Left Z after offset is applied."
                );
                left_z_offset: f64 = 0.0 [-1.0..1.0] (
                    label = "Offset",
                    description = "Calibration offset added to Left Z before dead-zone processing."
                );
            }
            folder(right_stick_x, label = "Right Stick X") {
                right_stick_x_dead_zone: f64 = 0.0 [0.0..1.0] (
                    label = "Dead Zone",
                    description = "Centered range ignored for Right Stick X after offset is applied."
                );
                right_stick_x_offset: f64 = 0.0 [-1.0..1.0] (
                    label = "Offset",
                    description = "Calibration offset added to Right Stick X before dead-zone processing."
                );
            }
            folder(right_stick_y, label = "Right Stick Y") {
                right_stick_y_dead_zone: f64 = 0.0 [0.0..1.0] (
                    label = "Dead Zone",
                    description = "Centered range ignored for Right Stick Y after offset is applied."
                );
                right_stick_y_offset: f64 = 0.0 [-1.0..1.0] (
                    label = "Offset",
                    description = "Calibration offset added to Right Stick Y before dead-zone processing."
                );
            }
            folder(right_z, label = "Right Z") {
                right_z_dead_zone: f64 = 0.0 [0.0..1.0] (
                    label = "Dead Zone",
                    description = "Centered range ignored for Right Z after offset is applied."
                );
                right_z_offset: f64 = 0.0 [-1.0..1.0] (
                    label = "Offset",
                    description = "Calibration offset added to Right Z before dead-zone processing."
                );
            }
            folder(dpad_x, label = "D-Pad X") {
                dpad_x_dead_zone: f64 = 0.0 [0.0..1.0] (
                    label = "Dead Zone",
                    description = "Centered range ignored for D-Pad X after offset is applied."
                );
                dpad_x_offset: f64 = 0.0 [-1.0..1.0] (
                    label = "Offset",
                    description = "Calibration offset added to D-Pad X before dead-zone processing."
                );
            }
            folder(dpad_y, label = "D-Pad Y") {
                dpad_y_dead_zone: f64 = 0.0 [0.0..1.0] (
                    label = "Dead Zone",
                    description = "Centered range ignored for D-Pad Y after offset is applied."
                );
                dpad_y_offset: f64 = 0.0 [-1.0..1.0] (
                    label = "Offset",
                    description = "Calibration offset added to D-Pad Y before dead-zone processing."
                );
            }
        }
        [base_children];
    }
    folder(values) {
        folder(info, label = "Info", collapsed = true) {
            active: bool = false (
                label = "Active",
                description = "Whether a gamepad is currently selected and connected.",
                read_only = true
            );
            connected_devices: i32 = 0 [0..2147483647] (
                label = "Connected Devices",
                description = "Number of gamepads currently visible to the runtime.",
                read_only = true
            );
            device_index: i32 = -1 [-1..2147483647] (
                label = "Device Index",
                description = "Runtime index for the active gamepad, or -1 when none is active.",
                read_only = true
            );
            device_id: String = String::new() (
                label = "Device ID",
                description = "Stable selection id for the active gamepad during this runtime.",
                read_only = true
            );
            device_name: String = String::new() (
                label = "Device Name",
                description = "Name reported by the active gamepad.",
                read_only = true
            );
            last_event: String = String::new() (
                label = "Last Event",
                description = "Last selected gamepad event processed by this module.",
                read_only = true
            );
        }
        folder(axes, label = "Axes") {
            left_stick: Vec2 = (0.0, 0.0) [(-1.0,-1.0)..(1.0, 1.0)] (
                label = "Left Stick",
                description = "Processed left stick X/Y values after per-axis offset and dead zone.",
                read_only = true
            );

            left_z: f64 = 0.0 [-1.0..1.0] (
                label = "Left Z",
                description = "Processed Left Z value.",
                read_only = true
            );

            right_stick: Vec2 = (0.0, 0.0) [(-1.0,-1.0)..(1.0, 1.0)] (
                label = "Right Stick",
                description = "Processed right stick X/Y values after per-axis offset and dead zone.",
                read_only = true
            );

            right_z: f64 = 0.0 [-1.0..1.0] (
                label = "Right Z",
                description = "Processed Right Z value.",
                read_only = true
            );

            dpad: Vec2 = (0.0, 0.0) [(-1.0,-1.0)..(1.0, 1.0)](
                label = "D-Pad",
                description = "Processed D-Pad X/Y axis values after per-axis offset and dead zone.",
                read_only = true
            );
        }
        folder(buttons, label = "Buttons") {
            south: bool = false (label = "South", read_only = true);
            east: bool = false (label = "East", read_only = true);
            north: bool = false (label = "North", read_only = true);
            west: bool = false (label = "West", read_only = true);
            c: bool = false (label = "C", read_only = true);
            z: bool = false (label = "Z", read_only = true);
            left_trigger: bool = false (label = "Left Trigger", read_only = true);
            left_trigger_2: bool = false (label = "Left Trigger 2", read_only = true);
            right_trigger: bool = false (label = "Right Trigger", read_only = true);
            right_trigger_2: bool = false (label = "Right Trigger 2", read_only = true);
            select: bool = false (label = "Select", read_only = true);
            start: bool = false (label = "Start", read_only = true);
            mode: bool = false (label = "Mode", read_only = true);
            left_thumb: bool = false (label = "Left Thumb", read_only = true);
            right_thumb: bool = false (label = "Right Thumb", read_only = true);
            dpad_up: bool = false (label = "D-Pad Up", read_only = true);
            dpad_down: bool = false (label = "D-Pad Down", read_only = true);
            dpad_left: bool = false (label = "D-Pad Left", read_only = true);
            dpad_right: bool = false (label = "D-Pad Right", read_only = true);
        }
        [base_children];
    }
)]

pub struct GamepadModule {
    base: crate::app::ModuleBase,
    service: service::Service,
}
impl GamepadModule {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::new(), service::Service::new())
    }
    #[cfg(test)]
    pub(crate) fn disable_runtime_for_test(&mut self) {
        self.stop_runtime();
        self.service.suppressed = true;
    }
    fn axis_processing(&self, axis: GamepadAxis) -> AxisProcessing {
        match axis {
            GamepadAxis::LeftStickX => AxisProcessing {
                dead_zone: self.left_stick_x_dead_zone.get(),
                offset: self.left_stick_x_offset.get(),
            },
            GamepadAxis::LeftStickY => AxisProcessing {
                dead_zone: self.left_stick_y_dead_zone.get(),
                offset: self.left_stick_y_offset.get(),
            },
            GamepadAxis::LeftZ => AxisProcessing {
                dead_zone: self.left_z_dead_zone.get(),
                offset: self.left_z_offset.get(),
            },
            GamepadAxis::RightStickX => AxisProcessing {
                dead_zone: self.right_stick_x_dead_zone.get(),
                offset: self.right_stick_x_offset.get(),
            },
            GamepadAxis::RightStickY => AxisProcessing {
                dead_zone: self.right_stick_y_dead_zone.get(),
                offset: self.right_stick_y_offset.get(),
            },
            GamepadAxis::RightZ => AxisProcessing {
                dead_zone: self.right_z_dead_zone.get(),
                offset: self.right_z_offset.get(),
            },
            GamepadAxis::DPadX => AxisProcessing {
                dead_zone: self.dpad_x_dead_zone.get(),
                offset: self.dpad_x_offset.get(),
            },
            GamepadAxis::DPadY => AxisProcessing {
                dead_zone: self.dpad_y_dead_zone.get(),
                offset: self.dpad_y_offset.get(),
            },
        }
    }

    fn axis_config(&self) -> [AxisProcessing; 8] {
        GamepadAxis::ALL.map(|axis| self.axis_processing(axis))
    }
    fn settings(&self) -> gamepad_runtime::Settings {
        gamepad_runtime::Settings {
            requested: self.device.get_ref().as_str().into(),
            axes: self.axis_config(),
        }
    }
    fn project_values(&mut self, ctx: &mut ProcessCtx, axes: [f64; 8], buttons: [bool; 19]) {
        self.left_stick.set(ctx, Vec2::new(axes[0], axes[1]));
        self.left_z.set(ctx, axes[2]);
        self.right_stick.set(ctx, Vec2::new(axes[3], axes[4]));
        self.right_z.set(ctx, axes[5]);
        self.dpad.set(ctx, Vec2::new(axes[6], axes[7]));
        self.south.set(ctx, buttons[0]);
        self.east.set(ctx, buttons[1]);
        self.north.set(ctx, buttons[2]);
        self.west.set(ctx, buttons[3]);
        self.c.set(ctx, buttons[4]);
        self.z.set(ctx, buttons[5]);
        self.left_trigger.set(ctx, buttons[6]);
        self.left_trigger_2.set(ctx, buttons[7]);
        self.right_trigger.set(ctx, buttons[8]);
        self.right_trigger_2.set(ctx, buttons[9]);
        self.select.set(ctx, buttons[10]);
        self.start.set(ctx, buttons[11]);
        self.mode.set(ctx, buttons[12]);
        self.left_thumb.set(ctx, buttons[13]);
        self.right_thumb.set(ctx, buttons[14]);
        self.dpad_up.set(ctx, buttons[15]);
        self.dpad_down.set(ctx, buttons[16]);
        self.dpad_left.set(ctx, buttons[17]);
        self.dpad_right.set(ctx, buttons[18]);
    }
    fn reset_values(&mut self, ctx: &mut ProcessCtx) {
        self.project_values(ctx, [0.0; 8], [false; 19]);
    }
    fn project_metadata(&mut self, ctx: &mut ProcessCtx, selected: Option<&DiscoveredGamepad>, count: usize) {
        self.base.set_connected(ctx, selected.is_some());
        self.active.set(ctx, selected.is_some());
        self.connected_devices.set(ctx, count as i32);
        self.device_index.set(ctx, selected.map_or(-1, |d| d.index as i32));
        self.device_id
            .set(ctx, selected.map_or(String::new(), |d| d.variant_id.clone()));
        self.device_name
            .set(ctx, selected.map_or(String::new(), |d| d.label.clone()));
    }
    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                self.device.get_ref().as_str() != NO_GAMEPAD_VARIANT,
                false,
            ),
        );
    }
    fn set_runtime_warning(&self, ctx: &mut ProcessCtx, error: &str) {
        let warning = gamepad_connection_warning(error);
        self.device.set_warning_with(
            ctx,
            Some(GAMEPAD_RUNTIME_WARNING_ID),
            warning.message,
            warning.detail.as_deref(),
        );
    }
    fn clear_runtime_warning(&self, ctx: &mut ProcessCtx) {
        self.device.clear_warning(ctx, Some(GAMEPAD_RUNTIME_WARNING_ID));
    }
    fn on_settings_change(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        let selection = self.device.is_bound() && self.device.id() == param;
        let calibration = self.calibration_ids().contains(&param);
        if !selection && !calibration {
            return;
        }
        self.configure_runtime(ctx);
        if selection {
            self.refresh_data_capabilities(ctx);
        }
        self.request_service();
    }
}
#[golden_core::item("module",node="gamepad_module",via=base,from_struct,menu_path=["Controllers"])]
impl Node for GamepadModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, GAMEPAD_MODULE_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
    }
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if let Err(error) = self.register_service(ctx) {
            self.set_runtime_warning(ctx, &error);
        }
        self.request_service();
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service_runtime(ctx);
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_runtime();
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.gamepad")
    }
    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), GAMEPAD_SCRIPT_METHODS)
    }
    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        self.base.engine_call_script_method(ctx, method, args)
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.base.emit_script_param_callback(ctx, &snapshot, param, &old_value);
        }
        self.on_settings_change(ctx, param);
    }
    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.service.retry_at = None;
            self.refresh_data_capabilities(ctx);
            self.request_service();
        } else {
            self.stop_runtime();
            self.clear_runtime_warning(ctx);
            self.device.clear_warning(ctx, Some(GAMEPAD_SELECTION_WARNING_ID));
            self.device.clear_warning(ctx, Some("gamepad_pressure"));
            self.project_metadata(ctx, None, 0);
            self.reset_values(ctx);
        }
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}
#[cfg(test)]
mod tests;
