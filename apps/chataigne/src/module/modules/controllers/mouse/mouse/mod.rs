mod devices;
mod input;
mod mouse_runtime;
mod output;
use crate::app::module::common::worker_wake::ModuleWake;
use devices::*;
pub(crate) use output::dispatch as queue_mouse_command;
use std::time::{Duration, Instant};

use golden_core::{
    engine::NodeExecutionRule,
    events::CustomEvent,
    logerror, node,
    node::{Node, NodeHandle, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Parameter, ParameterEnumOption, ParameterEventBehaviour, Vec2},
    process_ctx::ProcessCtx,
};

use crate::app::module::common::mouse::{
    MouseButtonKind, MOUSE_BUTTON_COMMAND_NODE_TYPE, MOUSE_MODULE_COMMAND_TYPES, MOUSE_MOVE_COMMAND_NODE_TYPE,
    MOUSE_SCROLL_COMMAND_NODE_TYPE,
};

use self::mouse_runtime::{
    DiscoveredMouseDevice, MouseInputConfig, MouseInputEvent, MouseInputRuntime, MouseRuntimeEvent,
};

const MOUSE_BACKEND_RETRY: Duration = Duration::from_secs(2);
const MOUSE_INPUT_WARNING_ID: &str = "mouse_input_backend";
const MOUSE_OUTPUT_WARNING_ID: &str = "mouse_output_backend";
const MOUSE_SELECTION_WARNING_ID: &str = "mouse_selection";

const AUTO_MOUSE_VARIANT: &str = "auto";
const NO_MOUSE_VARIANT: &str = "none";

const MOUSE_MOVED_CALLBACK: &str = "mouseMoved";
const MOUSE_BUTTON_PRESSED_CALLBACK: &str = "mouseButtonPressed";
const MOUSE_BUTTON_RELEASED_CALLBACK: &str = "mouseButtonReleased";
const MOUSE_BUTTON_CHANGED_CALLBACK: &str = "mouseButtonChanged";

const MOUSE_SCRIPT_METHODS: &[&str] = &["moveMouse", "click", "pressButton", "releaseButton", "scroll"];

#[node("mouse_module", label = "Mouse")]
#[children(
    folder(connection) {
        device: Enum = AUTO_MOUSE_VARIANT (
            label = "Input Device",
            description = "Mouse device to read. Auto uses the first connected mouse. Output remains system-wide.",
            enum_options = mouse_device_options(&[])
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
    input_owner: golden_io::RetirementPool,
    output: output::Service,
    pending_input_events: Vec<MouseInputEvent>,
    known_devices: Vec<DiscoveredMouseDevice>,
    devices_dirty: bool,
    input_retry_at: Option<Instant>,
    wake: Option<ModuleWake>,
    delta_reset_at: Option<Instant>,
    last_input_error: Option<String>,
    suppress_input_backend: bool,
    suppress_output_backend: bool,
}

impl MouseModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            golden_io::RetirementPool::new(std::num::NonZeroUsize::MIN),
            output::Service::new(),
            Vec::new(),
            Vec::new(),
            true,
            None,
            None,
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
        self.stop_output_backend();
        self.pending_input_events.clear();
        self.known_devices.clear();
        self.devices_dirty = true;
    }

    #[cfg(test)]
    pub(crate) fn enqueue_input_event_for_test(&mut self, event: MouseInputEvent) {
        assert!(
            self.pending_input_events.len() < 16,
            "fixture input turn exceeds 16 records"
        );
        self.pending_input_events.push(event);
        self.request_service();
    }

    fn register_wake(&mut self, ctx: &ProcessCtx) -> Result<(), String> {
        let waker = match ctx.node_waker() {
            Ok(waker) => waker,
            Err(golden_core::engine::NodeWakeError::Unavailable) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&waker)) {
            self.stop_input_backend();
            self.wake = Some(ModuleWake::new(waker)?);
        }
        Ok(())
    }
    fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }
    fn schedule_service(&self) {
        let Some(wake) = &self.wake else {
            return;
        };
        if self.input_backend.as_ref().is_some_and(MouseInputRuntime::has_pending) {
            wake.notifier.notify();
        }
        let next = [self.input_retry_at, self.delta_reset_at].into_iter().flatten().min();
        if let Some(at) = next {
            wake.later(at.saturating_duration_since(Instant::now()));
        } else {
            wake.cancel_deadline();
        }
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
        if self.input_retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }

        let _ = self.try_start_input_backend(ctx);
    }

    fn try_start_input_backend(&mut self, ctx: &mut ProcessCtx) -> Result<(), String> {
        if self.suppress_input_backend {
            return Err("mouse input backend is suppressed for tests".to_string());
        }

        self.input_retry_at = Some(Instant::now() + MOUSE_BACKEND_RETRY);
        if self.device.get_ref().as_str().len() > mouse_runtime::MAX_DEVICE_TEXT {
            let error = "Mouse selection exceeds 1024 bytes".to_string();
            self.set_backend_warning(ctx, MOUSE_INPUT_WARNING_ID, &error);
            return Err(error);
        }
        let notifier = self
            .wake
            .as_ref()
            .ok_or("mouse worker wake is unavailable")?
            .notifier
            .clone();
        match MouseInputRuntime::create(&self.input_owner, notifier, self.input_config()) {
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
                self.input_retry_at = None;
                self.last_input_error = None;
                self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
                self.refresh_input_state(ctx);
                Ok(())
            }
            Err(error) => {
                if self.last_input_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Failed to start mouse input backend: {error}"));
                }
                self.input_backend = None;
                self.last_input_error = Some(error.clone());
                self.set_backend_warning(ctx, MOUSE_INPUT_WARNING_ID, error.as_str());
                self.refresh_input_state(ctx);
                Err(error)
            }
        }
    }

    fn stop_input_backend(&mut self) {
        self.input_backend = None;
        self.input_retry_at = None;
        if !self.known_devices.is_empty() {
            self.known_devices.clear();
            self.devices_dirty = true;
        }
    }

    fn restart_input_backend(&mut self, ctx: &mut ProcessCtx) {
        self.stop_input_backend();
        self.input_retry_at = None;
        let _ = self.try_start_input_backend(ctx);
    }

    fn drain_input_events(&mut self, ctx: &mut ProcessCtx) -> Vec<MouseInputEvent> {
        let mut events = std::mem::take(&mut self.pending_input_events);

        let Some(input_backend) = self.input_backend.as_mut() else {
            self.clear_backend_warning(ctx, "mouse_input_pressure");
            return events;
        };

        let runtime_events = match input_backend.poll_events() {
            Ok(runtime_events) => runtime_events,
            Err(error) => {
                if self.last_input_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Mouse input runtime stopped: {error}"));
                }
                self.stop_input_backend();
                self.input_retry_at = Some(Instant::now() + MOUSE_BACKEND_RETRY);
                self.clear_backend_warning(ctx, "mouse_input_pressure");
                self.clear_input_values(ctx);
                self.last_input_error = Some(error.clone());
                self.set_backend_warning(ctx, MOUSE_INPUT_WARNING_ID, error.as_str());
                self.refresh_input_state(ctx);
                return events;
            }
        };

        for runtime_event in runtime_events {
            if !runtime_event.current() {
                continue;
            }
            match &runtime_event.event {
                MouseRuntimeEvent::DevicesChanged(devices) => {
                    if self.known_devices != *devices {
                        self.known_devices = devices.clone();
                        self.devices_dirty = true;
                    }
                }
                MouseRuntimeEvent::Input { device, event } => {
                    if self.input_event_matches_selection(device.as_str()) {
                        events.push(*event);
                    }
                }
            }
        }

        if self.input_backend.as_ref().is_some_and(MouseInputRuntime::pressure) {
            self.set_backend_warning(
                ctx,
                "mouse_input_pressure",
                "Mouse input capacity is full; native service is backpressured",
            );
        } else {
            self.clear_backend_warning(ctx, "mouse_input_pressure");
        }
        events
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        self.request_service();
        self.request_output_service();
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
                self.input_retry_at = None;
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
                self.input_retry_at = None;
            }
            self.refresh_connection_state(ctx);
        }

        if self.send_enabled.is_bound() && self.send_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.send_enabled.get() {
                self.request_output_service();
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
        if self.guard_output(ctx) && self.guard_output_settings(ctx) {
            self.project_connection_state(ctx);
        }
    }
    fn refresh_input_state(&mut self, ctx: &mut ProcessCtx) {
        let selected = self.selected_input_device();
        self.set_input_active(
            ctx,
            self.receive_enabled.get()
                && self.device.get_ref().as_str() != NO_MOUSE_VARIANT
                && self.input_backend.as_ref().is_some_and(MouseInputRuntime::is_running)
                && selected.is_some(),
        );
        self.set_connected_devices(ctx, self.known_devices.len());
        self.set_selected_device_id(ctx, selected.as_ref().map_or("", |d| d.variant_id.as_str()));
        self.set_selected_device_name(ctx, selected.as_ref().map_or("", |d| d.label.as_str()));
    }
    fn project_connection_state(&mut self, ctx: &mut ProcessCtx) {
        let ready = self.output.is_ready();
        let output_active = self.send_enabled.get() && ready;
        if output_active {
            ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                preconditions: golden_core::edit::EditPreconditions::new(
                    vec![],
                    self.output.readiness_guards().unwrap().to_vec(),
                )
                .unwrap(),
            });
        }
        self.refresh_input_state(ctx);
        self.set_output_active(ctx, output_active);
        self.base.set_connected(ctx, self.input_active.get() || output_active);
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

    fn refresh_selection_warning(&self, ctx: &mut ProcessCtx, selected: Option<&DiscoveredMouseDevice>) {
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

    fn emit_mouse_callback(&self, ctx: &mut ProcessCtx, callback: &str, args: Vec<serde_json::Value>) {
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

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: golden_core::node::NodeCreationContext) {
        if let Err(error) = self.register_wake(ctx) {
            self.set_backend_warning(ctx, MOUSE_INPUT_WARNING_ID, &error);
        }
        self.request_service();
        if let Err(error) = self.register_output(ctx) {
            self.set_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID, &error);
        }
        self.request_output_service();
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        if let Err(error) = self.register_wake(ctx) {
            self.set_backend_warning(ctx, MOUSE_INPUT_WARNING_ID, &error);
            return;
        }
        let can_project = self.service_output_work(ctx);
        if self.node_data().effective_enabled {
            self.ensure_input_backend(ctx);
            let events = self.drain_input_events(ctx);
            if self.devices_dirty {
                self.sync_device_options(ctx);
                self.devices_dirty = false;
            }
            self.refresh_selection_warning(ctx, self.selected_input_device().as_ref());
            self.handle_input_events(ctx, events);
            if can_project {
                self.project_connection_state(ctx);
            }
            self.schedule_service();
        } else if can_project {
            self.project_connection_state(ctx);
        }
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn worker_wake_when_disabled(&self) -> bool {
        self.output.has_results()
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_input_backend();
        self.stop_output_backend();
        self.pending_input_events.clear();
        self.devices_dirty = false;
        self.wake = None;
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.mouse")
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), MOUSE_SCRIPT_METHODS)
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        let output_method = match method {
            "moveMouse" => Some("moveMouse"),
            "click" => Some("click"),
            "pressButton" => Some("pressButton"),
            "releaseButton" => Some("releaseButton"),
            "scroll" => Some("scroll"),
            _ => None,
        };
        if let Some(method) = output_method {
            self.capture_mouse_script(ctx, method, args)?;
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
            self.request_service();
            self.input_retry_at = None;
            self.request_output_service();
            self.refresh_data_capabilities(ctx);
            return;
        }

        self.stop_input_backend();
        self.stop_output_backend();
        self.delta_reset_at = None;
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
        self.pending_input_events.clear();
        self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
        self.clear_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID);
        self.clear_backend_warning(ctx, "mouse_input_pressure");
        self.clear_device_warning(ctx, MOUSE_SELECTION_WARNING_ID);
        self.base.set_connected(ctx, false);
        self.reset_values(ctx);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.accept_mouse_output(ctx, event);
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

#[cfg(test)]
mod tests;
