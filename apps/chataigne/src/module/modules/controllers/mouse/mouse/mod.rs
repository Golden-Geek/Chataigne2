mod devices;
mod input;
mod mouse_runtime;
mod output;
mod projection;
use devices::*;
pub(crate) use output::dispatch as queue_mouse_command;

use golden_core::{
    engine::NodeExecutionRule,
    events::CustomEvent,
    node,
    node::{Node, NodeHandle, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, ParameterEnumOption, Vec2},
    process_ctx::ProcessCtx,
};

use crate::app::module::common::mouse::{
    MouseButtonKind, MOUSE_BUTTON_COMMAND_NODE_TYPE, MOUSE_MODULE_COMMAND_TYPES, MOUSE_MOVE_COMMAND_NODE_TYPE,
    MOUSE_SCROLL_COMMAND_NODE_TYPE,
};

use self::mouse_runtime::{DiscoveredMouseDevice, MouseInputConfig, MouseInputRuntime};

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
    input: input::Service,
    output: output::Service,
    suppress_input_backend: bool,
    suppress_output_backend: bool,
}

impl MouseModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            input::Service::new(),
            output::Service::new(),
            false,
            false,
        )
    }
    #[cfg(test)]
    pub(crate) fn disable_backends_for_test(&mut self) {
        self.suppress_input_backend = true;
        self.suppress_output_backend = true;
        self.stop_input_backend();
        self.stop_output_backend();
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        let settings_changed = [
            self.device.is_bound().then(|| self.device.id()),
            self.receive_enabled.is_bound().then(|| self.receive_enabled.id()),
            self.send_enabled.is_bound().then(|| self.send_enabled.id()),
            self.capture_os_input.is_bound().then(|| self.capture_os_input.id()),
        ]
        .into_iter()
        .any(|id| id == Some(param));
        // Read authored settings before our own warning/metadata edits can change them.
        let can_project = settings_changed && self.guard_output(ctx) && self.guard_output_settings(ctx);
        self.request_input_service();
        self.request_output_service();
        if self.device.is_bound() && self.device.id() == param {
            self.reconcile_input_configuration(ctx);
            if self.current_catalog().is_none() {
                self.clear_input_values(ctx);
            }
            self.refresh_data_capabilities(ctx);
            self.refresh_selection_warning(ctx, self.selected_input_device().as_ref());
        }

        if self.receive_enabled.is_bound() && self.receive_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.receive_enabled.get() {
                self.input.retry_at = None;
            } else {
                self.stop_input_backend();
                self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
                self.clear_backend_warning(ctx, "mouse_input_pressure");
                self.clear_device_warning(ctx, MOUSE_SELECTION_WARNING_ID);
                self.clear_input_values(ctx);
            }
        }

        if self.capture_os_input.is_bound() && self.capture_os_input.id() == param {
            self.reconcile_input_configuration(ctx);
            if self.current_catalog().is_none() {
                self.clear_input_values(ctx);
            }
            self.clear_backend_warning(ctx, MOUSE_INPUT_WARNING_ID);
            if self.node_data().effective_enabled && self.receive_enabled.get() {
                self.input.retry_at = None;
            }
        }

        if self.send_enabled.is_bound() && self.send_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.send_enabled.get() {
                self.request_output_service();
            } else {
                self.stop_output_backend();
                self.clear_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID);
            }
        }
        if can_project {
            self.project_connection_state(ctx);
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
    fn input_is_ready(&self) -> bool {
        self.receive_enabled.get()
            && self.device.get_ref().as_str() != NO_MOUSE_VARIANT
            && self.input.runtime.as_ref().is_some_and(MouseInputRuntime::is_running)
            && self.current_catalog().is_some_and(|catalog| catalog.selected.is_some())
    }
    fn refresh_input_state(&mut self, ctx: &mut ProcessCtx) {
        let selected = self.selected_input_device();
        self.set_input_active(ctx, self.input_is_ready());
        self.set_connected_devices(ctx, self.devices().len());
        self.set_selected_device_id(ctx, selected.as_ref().map_or("", |device| device.variant_id.as_str()));
        self.set_selected_device_name(ctx, selected.as_ref().map_or("", |device| device.label.as_str()));
    }
    fn project_connection_state(&mut self, ctx: &mut ProcessCtx) {
        let input_active = self.input_is_ready();
        let output_active = self.send_enabled.get() && self.output.is_ready();
        let mut guards = Vec::with_capacity(5);
        if input_active {
            guards.extend(self.input.catalog.as_ref().unwrap().guards.iter().cloned());
        }
        if output_active {
            guards.extend(self.output.readiness_guards().unwrap());
        }
        if !guards.is_empty() {
            ctx.edits.push(golden_core::edit::Edit::CheckPreconditions {
                preconditions: golden_core::edit::EditPreconditions::new(vec![], guards).unwrap(),
            });
        }
        self.refresh_input_state(ctx);
        self.set_output_active(ctx, output_active);
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
            sync_mouse_device_enum_options(ctx, self.device.id(), mouse_device_options(self.devices()));
        }
    }

    fn selected_input_device(&self) -> Option<DiscoveredMouseDevice> {
        self.current_catalog().and_then(|catalog| catalog.selected.clone())
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
        if let Err(error) = self.register_input(ctx) {
            self.set_backend_warning(ctx, MOUSE_INPUT_WARNING_ID, &error);
        }
        self.request_input_service();
        if let Err(error) = self.register_output(ctx) {
            self.set_backend_warning(ctx, MOUSE_OUTPUT_WARNING_ID, &error);
        }
        self.request_output_service();
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        let can_project = self.service_output_work(ctx);
        self.service_input(ctx);
        if can_project {
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
            self.request_input_service();
            self.input.retry_at = None;
            self.request_output_service();
            self.refresh_data_capabilities(ctx);
            return;
        }

        self.stop_input_backend();
        self.stop_output_backend();
        if let Some(wake) = &self.input.wake {
            wake.cancel_deadline();
        }
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
