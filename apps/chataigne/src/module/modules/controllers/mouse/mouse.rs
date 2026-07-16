mod mouse_runtime;

use golden_core::{
    engine::NodeExecutionRule,
    events::CustomEvent,
    logerror, node,
    node::{Node, NodeId, NodeScriptDescriptor, NodeHandle},
    parameter::{Enum, ParamValue, Parameter, ParameterEnumOption, ParameterEventBehaviour, Vec2},
    process_ctx::ProcessCtx,
};

use crate::app::module::common::mouse::{
    MouseButtonAction, MouseButtonKind, MouseButtonRequest, MouseMoveCoordinate,
    MouseMoveRequest, MouseMoveUnits, MouseScrollRequest, MOUSE_BUTTON_COMMAND_NODE_TYPE,
    MOUSE_MODULE_COMMAND_TYPES, MOUSE_MOVE_COMMAND_NODE_TYPE, MOUSE_SCROLL_COMMAND_NODE_TYPE,
};

use self::mouse_runtime::{
    DiscoveredMouseDevice, MouseInputConfig, MouseInputEvent, MouseInputRuntime,
    MouseOutputController, MouseRuntimeEvent,
};

const MOUSE_MODULE_UPDATE_RATE_HZ: u32 = 120;
const MOUSE_BACKEND_RETRY_INTERVAL_SECS: f64 = 2.0;
const MOUSE_INPUT_WARNING_ID: &str = "mouse_input_backend";
const MOUSE_OUTPUT_WARNING_ID: &str = "mouse_output_backend";
const MOUSE_SELECTION_WARNING_ID: &str = "mouse_selection";

const AUTO_MOUSE_VARIANT: &str = "auto";
const NO_MOUSE_VARIANT: &str = "none";

const MOUSE_MOVED_CALLBACK: &str = "mouseMoved";
const MOUSE_BUTTON_PRESSED_CALLBACK: &str = "mouseButtonPressed";
const MOUSE_BUTTON_RELEASED_CALLBACK: &str = "mouseButtonReleased";
const MOUSE_BUTTON_CHANGED_CALLBACK: &str = "mouseButtonChanged";

const MOUSE_SCRIPT_METHODS: &[&str] = &[
    "moveMouse",
    "click",
    "pressButton",
    "releaseButton",
    "scroll",
];

#[node("mouse_module", label = "Mouse")]
#[children(
    folder(connection) {
        device: Enum = AUTO_MOUSE_VARIANT (
            label = "Input Device",
            description = "Mouse device to read. Auto uses the first connected mouse. Output remains system-wide.",
            enum_options = ["auto (Auto)", "none (No Input Device)"]
        );
        receive_enabled: bool = true (
            label = "Receive Input",
            description = "Receive input from the selected local mouse device."
        );
        capture_os_input: bool = false (
            label = "Capture OS Mouse Input",
            description = "Windows only. Temporarily blocks physical mouse input from reaching the OS after the selected mouse becomes active. Other mice keep working while the selected mouse is idle. Capture remains global during that activity window, and injected output still passes through."
        );
        send_enabled: bool = true (
            label = "Send Output",
            description = "Allow this module to move the local system mouse, click buttons, and scroll. Output is always system-wide."
        );
        [base_children];
    }
    folder(parameters) {
        [base_children];
    }
    folder(values) {
        folder(info, label = "Info", collapsed = true) {
            input_active: bool = false (
                label = "Input Active",
                description = "Whether the selected mouse input device is currently available.",
                read_only = true
            );
            output_active: bool = false (
                label = "Output Active",
                description = "Whether mouse output injection is currently available.",
                read_only = true
            );
            connected_devices: i32 = 0 [0..2147483647] (
                label = "Connected Devices",
                description = "Number of mouse devices currently visible to the input runtime.",
                read_only = true
            );
            device_id: String = String::new() (
                label = "Device ID",
                description = "Stable id for the selected mouse device during this runtime.",
                read_only = true
            );
            device_name: String = String::new() (
                label = "Device Name",
                description = "Label for the selected mouse device.",
                read_only = true
            );
            last_event: String = String::new() (
                label = "Last Event",
                description = "Last mouse input or output event handled by this module.",
                read_only = true
            );
        }
        folder(pointer, label = "Pointer") {
            position: Vec2 = (0.0, 0.0) [(-100000.0,-100000.0)..(100000.0,100000.0)] (
                label = "Position",
                description = "Latest observed mouse position in pixels.",
                read_only = true
            );
            delta: Vec2 = (0.0, 0.0) [(-100000.0,-100000.0)..(100000.0,100000.0)] (
                label = "Delta",
                description = "Latest observed mouse movement delta in pixels.",
                read_only = true
            );
        }
        folder(buttons, label = "Buttons") {
            left: bool = false (label = "Left", read_only = true);
            middle: bool = false (label = "Middle", read_only = true);
            right: bool = false (label = "Right", read_only = true);
        }
        [base_children];
    }
)]
pub struct MouseModule {
    base: crate::app::ModuleBase,
    input_backend: Option<MouseInputRuntime>,
    output_backend: Option<MouseOutputController>,
    pending_input_events: Vec<MouseInputEvent>,
    known_devices: Vec<DiscoveredMouseDevice>,
    devices_dirty: bool,
    input_retry_elapsed: f64,
    output_retry_elapsed: f64,
    last_input_error: Option<String>,
    last_output_error: Option<String>,
    suppress_input_backend: bool,
    suppress_output_backend: bool,
}

impl MouseModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            None,
            Vec::new(),
            Vec::new(),
            true,
            MOUSE_BACKEND_RETRY_INTERVAL_SECS,
            MOUSE_BACKEND_RETRY_INTERVAL_SECS,
            None,
            None,
            false,
            false,
        )
    }

    #[cfg(test)]
    pub(crate) fn disable_backends_for_test(&mut self) {
        self.suppress_input_backend = true;
        self.suppress_output_backend = true;
        self.input_backend = None;
        self.output_backend = None;
        self.pending_input_events.clear();
        self.known_devices.clear();
        self.devices_dirty = true;
    }

    #[cfg(test)]
    pub(crate) fn enqueue_input_event_for_test(&mut self, event: MouseInputEvent) {
        self.pending_input_events.push(event);
    }

    fn ensure_input_backend(&mut self, ctx: &mut ProcessCtx) {
        if !self.receive_enabled.get() {
            self.stop_input_backend();
            self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
            return;
        }
        if self.suppress_input_backend || self.input_backend.is_some() {
            return;
        }
        if self.input_retry_elapsed < MOUSE_BACKEND_RETRY_INTERVAL_SECS {
            return;
        }

        let _ = self.try_start_input_backend(ctx);
    }

    fn ensure_output_backend(&mut self, ctx: &mut ProcessCtx) {
        if !self.send_enabled.get() {
            self.stop_output_backend();
            self.clear_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID);
            return;
        }
        if self.suppress_output_backend || self.output_backend.is_some() {
            return;
        }
        if self.output_retry_elapsed < MOUSE_BACKEND_RETRY_INTERVAL_SECS {
            return;
        }

        let _ = self.try_start_output_backend(ctx);
    }

    fn try_start_input_backend(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        if self.suppress_input_backend {
            return Err("mouse input backend is suppressed for tests".to_string());
        }

        self.input_retry_elapsed = 0.0;
        match MouseInputRuntime::create(self.input_config()) {
            Ok(backend) => {
                if self.capture_os_input.get() {
                    golden_core::log!(
                        origin = self.id();
                        "Started mouse input backend with activity-gated OS mouse capture enabled."
                    );
                } else {
                    golden_core::log!(origin = self.id(); "Started mouse input backend.");
                }
                self.input_backend = Some(backend);
                self.last_input_error = None;
                self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
                self.refresh_connection_state(ctx);
                Ok(())
            }
            Err(error) => {
                if self.last_input_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Failed to start mouse input backend: {error}"));
                }
                self.input_backend = None;
                self.last_input_error = Some(error.clone());
                self.set_backend_warning(ctx, MOUSE_INPUT_WARNING_ID, error.as_str());
                self.refresh_connection_state(ctx);
                Err(error)
            }
        }
    }

    fn try_start_output_backend(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        if self.suppress_output_backend {
            return Err("mouse output backend is suppressed for tests".to_string());
        }

        self.output_retry_elapsed = 0.0;
        match MouseOutputController::create() {
            Ok(backend) => {
                golden_core::log!(origin = self.id(); "Started mouse output backend.");
                self.output_backend = Some(backend);
                self.last_output_error = None;
                self.clear_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID);
                self.refresh_connection_state(ctx);
                Ok(())
            }
            Err(error) => {
                if self.last_output_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Failed to start mouse output backend: {error}"));
                }
                self.output_backend = None;
                self.last_output_error = Some(error.clone());
                self.set_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID, error.as_str());
                self.refresh_connection_state(ctx);
                Err(error)
            }
        }
    }

    fn stop_input_backend(&mut self) {
        self.input_backend = None;
        if !self.known_devices.is_empty() {
            self.known_devices.clear();
            self.devices_dirty = true;
        }
    }

    fn stop_output_backend(&mut self) {
        self.output_backend = None;
    }

    fn restart_input_backend(&mut self, ctx: &mut ProcessCtx) {
        self.input_backend = None;
        self.input_retry_elapsed = MOUSE_BACKEND_RETRY_INTERVAL_SECS;
        let _ = self.try_start_input_backend(ctx);
    }

    fn drain_input_events(&mut self, ctx: &mut ProcessCtx) -> Vec<MouseInputEvent> {
        let mut events = std::mem::take(&mut self.pending_input_events);

        let Some(input_backend) = self.input_backend.as_mut() else {
            return events;
        };

        let runtime_events = match input_backend.poll_events() {
            Ok(runtime_events) => runtime_events,
            Err(error) => {
                if self.last_input_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Mouse input runtime stopped: {error}"));
                }
                self.stop_input_backend();
                self.last_input_error = Some(error.clone());
                self.set_backend_warning(ctx, MOUSE_INPUT_WARNING_ID, error.as_str());
                self.refresh_connection_state(ctx);
                return events;
            }
        };

        for runtime_event in runtime_events {
            match runtime_event {
                MouseRuntimeEvent::DevicesChanged(devices) => {
                    if self.known_devices != devices {
                        self.known_devices = devices;
                        self.devices_dirty = true;
                    }
                }
                MouseRuntimeEvent::Input { device, event } => {
                    if self.input_event_matches_selection(device.as_str()) {
                        events.push(event);
                    }
                }
            }
        }

        events
    }

    fn handle_input_events(&mut self, ctx: &mut ProcessCtx, events: Vec<MouseInputEvent>) {
        if events.is_empty() {
            self.set_delta(ctx, 0, 0);
            return;
        }

        let device_label = self
            .selected_input_device()
            .map(|device| device.label)
            .unwrap_or_else(|| "Selected Mouse".to_string());
        self.set_delta(ctx, 0, 0);
        for event in events {
            match event {
                MouseInputEvent::Moved { x, y, dx, dy } => {
                    self.set_position(ctx, x, y);
                    self.set_delta(ctx, dx, dy);
                    self.set_last_event(ctx, format!("{device_label}: moved mouse to ({x}, {y})"));
                    if self.base.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Observed {device_label} move to ({x}, {y}) with delta ({dx}, {dy})")
                        );
                    }
                    self.emit_mouse_callback(
                        ctx,
                        MOUSE_MOVED_CALLBACK,
                        vec![
                            serde_json::json!({ "x": x, "y": y }),
                            serde_json::json!({ "x": dx, "y": dy }),
                            self.mouse_state_arg(),
                        ],
                    );
                }
                MouseInputEvent::ButtonChanged { button, pressed } => {
                    self.set_input_button(ctx, button, pressed);
                    let action = if pressed { "pressed" } else { "released" };
                    self.set_last_event(ctx, format!("{device_label}: {} button {action}", button.label()));
                    if self.base.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Observed {device_label} {} mouse button {action}", button.label().to_ascii_lowercase())
                        );
                    }
                    self.emit_mouse_callback(
                        ctx,
                        MOUSE_BUTTON_CHANGED_CALLBACK,
                        vec![serde_json::json!(button.as_str()), serde_json::json!(pressed), self.mouse_state_arg()],
                    );
                    self.emit_mouse_callback(
                        ctx,
                        if pressed {
                            MOUSE_BUTTON_PRESSED_CALLBACK
                        } else {
                            MOUSE_BUTTON_RELEASED_CALLBACK
                        },
                        vec![serde_json::json!(button.as_str()), self.mouse_state_arg()],
                    );
                }
            }
        }

        self.base.emit_incoming_traffic(ctx);
    }

    fn execute_move_request(&mut self, ctx: &mut ProcessCtx, request: MouseMoveRequest) -> Result<(), String> {
        let message = self.output_backend_mut(ctx)?.execute_move(&request)?;
        self.finish_output_request(ctx, message);
        Ok(())
    }

    fn execute_button_request(&mut self, ctx: &mut ProcessCtx, request: MouseButtonRequest) -> Result<(), String> {
        let message = self
            .output_backend_mut(ctx)?
            .execute_button(request.button, request.action)?;
        self.finish_output_request(ctx, message);
        Ok(())
    }

    fn execute_scroll_request(&mut self, ctx: &mut ProcessCtx, request: MouseScrollRequest) -> Result<(), String> {
        let message = self.output_backend_mut(ctx)?.execute_scroll(&request)?;
        self.finish_output_request(ctx, message);
        Ok(())
    }

    fn output_backend_mut(
        &mut self,
        ctx: &mut ProcessCtx,
    ) -> Result<&mut MouseOutputController, String> {
        if !self.node_data().effective_enabled {
            return Err("mouse module is disabled".to_string());
        }
        if !self.send_enabled.get() {
            return Err("mouse output is disabled".to_string());
        }
        if self.output_backend.is_none() {
            self.try_start_output_backend(ctx)?;
        }

        self.output_backend
            .as_mut()
            .ok_or_else(|| "mouse output backend is unavailable".to_string())
    }

    fn finish_output_request(&mut self, ctx: &mut ProcessCtx, message: String) {
        self.clear_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID);
        self.base.emit_outgoing_traffic(ctx);
        if self.base.log_outgoing_enabled() {
            golden_core::log!(origin = self.id(); format!("{message}"));
        }
        self.set_last_event(ctx, message);
        self.refresh_connection_state(ctx);
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !MOUSE_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str()) {
            return;
        }

        let result = match request.command_type.as_str() {
            MOUSE_MOVE_COMMAND_NODE_TYPE => serde_json::from_value::<MouseMoveRequest>(request.payload)
                .map_err(|error| format!("invalid mouse move command payload: {error}"))
                .and_then(|payload| self.execute_move_request(ctx, payload)),
            MOUSE_BUTTON_COMMAND_NODE_TYPE => serde_json::from_value::<MouseButtonRequest>(request.payload)
                .map_err(|error| format!("invalid mouse button command payload: {error}"))
                .and_then(|payload| self.execute_button_request(ctx, payload)),
            MOUSE_SCROLL_COMMAND_NODE_TYPE => serde_json::from_value::<MouseScrollRequest>(request.payload)
                .map_err(|error| format!("invalid mouse scroll command payload: {error}"))
                .and_then(|payload| self.execute_scroll_request(ctx, payload)),
            _ => Ok(()),
        };

        if let Err(error) = result {
            logerror!(format!("Failed to handle mouse command {:?}: {error}", request.command_id));
        }
    }

    fn handle_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let result = match method {
            "moveMouse" => script_move_request(args)?
                .and_then(|request| self.execute_move_request(ctx, request)),
            "click" => script_button_request(method, args, MouseButtonAction::Click)?
                .and_then(|request| self.execute_button_request(ctx, request)),
            "pressButton" => script_button_request(method, args, MouseButtonAction::Press)?
                .and_then(|request| self.execute_button_request(ctx, request)),
            "releaseButton" => script_button_request(method, args, MouseButtonAction::Release)?
                .and_then(|request| self.execute_button_request(ctx, request)),
            "scroll" => script_scroll_request(args)?
                .and_then(|request| self.execute_scroll_request(ctx, request)),
            _ => return None,
        };

        Some(result)
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if self.device.is_bound() && self.device.id() == param {
            self.pending_input_events.clear();
            self.clear_input_values(ctx);
            if self.capture_os_input.get() && self.receive_enabled.get() && self.node_data().effective_enabled {
                self.restart_input_backend(ctx);
            }
            self.refresh_data_capabilities(ctx);
            self.refresh_selection_warning(ctx, self.selected_input_device().as_ref());
            self.refresh_connection_state(ctx);
        }

        if self.receive_enabled.is_bound() && self.receive_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.receive_enabled.get() {
                self.input_retry_elapsed = MOUSE_BACKEND_RETRY_INTERVAL_SECS;
            } else {
                self.stop_input_backend();
                self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
                self.clear_device_warning(ctx, MOUSE_SELECTION_WARNING_ID);
                self.clear_input_values(ctx);
            }
            self.refresh_connection_state(ctx);
        }

        if self.capture_os_input.is_bound() && self.capture_os_input.id() == param {
            self.pending_input_events.clear();
            self.clear_input_values(ctx);
            self.stop_input_backend();
            self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
            if self.node_data().effective_enabled && self.receive_enabled.get() {
                self.input_retry_elapsed = MOUSE_BACKEND_RETRY_INTERVAL_SECS;
            }
            self.refresh_connection_state(ctx);
        }

        if self.send_enabled.is_bound() && self.send_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.send_enabled.get() {
                self.output_retry_elapsed = MOUSE_BACKEND_RETRY_INTERVAL_SECS;
            } else {
                self.stop_output_backend();
                self.clear_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID);
            }
            self.refresh_connection_state(ctx);
        }
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                self.receive_enabled.get() && self.device.get_ref().as_str() != NO_MOUSE_VARIANT,
                self.send_enabled.get(),
            ),
        );
    }

    fn refresh_connection_state(&mut self, ctx: &mut ProcessCtx) {
        let selected_device = self.selected_input_device();
        let input_active = self.receive_enabled.get()
            && self.device.get_ref().as_str() != NO_MOUSE_VARIANT
            && self.input_backend.is_some()
            && selected_device.is_some();
        let output_active = self.send_enabled.get() && self.output_backend.is_some();

        self.set_input_active(ctx, input_active);
        self.set_output_active(ctx, output_active);
        self.set_connected_devices(ctx, self.known_devices.len());
        match selected_device {
            Some(device) => {
                self.set_selected_device_id(ctx, device.variant_id.as_str());
                self.set_selected_device_name(ctx, device.label.as_str());
            }
            None => {
                self.set_selected_device_id(ctx, "");
                self.set_selected_device_name(ctx, "");
            }
        }
        self.base.set_connected(ctx, input_active || output_active);
    }

    fn input_config(&self) -> MouseInputConfig {
        MouseInputConfig {
            capture_os_input: self.capture_os_input.get(),
            selection: self.device.get_ref().as_str().to_string(),
        }
    }

    fn clear_input_values(&mut self, ctx: &mut ProcessCtx) {
        self.set_position(ctx, 0, 0);
        self.set_delta(ctx, 0, 0);
        self.set_input_button(ctx, MouseButtonKind::Left, false);
        self.set_input_button(ctx, MouseButtonKind::Middle, false);
        self.set_input_button(ctx, MouseButtonKind::Right, false);
    }

    fn reset_values(&mut self, ctx: &mut ProcessCtx) {
        self.clear_input_values(ctx);
        self.set_input_active(ctx, false);
        self.set_output_active(ctx, false);
        self.set_connected_devices(ctx, 0);
        self.set_selected_device_id(ctx, "");
        self.set_selected_device_name(ctx, "");
        self.set_last_event(ctx, String::new());
    }

    fn set_input_active(&mut self, ctx: &mut ProcessCtx, value: bool) {
        if self.input_active.is_bound() && self.input_active.get() != value {
            self.input_active.set(ctx, value);
        }
    }

    fn set_output_active(&mut self, ctx: &mut ProcessCtx, value: bool) {
        if self.output_active.is_bound() && self.output_active.get() != value {
            self.output_active.set(ctx, value);
        }
    }

    fn set_last_event(&mut self, ctx: &mut ProcessCtx, value: String) {
        if self.last_event.is_bound() && self.last_event.get_ref().as_str() != value.as_str() {
            self.last_event.set(ctx, value);
        }
    }

    fn set_connected_devices(&mut self, ctx: &mut ProcessCtx, value: usize) {
        let value = clamp_usize_to_i32(value);
        if self.connected_devices.is_bound() && self.connected_devices.get() != value {
            self.connected_devices.set(ctx, value);
        }
    }

    fn set_selected_device_id(&mut self, ctx: &mut ProcessCtx, value: &str) {
        if self.device_id.is_bound() && self.device_id.get_ref().as_str() != value {
            self.device_id.set(ctx, value.to_string());
        }
    }

    fn set_selected_device_name(&mut self, ctx: &mut ProcessCtx, value: &str) {
        if self.device_name.is_bound() && self.device_name.get_ref().as_str() != value {
            self.device_name.set(ctx, value.to_string());
        }
    }

    fn set_position(&mut self, ctx: &mut ProcessCtx, x: i32, y: i32) {
        let x = f64::from(x);
        let y = f64::from(y);
        let value = Vec2::new(x, y);
        if self.position.is_bound()
            && (float_changed(self.position.get().x, x) || float_changed(self.position.get().y, y))
        {
            self.position.set(ctx, value);
        }
    }

    fn set_delta(&mut self, ctx: &mut ProcessCtx, x: i32, y: i32) {
        let x = f64::from(x);
        let y = f64::from(y);
        let value = Vec2::new(x, y);
        if self.delta.is_bound() && (float_changed(self.delta.get().x, x) || float_changed(self.delta.get().y, y)) {
            self.delta.set(ctx, value);
        }
    }

    fn set_input_button(&mut self, ctx: &mut ProcessCtx, button: MouseButtonKind, pressed: bool) {
        match button {
            MouseButtonKind::Left => {
                if self.left.is_bound() && self.left.get() != pressed {
                    self.left.set(ctx, pressed);
                }
            }
            MouseButtonKind::Middle => {
                if self.middle.is_bound() && self.middle.get() != pressed {
                    self.middle.set(ctx, pressed);
                }
            }
            MouseButtonKind::Right => {
                if self.right.is_bound() && self.right.get() != pressed {
                    self.right.set(ctx, pressed);
                }
            }
        }
    }

    fn set_backend_warning(&self, ctx: &mut ProcessCtx, warning_id: &str, message: &str) {
        NodeHandle::new(self.id()).set_warning_with(ctx, Some(warning_id), message, None);
    }

    fn clear_backend_warning(&self, ctx: &mut ProcessCtx, warning_id: &str) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some(warning_id));
    }

    fn sync_device_options(&self, ctx: &mut ProcessCtx) {
        if self.device.is_bound() {
            sync_mouse_device_enum_options(
                ctx,
                self.device.id(),
                mouse_device_options(self.known_devices.as_slice()),
            );
        }
    }

    fn selected_input_device(&self) -> Option<DiscoveredMouseDevice> {
        selected_mouse_device(self.device.get_ref().as_str(), self.known_devices.as_slice())
    }

    fn input_event_matches_selection(&self, device: &str) -> bool {
        match self.device.get_ref().as_str() {
            NO_MOUSE_VARIANT => false,
            AUTO_MOUSE_VARIANT => self
                .known_devices
                .first()
                .is_some_and(|selected| selected.variant_id == device),
            selection => selection == device,
        }
    }

    fn refresh_selection_warning(
        &self,
        ctx: &mut ProcessCtx,
        selected: Option<&DiscoveredMouseDevice>,
    ) {
        if !self.receive_enabled.get() {
            self.clear_device_warning(ctx, MOUSE_SELECTION_WARNING_ID);
            return;
        }

        let selection = self.device.get_ref();
        if mouse_specific_device_selected(selection.as_str()) && selected.is_none() {
            self.set_device_warning(
                ctx,
                MOUSE_SELECTION_WARNING_ID,
                format!(
                    "Selected mouse '{}' is not connected.",
                    human_mouse_variant(selection.as_str())
                )
                .as_str(),
            );
        } else {
            self.clear_device_warning(ctx, MOUSE_SELECTION_WARNING_ID);
        }
    }

    fn set_device_warning(&self, ctx: &mut ProcessCtx, warning_id: &str, message: &str) {
        if self.device.is_bound() {
            self.device.set_warning_with(ctx, Some(warning_id), message, None);
        }
    }

    fn clear_device_warning(&self, ctx: &mut ProcessCtx, warning_id: &str) {
        if self.device.is_bound() {
            self.device.clear_warning(ctx, Some(warning_id));
        }
    }

    fn emit_mouse_callback(
        &self,
        ctx: &mut ProcessCtx,
        callback: &str,
        args: Vec<serde_json::Value>,
    ) {
        crate::app::module::script_api::emit_script_callback(ctx, self.id(), callback, args);
    }

    fn mouse_state_arg(&self) -> serde_json::Value {
        let position = self.position.get();
        let delta = self.delta.get();
        let selected_device = self.selected_input_device();
        serde_json::json!({
            "position": { "x": position.x, "y": position.y },
            "delta": { "x": delta.x, "y": delta.y },
            "buttons": {
                "left": self.left.get(),
                "middle": self.middle.get(),
                "right": self.right.get(),
            },
            "device": {
                "selection": self.device.get_ref().as_str(),
                "id": selected_device.as_ref().map(|device| device.variant_id.clone()),
                "name": selected_device.as_ref().map(|device| device.label.clone()),
                "connectedDevices": self.known_devices.len(),
            },
            "inputActive": self.input_active.get(),
            "outputActive": self.output_active.get(),
        })
    }
}

#[golden_core::item(
    "module",
    node = "mouse_module",
    via = base,
    from_struct,
    menu_path = ["Controllers"]
)]
impl Node for MouseModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, MOUSE_MODULE_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
        self.refresh_connection_state(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.input_retry_elapsed += ctx.delta_time.as_secs_f64();
        self.output_retry_elapsed += ctx.delta_time.as_secs_f64();

        if !self.node_data().effective_enabled {
            return;
        }

        self.ensure_input_backend(ctx);
        self.ensure_output_backend(ctx);

        let events = self.drain_input_events(ctx);
        if self.devices_dirty {
            self.sync_device_options(ctx);
            self.devices_dirty = false;
        }
        let selected_device = self.selected_input_device();
        self.refresh_selection_warning(ctx, selected_device.as_ref());
        self.refresh_connection_state(ctx);
        self.handle_input_events(ctx, events);
        self.refresh_connection_state(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_input_backend();
        self.stop_output_backend();
        self.pending_input_events.clear();
        self.devices_dirty = false;
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(MOUSE_MODULE_UPDATE_RATE_HZ)
            .with_compiled_kernel("chataigne.runtime.mouse")
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            MOUSE_SCRIPT_METHODS,
        )
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.handle_script_method(ctx, method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &old_value);
        }
        self.on_param_change_inner(ctx, param);
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.input_retry_elapsed = MOUSE_BACKEND_RETRY_INTERVAL_SECS;
            self.output_retry_elapsed = MOUSE_BACKEND_RETRY_INTERVAL_SECS;
            self.refresh_data_capabilities(ctx);
            return;
        }

        self.stop_input_backend();
        self.stop_output_backend();
        self.pending_input_events.clear();
        self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
        self.clear_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID);
        self.clear_device_warning(ctx, MOUSE_SELECTION_WARNING_ID);
        self.base.set_connected(ctx, false);
        self.reset_values(ctx);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn float_changed(a: f64, b: f64) -> bool {
    (a - b).abs() > f64::EPSILON
}

fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn script_move_request(args: &[ParamValue]) -> Option<Result<MouseMoveRequest, String>> {
    let Some(x) = args.first().and_then(param_value_as_number) else {
        return Some(Err("method 'moveMouse' expects a numeric x argument".to_string()));
    };
    let Some(y) = args.get(1).and_then(param_value_as_number) else {
        return Some(Err("method 'moveMouse' expects a numeric y argument".to_string()));
    };

    let coordinate = match args.get(2).and_then(ParamValue::as_str) {
        Some(value) => match MouseMoveCoordinate::parse(&value) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        },
        None => MouseMoveCoordinate::Absolute,
    };
    let units = match args.get(3).and_then(ParamValue::as_str) {
        Some(value) => match MouseMoveUnits::parse(&value) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        },
        None => MouseMoveUnits::Pixels,
    };
    if coordinate == MouseMoveCoordinate::Relative && units == MouseMoveUnits::Normalized {
        return Some(Err(
            "method 'moveMouse' only supports normalized coordinates with absolute movement"
                .to_string(),
        ));
    }

    Some(Ok(MouseMoveRequest {
        x,
        y,
        coordinate,
        units,
        description: "move mouse".to_string(),
    }))
}

fn script_button_request(
    method: &str,
    args: &[ParamValue],
    action: MouseButtonAction,
) -> Option<Result<MouseButtonRequest, String>> {
    let button = match args.first().and_then(ParamValue::as_str) {
        Some(value) => match MouseButtonKind::parse(&value) {
            Ok(value) => value,
            Err(error) => return Some(Err(format!("method '{method}' {error}"))),
        },
        None => MouseButtonKind::Left,
    };

    Some(Ok(MouseButtonRequest {
        button,
        action,
        description: format!("{} {} mouse button", action.as_str(), button.as_str()),
    }))
}

fn script_scroll_request(args: &[ParamValue]) -> Option<Result<MouseScrollRequest, String>> {
    let Some(vertical) = args.first().and_then(param_value_as_i32) else {
        return Some(Err("method 'scroll' expects a numeric vertical argument".to_string()));
    };

    Some(Ok(MouseScrollRequest {
        vertical,
        horizontal: args.get(1).and_then(param_value_as_i32).unwrap_or(0),
        description: "scroll mouse".to_string(),
    }))
}

fn param_value_as_number(value: &ParamValue) -> Option<f64> {
    value.as_float().or_else(|| value.as_int().map(f64::from))
}

fn param_value_as_i32(value: &ParamValue) -> Option<i32> {
    let number = param_value_as_number(value)?;
    if !number.is_finite() {
        return None;
    }
    let rounded = number.round();
    (rounded >= f64::from(i32::MIN) && rounded <= f64::from(i32::MAX)).then_some(rounded as i32)
}

fn mouse_device_options(devices: &[DiscoveredMouseDevice]) -> Vec<ParameterEnumOption> {
    let mut options = vec![
        enum_option(AUTO_MOUSE_VARIANT, "Auto", 0),
        enum_option(NO_MOUSE_VARIANT, "No Input Device", 1),
    ];

    options.extend(devices.iter().enumerate().map(|(index, device)| {
        let mut option = enum_option(device.variant_id.as_str(), device.label.as_str(), 10 + index as i32);
        option.tags = vec![device.details.clone()];
        option
    }));
    options
}

fn sync_mouse_device_enum_options(
    ctx: &mut ProcessCtx,
    param_id: NodeId,
    options: Vec<ParameterEnumOption>,
) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("mouse device target is not a parameter".to_string());
        };

        let mut next_options = options.clone();
        let current_variant = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .map(|variant| canonical_mouse_variant(variant.as_str(), options.as_slice()))
            .unwrap_or_else(|| AUTO_MOUSE_VARIANT.to_string());

        if mouse_specific_device_selected(current_variant.as_str())
            && !next_options
                .iter()
                .any(|option| option.variant_id == current_variant)
        {
            next_options.insert(2, missing_mouse_option(current_variant.as_str()));
        }

        let next_value = if next_options
            .iter()
            .any(|option| option.variant_id == current_variant)
        {
            ParamValue::Enum(current_variant.clone())
        } else {
            ParamValue::Enum(AUTO_MOUSE_VARIANT.to_string())
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

fn selected_mouse_device(
    selection: &str,
    devices: &[DiscoveredMouseDevice],
) -> Option<DiscoveredMouseDevice> {
    devices
        .iter()
        .find(|device| selection_matches_mouse(selection, device))
        .cloned()
}

fn selection_matches_mouse(selection: &str, device: &DiscoveredMouseDevice) -> bool {
    match selection.trim() {
        AUTO_MOUSE_VARIANT => true,
        NO_MOUSE_VARIANT | "" => false,
        selected => {
            selected == device.variant_id
                || legacy_mouse_variant_identity(selected)
                    .is_some_and(|identity| identity == device.variant_id)
        }
    }
}

fn canonical_mouse_variant(
    selection: &str,
    options: &[ParameterEnumOption],
) -> String {
    let trimmed = selection.trim();
    if trimmed.is_empty() {
        return AUTO_MOUSE_VARIANT.to_string();
    }
    if options.iter().any(|option| option.variant_id == trimmed) {
        return trimmed.to_string();
    }
    if let Some(identity) = legacy_mouse_variant_identity(trimmed) {
        if options.iter().any(|option| option.variant_id == identity) {
            return identity.to_string();
        }
    }

    trimmed.to_string()
}

fn legacy_mouse_variant_identity(selection: &str) -> Option<&str> {
    let (identity, label) = selection.split_once('|')?;
    if identity.trim().is_empty() || label.trim().is_empty() {
        return None;
    }
    Some(identity)
}

fn mouse_specific_device_selected(selection: &str) -> bool {
    let trimmed = selection.trim();
    !trimmed.is_empty() && trimmed != AUTO_MOUSE_VARIANT && trimmed != NO_MOUSE_VARIANT
}

fn missing_mouse_option(variant_id: &str) -> ParameterEnumOption {
    let mut option = enum_option(
        variant_id,
        format!("Missing: {}", human_mouse_variant(variant_id)).as_str(),
        5,
    );
    option.tags = vec!["missing".to_string()];
    option
}

fn human_mouse_variant(variant_id: &str) -> String {
    if let Some((_, label)) = variant_id.rsplit_once('|') {
        if !label.trim().is_empty() {
            return label.to_string();
        }
    }
    variant_id
        .split('#')
        .nth(1)
        .or_else(|| variant_id.split('#').next())
        .map(str::to_string)
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

#[cfg(test)]
mod mouse_tests;
