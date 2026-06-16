mod gamepad_runtime;

use golden_core::{
    engine::NodeExecutionRule,
    events::Event,
    logerror, node,
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Parameter, ParameterEnumOption, ParameterEventBehaviour, Vec2},
    process_ctx::ProcessCtx,
};

use self::gamepad_runtime::{
    selected_gamepad, DiscoveredGamepad, GamepadAxis, GamepadButton, GamepadRuntime, GamepadRuntimeEvent,
    GamepadStateSnapshot,
};

const GAMEPAD_MODULE_UPDATE_RATE_HZ: u32 = 120;
const GAMEPAD_RUNTIME_RETRY_INTERVAL_SECS: f64 = 2.0;
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct AxisProcessing {
    dead_zone: f64,
    offset: f64,
}

#[node("gamepad_module", label = "Gamepad")]
#[children(
    folder(connection) {
        device: Enum = AUTO_GAMEPAD_VARIANT (
            label = "Device",
            description = "Gamepad device to read. Auto uses the first connected gamepad.",
            enum_options = ["auto (Auto)", "none (No Gamepad)"]
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
    runtime: Option<GamepadRuntime>,
    runtime_dirty: bool,
    runtime_retry_elapsed: f64,
    known_devices: Vec<DiscoveredGamepad>,
    axis_values: [f64; 8],
    pending_runtime_events: Vec<GamepadRuntimeEvent>,
    runtime_start_suppressed: bool,
    dpad_button_state: (bool, bool, bool, bool), // (up, down, left, right)
    last_connected_device_id: Option<String>,
    button_state: Vec<bool>, // Track previous button states to avoid duplicate logs
    devices_dirty: bool,
    was_selected: bool,
}

impl GamepadModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            true,
            GAMEPAD_RUNTIME_RETRY_INTERVAL_SECS,
            Vec::new(),
            [0.0; 8],
            Vec::new(),
            false,
            (false, false, false, false),
            None,
            vec![false; GamepadButton::ALL.len()],
            true,
            false,
        )
    }

    #[cfg(test)]
    pub(crate) fn disable_runtime_for_test(&mut self) {
        self.runtime = None;
        self.runtime_dirty = false;
        self.runtime_start_suppressed = true;
    }

    #[cfg(test)]
    pub(crate) fn enqueue_runtime_event_for_test(&mut self, event: GamepadRuntimeEvent) {
        self.pending_runtime_events.push(event);
    }

    fn ensure_runtime(&mut self, ctx: &mut ProcessCtx) {
        if self.runtime_start_suppressed {
            self.runtime_dirty = false;
            return;
        }
        if self.runtime.is_some() {
            return;
        }
        if !self.runtime_dirty && self.runtime_retry_elapsed < GAMEPAD_RUNTIME_RETRY_INTERVAL_SECS {
            return;
        }

        self.runtime_dirty = false;
        self.runtime_retry_elapsed = 0.0;
        match GamepadRuntime::spawn() {
            Ok(runtime) => {
                golden_core::log!(origin = self.id(); "Started gamepad input runtime.");
                self.runtime = Some(runtime);
                self.devices_dirty = true;
                self.clear_device_warning(ctx, GAMEPAD_RUNTIME_WARNING_ID);
            }
            Err(error) => {
                logerror!(origin = self.id(); format!("Failed to start gamepad input: {error}"));
                self.runtime = None;
                self.set_runtime_warning(ctx, error.as_str());
            }
        }
    }

    fn stop_runtime(&mut self) {
        self.runtime = None;
        if !self.known_devices.is_empty() {
            self.known_devices.clear();
            self.devices_dirty = true;
        }
        self.last_connected_device_id = None;
        self.dpad_button_state = (false, false, false, false);
        self.button_state.iter_mut().for_each(|b| *b = false);
    }

    fn drain_runtime_events(&mut self) -> Vec<GamepadRuntimeEvent> {
        let mut events = std::mem::take(&mut self.pending_runtime_events);
        if let Some(runtime) = self.runtime.as_mut() {
            events.extend(runtime.poll_events());
        }

        let devices_before = self.known_devices.len();
        for event in &events {
            self.apply_runtime_device_event(event);
        }
        if let Some(runtime) = self.runtime.as_ref() {
            self.known_devices = runtime.connected_gamepads();
        }
        if self.known_devices.len() != devices_before {
            self.devices_dirty = true;
        }

        events
    }

    fn apply_runtime_device_event(&mut self, event: &GamepadRuntimeEvent) {
        match event {
            GamepadRuntimeEvent::Connected { device } => {
                if !self
                    .known_devices
                    .iter()
                    .any(|known| known.variant_id == device.variant_id)
                {
                    self.known_devices.push(device.clone());
                }
            }
            GamepadRuntimeEvent::Disconnected { device } => {
                self.known_devices
                    .retain(|known| known.variant_id != device.variant_id);
            }
            GamepadRuntimeEvent::AxisChanged { .. }
            | GamepadRuntimeEvent::ButtonPressed { .. }
            | GamepadRuntimeEvent::ButtonReleased { .. }
            | GamepadRuntimeEvent::ButtonChanged { .. } => {}
        }
    }

    fn selected_device(&self) -> Option<DiscoveredGamepad> {
        selected_gamepad(self.device.get_ref().as_str(), self.known_devices.as_slice())
    }

    fn selected_state(&self) -> Option<GamepadStateSnapshot> {
        let runtime = self.runtime.as_ref()?;
        runtime.selected_state(self.device.get_ref().as_str())
    }

    fn update_connection_state(&mut self, ctx: &mut ProcessCtx, selected: Option<&DiscoveredGamepad>) {
        self.base.set_connected(ctx, selected.is_some());
        self.set_bool_param(ctx, InfoParam::Active, selected.is_some());
        self.set_int_param(
            ctx,
            InfoParam::ConnectedDevices,
            clamp_usize_to_i32(self.known_devices.len()),
        );

        match selected {
            Some(device) => {
                if self.last_connected_device_id.as_ref() != Some(&device.variant_id) {
                    golden_core::logsuccess!(origin = self.id(); format!("Connected to gamepad: {} ({})", device.label, device.details));
                    self.last_connected_device_id = Some(device.variant_id.clone());
                }
                self.set_int_param(ctx, InfoParam::DeviceIndex, clamp_usize_to_i32(device.index));
                self.set_string_param(ctx, InfoParam::DeviceId, device.variant_id.as_str());
                self.set_string_param(ctx, InfoParam::DeviceName, device.label.as_str());
            }
            None => {
                self.last_connected_device_id = None;
                self.set_int_param(ctx, InfoParam::DeviceIndex, -1);
                self.set_string_param(ctx, InfoParam::DeviceId, "");
                self.set_string_param(ctx, InfoParam::DeviceName, "");
            }
        }
    }

    fn sync_selected_state(&mut self, ctx: &mut ProcessCtx) {
        if let Some(state) = self.selected_state() {
            for (axis, value) in state.axes {
                self.set_axis_value(ctx, axis, value);
            }
            for (button, pressed, _value) in state.buttons {
                self.set_button_value(ctx, button, pressed);
            }
        } else if self.runtime.is_some() {
            self.reset_values(ctx);
        }
    }

    fn reset_values(&mut self, ctx: &mut ProcessCtx) {
        self.dpad_button_state = (false, false, false, false);
        self.button_state.iter_mut().for_each(|b| *b = false);
        for axis in GamepadAxis::ALL {
            self.set_axis_value(ctx, axis, 0.0);
        }
        for button in GamepadButton::ALL {
            self.set_button_value(ctx, button, false);
        }
    }

    fn handle_runtime_event(
        &mut self,
        ctx: &mut ProcessCtx,
        event: GamepadRuntimeEvent,
        selected: Option<&DiscoveredGamepad>,
    ) {
        match event {
            GamepadRuntimeEvent::Connected { device } => {
                self.emit_gamepad_callback(ctx, GAMEPAD_CONNECTED_CALLBACK, vec![device_arg(&device)]);
                if self.base.log_incoming_enabled() {
                    golden_core::log!(origin = self.id(); format!("Gamepad connected: {}", device.label));
                }
            }
            GamepadRuntimeEvent::Disconnected { device } => {
                self.emit_gamepad_callback(ctx, GAMEPAD_DISCONNECTED_CALLBACK, vec![device_arg(&device)]);
                if self.base.log_incoming_enabled() {
                    golden_core::log!(origin = self.id(); format!("Gamepad disconnected: {}", device.label));
                }
            }
            GamepadRuntimeEvent::AxisChanged {
                device,
                axis,
                value,
            } => {
                if !event_device_is_selected(&device, selected) {
                    return;
                }
                let processed = self.set_axis_value(ctx, axis, value);
                self.set_last_event(ctx, format!("{} {}", axis.label(), format_axis_value(processed)));
                self.base.emit_incoming_traffic(ctx);
                self.emit_gamepad_callback(
                    ctx,
                    GAMEPAD_AXIS_CHANGED_CALLBACK,
                    vec![
                        serde_json::json!(axis.decl_id()),
                        serde_json::json!(processed),
                        serde_json::json!(value),
                        device_arg(&device),
                    ],
                );
                if self.base.log_incoming_enabled() {
                    golden_core::log!(
                        origin = self.id();
                        format!(
                            "Gamepad axis {} = {} (raw {})",
                            axis.label(),
                            format_axis_value(processed),
                            format_axis_value(value)
                        )
                    );
                }
            }
            GamepadRuntimeEvent::ButtonPressed { device, button } => {
                if !event_device_is_selected(&device, selected) {
                    return;
                }
                self.set_button_value(ctx, button, true);
                self.set_last_event(ctx, format!("{} pressed", button.label()));
                self.base.emit_incoming_traffic(ctx);
                self.emit_gamepad_callback(
                    ctx,
                    GAMEPAD_BUTTON_PRESSED_CALLBACK,
                    vec![
                        serde_json::json!(button.decl_id()),
                        serde_json::json!(1.0),
                        device_arg(&device),
                    ],
                );
            }
            GamepadRuntimeEvent::ButtonReleased { device, button } => {
                if !event_device_is_selected(&device, selected) {
                    return;
                }
                self.set_button_value(ctx, button, false);
                self.set_last_event(ctx, format!("{} released", button.label()));
                self.base.emit_incoming_traffic(ctx);
                self.emit_gamepad_callback(
                    ctx,
                    GAMEPAD_BUTTON_RELEASED_CALLBACK,
                    vec![
                        serde_json::json!(button.decl_id()),
                        serde_json::json!(0.0),
                        device_arg(&device),
                    ],
                );
            }
            GamepadRuntimeEvent::ButtonChanged {
                device,
                button,
                value,
            } => {
                if !event_device_is_selected(&device, selected) {
                    return;
                }
                let pressed = value > 0.5;
                self.set_button_value(ctx, button, pressed);
                self.set_last_event(ctx, format!("{} {}", button.label(), format_axis_value(value)));
                self.base.emit_incoming_traffic(ctx);
                self.emit_gamepad_callback(
                    ctx,
                    GAMEPAD_BUTTON_CHANGED_CALLBACK,
                    vec![
                        serde_json::json!(button.decl_id()),
                        serde_json::json!(value),
                        serde_json::json!(pressed),
                        device_arg(&device),
                    ],
                );
            }
        }
    }

    fn set_axis_value(&mut self, ctx: &mut ProcessCtx, axis: GamepadAxis, raw_value: f64) -> f64 {
        let settings = self.axis_processing(axis);
        let value = process_axis_value(raw_value, settings.dead_zone, settings.offset);
        self.axis_values[axis_index(axis)] = value;
        self.set_axis_scalar(ctx, axis, value);
        self.sync_axis_vector(ctx, axis, value);
        value
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

    fn set_axis_scalar(&mut self, ctx: &mut ProcessCtx, axis: GamepadAxis, value: f64) {
        match axis {
            // GamepadAxis::LeftStickX => {
            //     if self.left_stick_x.is_bound() && float_changed(self.left_stick_x.get(), value) {
            //         self.left_stick_x.set(ctx, value);
            //     }
            // }
            // GamepadAxis::LeftStickY => {
            //     if self.left_stick_y.is_bound() && float_changed(self.left_stick_y.get(), value) {
            //         self.left_stick_y.set(ctx, value);
            //     }
            // }
            GamepadAxis::LeftZ => {
                if self.left_z.is_bound() && float_changed(self.left_z.get(), value) {
                    self.left_z.set(ctx, value);
                }
            }
            // GamepadAxis::RightStickX => {
            //     if self.right_stick_x.is_bound() && float_changed(self.right_stick_x.get(), value) {
            //         self.right_stick_x.set(ctx, value);
            //     }
            // }
            // GamepadAxis::RightStickY => {
            //     if self.right_stick_y.is_bound() && float_changed(self.right_stick_y.get(), value) {
            //         self.right_stick_y.set(ctx, value);
            //     }
            // }
            GamepadAxis::RightZ => {
                if self.right_z.is_bound() && float_changed(self.right_z.get(), value) {
                    self.right_z.set(ctx, value);
                }
            }
            // GamepadAxis::DPadX => {
            //     if self.dpad_x.is_bound() && float_changed(self.dpad_x.get(), value) {
            //         self.dpad_x.set(ctx, value);
            //     }
            // }
            // GamepadAxis::DPadY => {
            //     if self.dpad_y.is_bound() && float_changed(self.dpad_y.get(), value) {
            //         self.dpad_y.set(ctx, value);
            //     }
            // }
            _ => {}
        }
    }

    fn sync_axis_vector(&mut self, ctx: &mut ProcessCtx, axis: GamepadAxis, value: f64) {
        match axis {
            GamepadAxis::LeftStickX => {
                self.set_vec2_param(
                    ctx,
                    Vec2Param::LeftStick,
                    value,
                    self.axis_values[axis_index(GamepadAxis::LeftStickY)],
                );
            }
            GamepadAxis::LeftStickY => {
                self.set_vec2_param(
                    ctx,
                    Vec2Param::LeftStick,
                    self.axis_values[axis_index(GamepadAxis::LeftStickX)],
                    value,
                );
            }
            GamepadAxis::RightStickX => {
                self.set_vec2_param(
                    ctx,
                    Vec2Param::RightStick,
                    value,
                    self.axis_values[axis_index(GamepadAxis::RightStickY)],
                );
            }
            GamepadAxis::RightStickY => {
                self.set_vec2_param(
                    ctx,
                    Vec2Param::RightStick,
                    self.axis_values[axis_index(GamepadAxis::RightStickX)],
                    value,
                );
            }
            GamepadAxis::DPadX => {
                let y = self.axis_values[axis_index(GamepadAxis::DPadY)];
                self.set_vec2_param(
                    ctx,
                    Vec2Param::DPad,
                    value,
                    y,
                );
            }
            GamepadAxis::DPadY => {
                let x = self.axis_values[axis_index(GamepadAxis::DPadX)];
                self.set_vec2_param(
                    ctx,
                    Vec2Param::DPad,
                    x,
                    value,
                );
            }
            GamepadAxis::LeftZ | GamepadAxis::RightZ => {}
        }
    }

    fn set_vec2_param(&mut self, ctx: &mut ProcessCtx, param: Vec2Param, x: f64, y: f64) {
        let value: Vec2 = Vec2::new(x, y);
        match param {
            Vec2Param::LeftStick => {
                if self.left_stick.is_bound() && (float_changed(self.left_stick.get().x, x) || float_changed(self.left_stick.get().y, y)) {
                    self.left_stick.set(ctx, value);
                }
            }
            Vec2Param::RightStick => {
                if self.right_stick.is_bound() && (float_changed(self.right_stick.get().x, x) || float_changed(self.right_stick.get().y, y)) {
                    self.right_stick.set(ctx, value);
                }
            }
            Vec2Param::DPad => {
                if self.dpad.is_bound() && (float_changed(self.dpad.get().x, x) || float_changed(self.dpad.get().y, y)) {
                    self.dpad.set(ctx, value);
                }
            }
        }
    }

    fn set_button_value(&mut self, ctx: &mut ProcessCtx, button: GamepadButton, pressed: bool) {
        let button_index = button_state_index(button);
        let old_pressed = self.button_state.get(button_index).copied().unwrap_or(false);
        
        // Only log if state actually changed
        if old_pressed != pressed && self.base.log_incoming_enabled() {
            golden_core::log!(origin = self.id(); format!("Button {} {}", button.label(), if pressed { "pressed" } else { "released" }));
        }
        
        if old_pressed == pressed && !matches!(button, GamepadButton::DPadUp | GamepadButton::DPadDown | GamepadButton::DPadLeft | GamepadButton::DPadRight) {
            return; // No change, skip update
        }
        
        // Update state tracking
        if button_index < self.button_state.len() {
            self.button_state[button_index] = pressed;
        }
        
        // Track D-pad button state and synthesize axis values
        match button {
            GamepadButton::DPadUp => {
                self.dpad_button_state.0 = pressed;
                self.update_dpad_from_buttons(ctx);
                self.set_bool_param(ctx, InfoParam::DPadUp, pressed);
            }
            GamepadButton::DPadDown => {
                self.dpad_button_state.1 = pressed;
                self.update_dpad_from_buttons(ctx);
                self.set_bool_param(ctx, InfoParam::DPadDown, pressed);
            }
            GamepadButton::DPadLeft => {
                self.dpad_button_state.2 = pressed;
                self.update_dpad_from_buttons(ctx);
                self.set_bool_param(ctx, InfoParam::DPadLeft, pressed);
            }
            GamepadButton::DPadRight => {
                self.dpad_button_state.3 = pressed;
                self.update_dpad_from_buttons(ctx);
                self.set_bool_param(ctx, InfoParam::DPadRight, pressed);
            }
            GamepadButton::South => self.set_bool_param(ctx, InfoParam::South, pressed),
            GamepadButton::East => self.set_bool_param(ctx, InfoParam::East, pressed),
            GamepadButton::North => self.set_bool_param(ctx, InfoParam::North, pressed),
            GamepadButton::West => self.set_bool_param(ctx, InfoParam::West, pressed),
            GamepadButton::C => self.set_bool_param(ctx, InfoParam::C, pressed),
            GamepadButton::Z => self.set_bool_param(ctx, InfoParam::Z, pressed),
            GamepadButton::LeftTrigger => self.set_bool_param(ctx, InfoParam::LeftTrigger, pressed),
            GamepadButton::LeftTrigger2 => self.set_bool_param(ctx, InfoParam::LeftTrigger2, pressed),
            GamepadButton::RightTrigger => self.set_bool_param(ctx, InfoParam::RightTrigger, pressed),
            GamepadButton::RightTrigger2 => self.set_bool_param(ctx, InfoParam::RightTrigger2, pressed),
            GamepadButton::Select => self.set_bool_param(ctx, InfoParam::Select, pressed),
            GamepadButton::Start => self.set_bool_param(ctx, InfoParam::Start, pressed),
            GamepadButton::Mode => self.set_bool_param(ctx, InfoParam::Mode, pressed),
            GamepadButton::LeftThumb => self.set_bool_param(ctx, InfoParam::LeftThumb, pressed),
            GamepadButton::RightThumb => self.set_bool_param(ctx, InfoParam::RightThumb, pressed),
        }
    }

    fn update_dpad_from_buttons(&mut self, ctx: &mut ProcessCtx) {
        let (up, down, left, right) = self.dpad_button_state;
        let x = if right { 1.0 } else if left { -1.0 } else { 0.0 };
        let y = if up { 1.0 } else if down { -1.0 } else { 0.0 };
        
        self.axis_values[axis_index(GamepadAxis::DPadX)] = x;
        self.axis_values[axis_index(GamepadAxis::DPadY)] = y;
        
        self.set_vec2_param(ctx, Vec2Param::DPad, x, y);
    }

    fn set_bool_param(&mut self, ctx: &mut ProcessCtx, param: InfoParam, value: bool) {
        match param {
            // InfoParam::Active if self.active.is_bound() && self.active.get() != value => self.active.set(ctx, value),
            InfoParam::South if self.south.is_bound() && self.south.get() != value => self.south.set(ctx, value),
            InfoParam::East if self.east.is_bound() && self.east.get() != value => self.east.set(ctx, value),
            InfoParam::North if self.north.is_bound() && self.north.get() != value => self.north.set(ctx, value),
            InfoParam::West if self.west.is_bound() && self.west.get() != value => self.west.set(ctx, value),
            InfoParam::C if self.c.is_bound() && self.c.get() != value => self.c.set(ctx, value),
            InfoParam::Z if self.z.is_bound() && self.z.get() != value => self.z.set(ctx, value),
            InfoParam::LeftTrigger if self.left_trigger.is_bound() && self.left_trigger.get() != value => {
                self.left_trigger.set(ctx, value);
            }
            InfoParam::LeftTrigger2 if self.left_trigger_2.is_bound() && self.left_trigger_2.get() != value => {
                self.left_trigger_2.set(ctx, value);
            }
            InfoParam::RightTrigger if self.right_trigger.is_bound() && self.right_trigger.get() != value => {
                self.right_trigger.set(ctx, value);
            }
            InfoParam::RightTrigger2 if self.right_trigger_2.is_bound() && self.right_trigger_2.get() != value => {
                self.right_trigger_2.set(ctx, value);
            }
            InfoParam::Select if self.select.is_bound() && self.select.get() != value => self.select.set(ctx, value),
            InfoParam::Start if self.start.is_bound() && self.start.get() != value => self.start.set(ctx, value),
            InfoParam::Mode if self.mode.is_bound() && self.mode.get() != value => self.mode.set(ctx, value),
            InfoParam::LeftThumb if self.left_thumb.is_bound() && self.left_thumb.get() != value => {
                self.left_thumb.set(ctx, value);
            }
            InfoParam::RightThumb if self.right_thumb.is_bound() && self.right_thumb.get() != value => {
                self.right_thumb.set(ctx, value);
            }
            InfoParam::DPadUp if self.dpad_up.is_bound() && self.dpad_up.get() != value => {
                self.dpad_up.set(ctx, value);
            }
            InfoParam::DPadDown if self.dpad_down.is_bound() && self.dpad_down.get() != value => {
                self.dpad_down.set(ctx, value);
            }
            InfoParam::DPadLeft if self.dpad_left.is_bound() && self.dpad_left.get() != value => {
                self.dpad_left.set(ctx, value);
            }
            InfoParam::DPadRight if self.dpad_right.is_bound() && self.dpad_right.get() != value => {
                self.dpad_right.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_int_param(&mut self, ctx: &mut ProcessCtx, param: InfoParam, value: i32) {
        match param {
            // InfoParam::ConnectedDevices
            //     if self.connected_devices.is_bound() && self.connected_devices.get() != value =>
            // {
            //     self.connected_devices.set(ctx, value);
            // }
            InfoParam::DeviceIndex if self.device_index.is_bound() && self.device_index.get() != value => {
                self.device_index.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_string_param(&mut self, ctx: &mut ProcessCtx, param: InfoParam, value: &str) {
        match param {
            InfoParam::DeviceId if self.device_id.is_bound() && self.device_id.get_ref() != value => {
                self.device_id.set(ctx, value.to_string());
            }
            InfoParam::DeviceName if self.device_name.is_bound() && self.device_name.get_ref() != value => {
                self.device_name.set(ctx, value.to_string());
            }
            InfoParam::LastEvent if self.last_event.is_bound() && self.last_event.get_ref() != value => {
                self.last_event.set(ctx, value.to_string());
            }
            _ => {}
        }
    }

    fn set_last_event(&mut self, ctx: &mut ProcessCtx, value: String) {
        self.set_string_param(ctx, InfoParam::LastEvent, value.as_str());
    }

    fn sync_device_options(&self, ctx: &mut ProcessCtx) {
        if self.device.is_bound() {
            sync_gamepad_device_enum_options(
                ctx,
                self.device.id(),
                gamepad_device_options(self.known_devices.as_slice()),
            );
        }
    }

    fn refresh_selection_warning(&self, ctx: &mut ProcessCtx, selected: Option<&DiscoveredGamepad>) {
        if self.runtime.is_none() {
            return;
        }

        let selection = self.device.get_ref();
        if gamepad_specific_device_selected(selection.as_str()) && selected.is_none() {
            self.set_device_warning(
                ctx,
                GAMEPAD_SELECTION_WARNING_ID,
                format!(
                    "Selected gamepad '{}' is not connected.",
                    human_gamepad_variant(selection.as_str())
                )
                .as_str(),
                None,
            );
        } else {
            self.clear_device_warning(ctx, GAMEPAD_SELECTION_WARNING_ID);
        }
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        let can_receive = self.device.get_ref().as_str() != NO_GAMEPAD_VARIANT;
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(can_receive, false));
    }

    fn set_runtime_warning(&self, ctx: &mut ProcessCtx, error: &str) {
        let warning = gamepad_connection_warning(error);
        self.set_device_warning(
            ctx,
            GAMEPAD_RUNTIME_WARNING_ID,
            warning.message.as_str(),
            warning.detail.as_deref(),
        );
    }

    fn set_device_warning(&self, ctx: &mut ProcessCtx, warning_id: &str, message: &str, detail: Option<&str>) {
        if self.device.is_bound() {
            self.device
                .set_warning_with(ctx, Some(warning_id), message, detail);
        }
    }

    fn clear_device_warning(&self, ctx: &mut ProcessCtx, warning_id: &str) {
        if self.device.is_bound() {
            self.device.clear_warning(ctx, Some(warning_id));
        }
    }

    fn emit_gamepad_callback(
        &self,
        ctx: &mut ProcessCtx,
        callback: &str,
        args: Vec<serde_json::Value>,
    ) {
        crate::app::module::script_api::emit_script_callback(ctx, self.id(), callback, args);
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if self.device.is_bound() && self.device.id() == param {
            self.refresh_data_capabilities(ctx);
        }
    }
}

#[golden_core::item(
    "module",
    node = "gamepad_module",
    via = base,
    from_struct,
    menu_path = ["Controllers"]
)]
impl Node for GamepadModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, GAMEPAD_MODULE_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.ensure_runtime(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.runtime_retry_elapsed += ctx.delta_time.as_secs_f64();
        if !self.node_data().effective_enabled {
            self.stop_runtime();
            self.update_connection_state(ctx, None);
            self.reset_values(ctx);
            return;
        }

        self.ensure_runtime(ctx);

        let events = self.drain_runtime_events();
        if self.devices_dirty {
            self.sync_device_options(ctx);
            self.devices_dirty = false;
        }
        let selected = self.selected_device();
        let is_selected = selected.is_some();
        self.refresh_selection_warning(ctx, selected.as_ref());
        self.update_connection_state(ctx, selected.as_ref());

        for event in events {
            self.handle_runtime_event(ctx, event, selected.as_ref());
        }

        if is_selected || self.was_selected {
            self.sync_selected_state(ctx);
        }
        self.was_selected = is_selected;
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_runtime();
        self.pending_runtime_events.clear();
        self.runtime_dirty = false;
    }

    fn needs_update(&self) -> bool {
        self.runtime.is_some()
            || self.runtime_dirty
            || self.devices_dirty
            || !self.pending_runtime_events.is_empty()
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(GAMEPAD_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            GAMEPAD_SCRIPT_METHODS,
        )
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
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &old_value);
        }
        self.on_param_change_inner(ctx, param);
    }

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if node != self.id() && patch.enabled.is_some() {
            self.runtime_dirty = true;
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.runtime_dirty = true;
            self.runtime_retry_elapsed = GAMEPAD_RUNTIME_RETRY_INTERVAL_SECS;
        } else {
            self.stop_runtime();
            self.clear_device_warning(ctx, GAMEPAD_RUNTIME_WARNING_ID);
            self.update_connection_state(ctx, None);
            self.reset_values(ctx);
            self.runtime_dirty = false;
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[derive(Clone, Copy)]
enum Vec2Param {
    LeftStick,
    RightStick,
    DPad,
}

#[derive(Clone, Copy)]
enum InfoParam {
    Active,
    ConnectedDevices,
    DeviceIndex,
    DeviceId,
    DeviceName,
    LastEvent,
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

struct GamepadConnectionWarning {
    message: String,
    detail: Option<String>,
}

fn gamepad_connection_warning(error: &str) -> GamepadConnectionWarning {
    if permission_denied_error(error) {
        return GamepadConnectionWarning {
            message: format!(
                "{error}. On Linux, GilRs reads /dev/input/event* directly; add udev rules or grant access to the input device."
            ),
            detail: Some(format!("GilRs Linux evdev notes: {GAMEPAD_UDEV_HELP_URL}")),
        };
    }

    GamepadConnectionWarning {
        message: error.to_string(),
        detail: None,
    }
}

fn permission_denied_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("permission denied") || lower.contains("os error 13") || lower.contains("access is denied")
}

fn process_axis_value(raw_value: f64, dead_zone: f64, offset: f64) -> f64 {
    let value = (raw_value + offset).clamp(-1.0, 1.0);
    let dead_zone = dead_zone.clamp(0.0, 1.0);
    if dead_zone >= 1.0 || value.abs() <= dead_zone {
        return 0.0;
    }

    let scaled = (value.abs() - dead_zone) / (1.0 - dead_zone);
    scaled.copysign(value).clamp(-1.0, 1.0)
}

fn event_device_is_selected(device: &DiscoveredGamepad, selected: Option<&DiscoveredGamepad>) -> bool {
    selected.is_some_and(|selected| selected.variant_id == device.variant_id)
}

fn axis_index(axis: GamepadAxis) -> usize {
    match axis {
        GamepadAxis::LeftStickX => 0,
        GamepadAxis::LeftStickY => 1,
        GamepadAxis::LeftZ => 2,
        GamepadAxis::RightStickX => 3,
        GamepadAxis::RightStickY => 4,
        GamepadAxis::RightZ => 5,
        GamepadAxis::DPadX => 6,
        GamepadAxis::DPadY => 7,
    }
}

fn device_arg(device: &DiscoveredGamepad) -> serde_json::Value {
    serde_json::json!({
        "id": device.variant_id,
        "index": device.index,
        "name": device.label,
        "details": device.details,
    })
}

fn float_changed(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() > 0.000_001
}

fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn format_axis_value(value: f64) -> String {
    format!("{value:.4}")
}

fn gamepad_device_options(gamepads: &[DiscoveredGamepad]) -> Vec<ParameterEnumOption> {
    let mut options = vec![
        enum_option(AUTO_GAMEPAD_VARIANT, "Auto", 0),
        enum_option(NO_GAMEPAD_VARIANT, "No Gamepad", 1),
    ];

    options.extend(gamepads.iter().enumerate().map(|(index, gamepad)| {
        let mut option = enum_option(gamepad.variant_id.as_str(), gamepad.label.as_str(), 10 + index as i32);
        option.tags = vec![gamepad.details.clone()];
        option
    }));
    options
}

fn sync_gamepad_device_enum_options(ctx: &mut ProcessCtx, param_id: NodeId, options: Vec<ParameterEnumOption>) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("gamepad device target is not a parameter".to_string());
        };

        let mut next_options = options.clone();
        let current_variant = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .unwrap_or_else(|| AUTO_GAMEPAD_VARIANT.to_string());

        if gamepad_specific_device_selected(current_variant.as_str())
            && !next_options
                .iter()
                .any(|option| option.variant_id == current_variant)
        {
            next_options.insert(2, missing_gamepad_option(current_variant.as_str()));
        }

        let next_value = if next_options
            .iter()
            .any(|option| option.variant_id == current_variant)
        {
            ParamValue::Enum(current_variant.clone())
        } else {
            ParamValue::Enum(AUTO_GAMEPAD_VARIANT.to_string())
        };

        if parameter.constraints.enum_options == next_options && parameter.value == next_value {
            return Ok(());
        }

        let label = parameter.node_data().meta.label.clone();
        let change_check = parameter.change_check.clone();
        let mut replacement = Parameter::new(label.as_str(), next_value, change_check);
        *replacement.node_data_mut() = parameter.node_data().clone();
        replacement.default_value = parameter.default_value.clone();
        replacement.event_behaviour = ParameterEventBehaviour::Coalesce;
        replacement.read_only = parameter.read_only;
        replacement.persist_read_only_value = parameter.persist_read_only_value;
        replacement.constraints = parameter.constraints.clone();
        replacement.constraints.enum_options = next_options;
        replacement.ui_hints = parameter.ui_hints.clone();
        replacement.control = parameter.control.clone();
        replacement.control_modes_enabled = parameter.control_modes_enabled;

        inner_ctx.replace_node(param_id, replacement);
        Ok(())
    });
}

fn gamepad_specific_device_selected(selection: &str) -> bool {
    let trimmed = selection.trim();
    !trimmed.is_empty() && trimmed != AUTO_GAMEPAD_VARIANT && trimmed != NO_GAMEPAD_VARIANT
}

fn missing_gamepad_option(variant_id: &str) -> ParameterEnumOption {
    let mut option = enum_option(
        variant_id,
        format!("Missing: {}", human_gamepad_variant(variant_id)).as_str(),
        5,
    );
    option.tags = vec!["missing".to_string()];
    option
}

fn human_gamepad_variant(variant_id: &str) -> String {
    variant_id
        .split_once('|')
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| variant_id.to_string())
}

fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

fn button_state_index(button: GamepadButton) -> usize {
    GamepadButton::ALL.iter().position(|&b| b == button).unwrap_or(0)
}

#[cfg(test)]
mod gamepad_tests;
