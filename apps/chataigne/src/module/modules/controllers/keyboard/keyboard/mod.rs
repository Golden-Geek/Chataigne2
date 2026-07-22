mod keyboard_runtime;

use std::collections::BTreeSet;

use golden_core::{
    engine::NodeExecutionRule,
    events::CustomEvent,
    logerror, node,
    node::{Node, NodeId, NodeScriptDescriptor, NodeHandle},
    parameter::{Enum, ParamValue, Parameter, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::ProcessCtx,
};

use crate::app::module::common::keyboard::{
    KeyboardKey, KeyboardKeyAction, KeyboardKeyRequest, KEYBOARD_KEY_COMMAND_NODE_TYPE,
    KEYBOARD_MODULE_COMMAND_TYPES,
};

use self::keyboard_runtime::{
    DiscoveredKeyboardDevice, KeyboardInputEvent, KeyboardInputRuntime,
    KeyboardOutputController, KeyboardRuntimeEvent,
};

const KEYBOARD_MODULE_UPDATE_RATE_HZ: u32 = 120;
const KEYBOARD_BACKEND_RETRY_INTERVAL_SECS: f64 = 2.0;
const KEYBOARD_INPUT_WARNING_ID: &str = "keyboard_input_backend";
const KEYBOARD_OUTPUT_WARNING_ID: &str = "keyboard_output_backend";
const KEYBOARD_SELECTION_WARNING_ID: &str = "keyboard_selection";

const AUTO_KEYBOARD_VARIANT: &str = "auto";
const NO_KEYBOARD_VARIANT: &str = "none";

const KEYBOARD_KEY_PRESSED_CALLBACK: &str = "keyboardKeyPressed";
const KEYBOARD_KEY_RELEASED_CALLBACK: &str = "keyboardKeyReleased";
const KEYBOARD_KEY_CHANGED_CALLBACK: &str = "keyboardKeyChanged";

const KEYBOARD_SCRIPT_METHODS: &[&str] = &["tapKey", "pressKey", "releaseKey"];

#[node("keyboard_module", label = "Keyboard")]
#[children(
    folder(connection) {
        device: Enum = AUTO_KEYBOARD_VARIANT (
            label = "Input Device",
            description = "Keyboard device to read. Auto uses the first connected keyboard. Output remains system-wide.",
            enum_options = ["auto (Auto)", "none (No Input Device)"]
        );
        receive_enabled: bool = true (
            label = "Receive Input",
            description = "Receive key presses from the selected local keyboard device."
        );
        send_enabled: bool = true (
            label = "Send Output",
            description = "Allow this module to press and release keys on the local system keyboard. Output is always system-wide."
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
                description = "Whether the selected keyboard input device is currently available.",
                read_only = true
            );
            output_active: bool = false (
                label = "Output Active",
                description = "Whether keyboard output injection is currently available.",
                read_only = true
            );
            connected_devices: i32 = 0 [0..2147483647] (
                label = "Connected Devices",
                description = "Number of keyboard devices currently visible to the input runtime.",
                read_only = true
            );
            device_id: String = String::new() (
                label = "Device ID",
                description = "Stable id for the selected keyboard device during this runtime.",
                read_only = true
            );
            device_name: String = String::new() (
                label = "Device Name",
                description = "Label for the selected keyboard device.",
                read_only = true
            );
            last_event: String = String::new() (
                label = "Last Event",
                description = "Last keyboard input or output event handled by this module.",
                read_only = true
            );
            last_key: String = String::new() (
                label = "Last Key",
                description = "Id of the last observed keyboard key event.",
                read_only = true
            );
            last_pressed: bool = false (
                label = "Last Pressed",
                description = "Whether the last observed keyboard key event was a press.",
                read_only = true
            );
            held_keys: String = String::new() (
                label = "Held Keys",
                description = "Comma-separated ids for the currently held supported keys.",
                read_only = true
            );
            held_key_count: i32 = 0 [0..2147483647] (
                label = "Held Key Count",
                description = "Number of currently held supported keys.",
                read_only = true
            );
        }
        folder(modifiers, label = "Modifiers") {
            left_shift: bool = false (label = "Left Shift", read_only = true);
            right_shift: bool = false (label = "Right Shift", read_only = true);
            left_control: bool = false (label = "Left Control", read_only = true);
            right_control: bool = false (label = "Right Control", read_only = true);
            left_alt: bool = false (label = "Left Alt", read_only = true);
            right_alt: bool = false (label = "Right Alt", read_only = true);
            left_meta: bool = false (label = "Left Meta", read_only = true);
            right_meta: bool = false (label = "Right Meta", read_only = true);
            caps_lock: bool = false (label = "Caps Lock", read_only = true);
        }
        [base_children];
    }
)]
pub struct KeyboardModule {
    base: crate::app::ModuleBase,
    input_backend: Option<KeyboardInputRuntime>,
    output_backend: Option<KeyboardOutputController>,
    pending_input_events: Vec<KeyboardInputEvent>,
    known_devices: Vec<DiscoveredKeyboardDevice>,
    devices_dirty: bool,
    input_retry_elapsed: f64,
    output_retry_elapsed: f64,
    last_input_error: Option<String>,
    last_output_error: Option<String>,
    suppress_input_backend: bool,
    suppress_output_backend: bool,
    pressed_keys: BTreeSet<KeyboardKey>,
}

impl KeyboardModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            None,
            Vec::new(),
            Vec::new(),
            true,
            KEYBOARD_BACKEND_RETRY_INTERVAL_SECS,
            KEYBOARD_BACKEND_RETRY_INTERVAL_SECS,
            None,
            None,
            false,
            false,
            BTreeSet::new(),
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
        self.pressed_keys.clear();
    }

    #[cfg(test)]
    pub(crate) fn enqueue_input_event_for_test(&mut self, event: KeyboardInputEvent) {
        self.pending_input_events.push(event);
    }

    fn ensure_input_backend(&mut self, ctx: &mut ProcessCtx) {
        if !self.receive_enabled.get() {
            self.stop_input_backend();
            self.clear_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID);
            return;
        }
        if self.suppress_input_backend || self.input_backend.is_some() {
            return;
        }
        if self.input_retry_elapsed < KEYBOARD_BACKEND_RETRY_INTERVAL_SECS {
            return;
        }

        let _ = self.try_start_input_backend(ctx);
    }

    fn ensure_output_backend(&mut self, ctx: &mut ProcessCtx) {
        if !self.send_enabled.get() {
            self.stop_output_backend();
            self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
            return;
        }
        if self.suppress_output_backend || self.output_backend.is_some() {
            return;
        }
        if self.output_retry_elapsed < KEYBOARD_BACKEND_RETRY_INTERVAL_SECS {
            return;
        }

        let _ = self.try_start_output_backend(ctx);
    }

    fn try_start_input_backend(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        if self.suppress_input_backend {
            return Err("keyboard input backend is suppressed for tests".to_string());
        }

        self.input_retry_elapsed = 0.0;
        match KeyboardInputRuntime::create() {
            Ok(backend) => {
                golden_core::log!(origin = self.id(); "Started keyboard input backend.");
                self.input_backend = Some(backend);
                self.last_input_error = None;
                self.clear_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID);
                self.refresh_connection_state(ctx);
                Ok(())
            }
            Err(error) => {
                if self.last_input_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Failed to start keyboard input backend: {error}"));
                }
                self.input_backend = None;
                self.last_input_error = Some(error.clone());
                self.set_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID, error.as_str());
                self.refresh_connection_state(ctx);
                Err(error)
            }
        }
    }

    fn try_start_output_backend(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        if self.suppress_output_backend {
            return Err("keyboard output backend is suppressed for tests".to_string());
        }

        self.output_retry_elapsed = 0.0;
        match KeyboardOutputController::create() {
            Ok(backend) => {
                golden_core::log!(origin = self.id(); "Started keyboard output backend.");
                self.output_backend = Some(backend);
                self.last_output_error = None;
                self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
                self.refresh_connection_state(ctx);
                Ok(())
            }
            Err(error) => {
                if self.last_output_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Failed to start keyboard output backend: {error}"));
                }
                self.output_backend = None;
                self.last_output_error = Some(error.clone());
                self.set_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID, error.as_str());
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

    fn drain_input_events(&mut self, ctx: &mut ProcessCtx) -> Vec<KeyboardInputEvent> {
        let mut events = std::mem::take(&mut self.pending_input_events);

        let Some(input_backend) = self.input_backend.as_mut() else {
            return events;
        };

        let runtime_events = match input_backend.poll_events() {
            Ok(runtime_events) => runtime_events,
            Err(error) => {
                if self.last_input_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Keyboard input runtime stopped: {error}"));
                }
                self.stop_input_backend();
                self.last_input_error = Some(error.clone());
                self.set_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID, error.as_str());
                self.clear_input_values(ctx);
                self.refresh_connection_state(ctx);
                return events;
            }
        };

        for runtime_event in runtime_events {
            match runtime_event {
                KeyboardRuntimeEvent::DevicesChanged(devices) => {
                    if self.known_devices != devices {
                        self.known_devices = devices;
                        self.devices_dirty = true;
                    }
                }
                KeyboardRuntimeEvent::Input { device, event } => {
                    if self.input_event_matches_selection(device.as_str()) {
                        events.push(event);
                    }
                }
            }
        }

        events
    }

    fn handle_input_events(&mut self, ctx: &mut ProcessCtx, events: Vec<KeyboardInputEvent>) {
        if events.is_empty() {
            return;
        }

        let device_label = self
            .selected_input_device()
            .map(|device| device.label)
            .unwrap_or_else(|| "Selected Keyboard".to_string());

        for event in events {
            match event {
                KeyboardInputEvent::KeyChanged { key, pressed } => {
                    if pressed {
                        self.pressed_keys.insert(key);
                    } else {
                        self.pressed_keys.remove(&key);
                    }
                    self.sync_pressed_key_values(ctx);
                    self.set_last_key(ctx, key.as_str());
                    self.set_last_pressed(ctx, pressed);
                    let action = if pressed { "pressed" } else { "released" };
                    self.set_last_event(ctx, format!("{device_label}: {} key {action}", key.label()));
                    if self.base.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Observed {device_label} {} key {action}", key.label().to_ascii_lowercase())
                        );
                    }
                    self.emit_keyboard_callback(
                        ctx,
                        KEYBOARD_KEY_CHANGED_CALLBACK,
                        vec![
                            serde_json::json!(key.as_str()),
                            serde_json::json!(pressed),
                            self.keyboard_state_arg(),
                        ],
                    );
                    self.emit_keyboard_callback(
                        ctx,
                        if pressed {
                            KEYBOARD_KEY_PRESSED_CALLBACK
                        } else {
                            KEYBOARD_KEY_RELEASED_CALLBACK
                        },
                        vec![serde_json::json!(key.as_str()), self.keyboard_state_arg()],
                    );
                }
            }
        }

        self.base.emit_incoming_traffic(ctx);
    }

    fn execute_key_request(&mut self, ctx: &mut ProcessCtx, request: KeyboardKeyRequest) -> Result<(), String> {
        let key = KeyboardKey::parse(request.key.as_str())?;
        let message = self.output_backend_mut(ctx)?.execute_key(key, request.action)?;
        self.finish_output_request(ctx, message);
        Ok(())
    }

    fn output_backend_mut(
        &mut self,
        ctx: &mut ProcessCtx,
    ) -> Result<&mut KeyboardOutputController, String> {
        if !self.node_data().effective_enabled {
            return Err("keyboard module is disabled".to_string());
        }
        if !self.send_enabled.get() {
            return Err("keyboard output is disabled".to_string());
        }
        if self.output_backend.is_none() {
            self.try_start_output_backend(ctx)?;
        }

        self.output_backend
            .as_mut()
            .ok_or_else(|| "keyboard output backend is unavailable".to_string())
    }

    fn finish_output_request(&mut self, ctx: &mut ProcessCtx, message: String) {
        self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
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
        if request.module_id != self.id() || !KEYBOARD_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str())
        {
            return;
        }

        let result = match request.command_type.as_str() {
            KEYBOARD_KEY_COMMAND_NODE_TYPE => serde_json::from_value::<KeyboardKeyRequest>(request.payload)
                .map_err(|error| format!("invalid keyboard key command payload: {error}"))
                .and_then(|payload| self.execute_key_request(ctx, payload)),
            _ => Ok(()),
        };

        if let Err(error) = result {
            logerror!(format!("Failed to handle keyboard command {:?}: {error}", request.command_id));
        }
    }

    fn handle_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let result = match method {
            "tapKey" => script_key_request(method, args, KeyboardKeyAction::Tap)?
                .and_then(|request| self.execute_key_request(ctx, request)),
            "pressKey" => script_key_request(method, args, KeyboardKeyAction::Press)?
                .and_then(|request| self.execute_key_request(ctx, request)),
            "releaseKey" => script_key_request(method, args, KeyboardKeyAction::Release)?
                .and_then(|request| self.execute_key_request(ctx, request)),
            _ => return None,
        };

        Some(result)
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if self.device.is_bound() && self.device.id() == param {
            self.pending_input_events.clear();
            self.clear_input_values(ctx);
            self.refresh_data_capabilities(ctx);
            self.refresh_selection_warning(ctx, self.selected_input_device().as_ref());
            self.refresh_connection_state(ctx);
        }

        if self.receive_enabled.is_bound() && self.receive_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.receive_enabled.get() {
                self.input_retry_elapsed = KEYBOARD_BACKEND_RETRY_INTERVAL_SECS;
            } else {
                self.stop_input_backend();
                self.clear_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID);
                self.clear_device_warning(ctx, KEYBOARD_SELECTION_WARNING_ID);
                self.clear_input_values(ctx);
            }
            self.refresh_connection_state(ctx);
        }

        if self.send_enabled.is_bound() && self.send_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.send_enabled.get() {
                self.output_retry_elapsed = KEYBOARD_BACKEND_RETRY_INTERVAL_SECS;
            } else {
                self.stop_output_backend();
                self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
            }
            self.refresh_connection_state(ctx);
        }
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                self.receive_enabled.get() && self.device.get_ref().as_str() != NO_KEYBOARD_VARIANT,
                self.send_enabled.get(),
            ),
        );
    }

    fn refresh_connection_state(&mut self, ctx: &mut ProcessCtx) {
        let selected_device = self.selected_input_device();
        let input_active = self.receive_enabled.get()
            && self.device.get_ref().as_str() != NO_KEYBOARD_VARIANT
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

    fn clear_input_values(&mut self, ctx: &mut ProcessCtx) {
        self.pressed_keys.clear();
        self.sync_pressed_key_values(ctx);
        self.set_last_key(ctx, "");
        self.set_last_pressed(ctx, false);
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

    fn sync_pressed_key_values(&mut self, ctx: &mut ProcessCtx) {
        let held_keys = self
            .pressed_keys
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        self.set_held_keys(ctx, held_keys.as_str());
        self.set_held_key_count(ctx, self.pressed_keys.len());

        let left_shift = self.pressed_keys.contains(&KeyboardKey::LeftShift);
        if self.left_shift.is_bound() && self.left_shift.get() != left_shift {
            self.left_shift.set(ctx, left_shift);
        }
        let right_shift = self.pressed_keys.contains(&KeyboardKey::RightShift);
        if self.right_shift.is_bound() && self.right_shift.get() != right_shift {
            self.right_shift.set(ctx, right_shift);
        }
        let left_control = self.pressed_keys.contains(&KeyboardKey::LeftControl);
        if self.left_control.is_bound() && self.left_control.get() != left_control {
            self.left_control.set(ctx, left_control);
        }
        let right_control = self.pressed_keys.contains(&KeyboardKey::RightControl);
        if self.right_control.is_bound() && self.right_control.get() != right_control {
            self.right_control.set(ctx, right_control);
        }
        let left_alt = self.pressed_keys.contains(&KeyboardKey::LeftAlt);
        if self.left_alt.is_bound() && self.left_alt.get() != left_alt {
            self.left_alt.set(ctx, left_alt);
        }
        let right_alt = self.pressed_keys.contains(&KeyboardKey::RightAlt);
        if self.right_alt.is_bound() && self.right_alt.get() != right_alt {
            self.right_alt.set(ctx, right_alt);
        }
        let left_meta = self.pressed_keys.contains(&KeyboardKey::LeftMeta);
        if self.left_meta.is_bound() && self.left_meta.get() != left_meta {
            self.left_meta.set(ctx, left_meta);
        }
        let right_meta = self.pressed_keys.contains(&KeyboardKey::RightMeta);
        if self.right_meta.is_bound() && self.right_meta.get() != right_meta {
            self.right_meta.set(ctx, right_meta);
        }
        let caps_lock = self.pressed_keys.contains(&KeyboardKey::CapsLock);
        if self.caps_lock.is_bound() && self.caps_lock.get() != caps_lock {
            self.caps_lock.set(ctx, caps_lock);
        }
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

    fn set_last_key(&mut self, ctx: &mut ProcessCtx, value: &str) {
        if self.last_key.is_bound() && self.last_key.get_ref().as_str() != value {
            self.last_key.set(ctx, value.to_string());
        }
    }

    fn set_last_pressed(&mut self, ctx: &mut ProcessCtx, value: bool) {
        if self.last_pressed.is_bound() && self.last_pressed.get() != value {
            self.last_pressed.set(ctx, value);
        }
    }

    fn set_held_keys(&mut self, ctx: &mut ProcessCtx, value: &str) {
        if self.held_keys.is_bound() && self.held_keys.get_ref().as_str() != value {
            self.held_keys.set(ctx, value.to_string());
        }
    }

    fn set_held_key_count(&mut self, ctx: &mut ProcessCtx, value: usize) {
        let value = clamp_usize_to_i32(value);
        if self.held_key_count.is_bound() && self.held_key_count.get() != value {
            self.held_key_count.set(ctx, value);
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

    fn set_backend_warning(&self, ctx: &mut ProcessCtx, warning_id: &str, message: &str) {
        NodeHandle::new(self.id()).set_warning_with(ctx, Some(warning_id), message, None);
    }

    fn clear_backend_warning(&self, ctx: &mut ProcessCtx, warning_id: &str) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some(warning_id));
    }

    fn sync_device_options(&self, ctx: &mut ProcessCtx) {
        if self.device.is_bound() {
            sync_keyboard_device_enum_options(
                ctx,
                self.device.id(),
                keyboard_device_options(self.known_devices.as_slice()),
            );
        }
    }

    fn selected_input_device(&self) -> Option<DiscoveredKeyboardDevice> {
        selected_keyboard_device(self.device.get_ref().as_str(), self.known_devices.as_slice())
    }

    fn input_event_matches_selection(&self, device: &str) -> bool {
        match self.device.get_ref().as_str() {
            NO_KEYBOARD_VARIANT => false,
            AUTO_KEYBOARD_VARIANT => self
                .known_devices
                .first()
                .is_some_and(|selected| selected.variant_id == device),
            selection => selection == device,
        }
    }

    fn refresh_selection_warning(
        &self,
        ctx: &mut ProcessCtx,
        selected: Option<&DiscoveredKeyboardDevice>,
    ) {
        if !self.receive_enabled.get() {
            self.clear_device_warning(ctx, KEYBOARD_SELECTION_WARNING_ID);
            return;
        }

        let selection = self.device.get_ref();
        if keyboard_specific_device_selected(selection.as_str()) && selected.is_none() {
            self.set_device_warning(
                ctx,
                KEYBOARD_SELECTION_WARNING_ID,
                format!(
                    "Selected keyboard '{}' is not connected.",
                    human_keyboard_variant(selection.as_str())
                )
                .as_str(),
            );
        } else {
            self.clear_device_warning(ctx, KEYBOARD_SELECTION_WARNING_ID);
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

    fn emit_keyboard_callback(
        &self,
        ctx: &mut ProcessCtx,
        callback: &str,
        args: Vec<serde_json::Value>,
    ) {
        crate::app::module::script_api::emit_script_callback(ctx, self.id(), callback, args);
    }

    fn keyboard_state_arg(&self) -> serde_json::Value {
        let selected_device = self.selected_input_device();
        serde_json::json!({
            "heldKeys": self.pressed_keys.iter().map(|key| key.as_str()).collect::<Vec<_>>(),
            "modifiers": {
                "leftShift": self.left_shift.get(),
                "rightShift": self.right_shift.get(),
                "leftControl": self.left_control.get(),
                "rightControl": self.right_control.get(),
                "leftAlt": self.left_alt.get(),
                "rightAlt": self.right_alt.get(),
                "leftMeta": self.left_meta.get(),
                "rightMeta": self.right_meta.get(),
                "capsLock": self.caps_lock.get(),
            },
            "device": {
                "selection": self.device.get_ref().as_str(),
                "id": selected_device.as_ref().map(|device| device.variant_id.clone()),
                "name": selected_device.as_ref().map(|device| device.label.clone()),
                "connectedDevices": self.known_devices.len(),
            },
            "lastKey": self.last_key.get_ref().as_str(),
            "lastPressed": self.last_pressed.get(),
            "inputActive": self.input_active.get(),
            "outputActive": self.output_active.get(),
        })
    }
}

#[golden_core::item(
    "module",
    node = "keyboard_module",
    via = base,
    from_struct,
    menu_path = ["Controllers"]
)]
impl Node for KeyboardModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base
            .configure_command_tester(ctx, KEYBOARD_MODULE_COMMAND_TYPES);
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
        self.pressed_keys.clear();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(KEYBOARD_MODULE_UPDATE_RATE_HZ)
            .with_compiled_kernel("chataigne.runtime.keyboard")
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            KEYBOARD_SCRIPT_METHODS,
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
            self.input_retry_elapsed = KEYBOARD_BACKEND_RETRY_INTERVAL_SECS;
            self.output_retry_elapsed = KEYBOARD_BACKEND_RETRY_INTERVAL_SECS;
            self.refresh_data_capabilities(ctx);
            return;
        }

        self.stop_input_backend();
        self.stop_output_backend();
        self.pending_input_events.clear();
        self.clear_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID);
        self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
        self.clear_device_warning(ctx, KEYBOARD_SELECTION_WARNING_ID);
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

fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn script_key_request(
    method: &str,
    args: &[ParamValue],
    action: KeyboardKeyAction,
) -> Option<Result<KeyboardKeyRequest, String>> {
    let Some(value) = args.first().and_then(ParamValue::as_str) else {
        return Some(Err(format!(
            "method '{method}' expects a keyboard key id string argument"
        )));
    };
    let key = match KeyboardKey::parse(&value) {
        Ok(key) => key,
        Err(error) => return Some(Err(format!("method '{method}' {error}"))),
    };

    Some(Ok(KeyboardKeyRequest {
        key: key.as_str().to_string(),
        action,
        description: format!("{} {} key", action.as_str(), key.as_str()),
    }))
}

fn keyboard_device_options(devices: &[DiscoveredKeyboardDevice]) -> Vec<ParameterEnumOption> {
    let mut options = vec![
        enum_option(AUTO_KEYBOARD_VARIANT, "Auto", 0),
        enum_option(NO_KEYBOARD_VARIANT, "No Input Device", 1),
    ];

    options.extend(devices.iter().enumerate().map(|(index, device)| {
        let mut option = enum_option(device.variant_id.as_str(), device.label.as_str(), 10 + index as i32);
        option.tags = vec![device.details.clone()];
        option
    }));
    options
}

fn sync_keyboard_device_enum_options(
    ctx: &mut ProcessCtx,
    param_id: NodeId,
    options: Vec<ParameterEnumOption>,
) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("keyboard device target is not a parameter".to_string());
        };

        let mut next_options = options.clone();
        let current_variant = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .map(|variant| canonical_keyboard_variant(variant.as_str(), options.as_slice()))
            .unwrap_or_else(|| AUTO_KEYBOARD_VARIANT.to_string());

        if keyboard_specific_device_selected(current_variant.as_str())
            && !next_options
                .iter()
                .any(|option| option.variant_id == current_variant)
        {
            next_options.insert(2, missing_keyboard_option(current_variant.as_str()));
        }

        let next_value = if next_options
            .iter()
            .any(|option| option.variant_id == current_variant)
        {
            ParamValue::Enum(current_variant.clone())
        } else {
            ParamValue::Enum(AUTO_KEYBOARD_VARIANT.to_string())
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

fn selected_keyboard_device(
    selection: &str,
    devices: &[DiscoveredKeyboardDevice],
) -> Option<DiscoveredKeyboardDevice> {
    devices
        .iter()
        .find(|device| selection_matches_keyboard(selection, device))
        .cloned()
}

fn selection_matches_keyboard(selection: &str, device: &DiscoveredKeyboardDevice) -> bool {
    match selection.trim() {
        AUTO_KEYBOARD_VARIANT => true,
        NO_KEYBOARD_VARIANT | "" => false,
        selected => {
            selected == device.variant_id
                || legacy_keyboard_variant_identity(selected)
                    .is_some_and(|identity| identity == device.variant_id)
        }
    }
}

fn canonical_keyboard_variant(selection: &str, options: &[ParameterEnumOption]) -> String {
    let trimmed = selection.trim();
    if trimmed.is_empty() {
        return AUTO_KEYBOARD_VARIANT.to_string();
    }
    if options.iter().any(|option| option.variant_id == trimmed) {
        return trimmed.to_string();
    }
    if let Some(identity) = legacy_keyboard_variant_identity(trimmed) {
        if options.iter().any(|option| option.variant_id == identity) {
            return identity.to_string();
        }
    }

    trimmed.to_string()
}

fn legacy_keyboard_variant_identity(selection: &str) -> Option<&str> {
    let (identity, label) = selection.split_once('|')?;
    if identity.trim().is_empty() || label.trim().is_empty() {
        return None;
    }
    Some(identity)
}

fn keyboard_specific_device_selected(selection: &str) -> bool {
    let trimmed = selection.trim();
    !trimmed.is_empty() && trimmed != AUTO_KEYBOARD_VARIANT && trimmed != NO_KEYBOARD_VARIANT
}

fn missing_keyboard_option(variant_id: &str) -> ParameterEnumOption {
    let mut option = enum_option(
        variant_id,
        format!("Missing: {}", human_keyboard_variant(variant_id)).as_str(),
        5,
    );
    option.tags = vec!["missing".to_string()];
    option
}

fn human_keyboard_variant(variant_id: &str) -> String {
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
mod tests;
