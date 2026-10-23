mod input;
mod keyboard_runtime;
mod output;
pub(crate) use output::dispatch as queue_keyboard_command;
mod projection;
mod selection;
mod values;
use golden_io::{RetainedPayload, RetirementPool};
use selection::*;
use std::num::NonZeroUsize;

use golden_core::{
    engine::NodeExecutionRule,
    events::CustomEvent,
    node,
    node::{Node, NodeHandle, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, ParameterEnumOption},
    process_ctx::ProcessCtx,
};

use crate::app::module::common::keyboard::KEYBOARD_MODULE_COMMAND_TYPES;

use self::keyboard_runtime::{DiscoveredKeyboardDevice, KeyboardInputRuntime};

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
    input: input::Service,
    output: output::Service,
    suppress_input_backend: bool,
    suppress_output_backend: bool,
}

impl KeyboardModule {
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
        ]
        .into_iter()
        .any(|id| id == Some(param));
        // Read authored settings before our own warning/metadata edits can change them.
        let can_project = settings_changed && self.guard_output(ctx) && self.guard_output_settings(ctx);
        if self.device.is_bound() && self.device.id() == param {
            if let Some(runtime) = self.input.runtime.as_mut() {
                if let Err(error) = runtime.configure(self.device.get_ref().as_str()) {
                    self.stop_input_backend();
                    self.set_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID, &error);
                }
            }
            self.request_input_service();
            if self.input.catalog.as_ref().is_none_or(|row| !row.current()) {
                self.clear_input_values(ctx);
            }
            self.refresh_data_capabilities(ctx);
            self.refresh_selection_warning(ctx, self.selected_input_device().as_ref());
        }

        if self.receive_enabled.is_bound() && self.receive_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.receive_enabled.get() {
                self.input.retry_at = None;
                self.request_input_service();
            } else {
                self.stop_input_backend();
                self.sync_device_options(ctx);
                self.clear_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID);
                self.clear_device_warning(ctx, KEYBOARD_SELECTION_WARNING_ID);
                self.clear_backend_warning(ctx, "keyboard_input_pressure");
                self.clear_input_values(ctx);
            }
        }

        if self.send_enabled.is_bound() && self.send_enabled.id() == param {
            self.refresh_data_capabilities(ctx);
            if self.send_enabled.get() {
                self.request_output_service();
            } else {
                self.stop_output_backend();
                self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
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
                self.receive_enabled.get() && self.device.get_ref().as_str() != NO_KEYBOARD_VARIANT,
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
            && self.device.get_ref().as_str() != NO_KEYBOARD_VARIANT
            && self
                .input
                .runtime
                .as_ref()
                .is_some_and(KeyboardInputRuntime::is_running)
            && self
                .input
                .catalog
                .as_ref()
                .is_some_and(|row| row.current() && row.prepared.catalog.selected.is_some())
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
        let output_ready = self.output.is_ready();
        let output_active = self.send_enabled.get() && output_ready;
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

    fn set_backend_warning(&self, ctx: &mut ProcessCtx, warning_id: &str, message: &str) {
        NodeHandle::new(self.id()).set_warning_with(ctx, Some(warning_id), message, None);
    }

    fn clear_backend_warning(&self, ctx: &mut ProcessCtx, warning_id: &str) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some(warning_id));
    }

    fn sync_device_options(&self, ctx: &mut ProcessCtx) {
        if self.device.is_bound() {
            sync_keyboard_device_enum_options(ctx, self.device.id(), keyboard_device_options(&[]));
        }
    }

    fn selected_input_device(&self) -> Option<DiscoveredKeyboardDevice> {
        selected_keyboard_device(self.device.get_ref().as_str(), self.devices())
    }

    fn refresh_selection_warning(&self, ctx: &mut ProcessCtx, selected: Option<&DiscoveredKeyboardDevice>) {
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
        self.base.configure_command_tester(ctx, KEYBOARD_MODULE_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
        self.refresh_connection_state(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: golden_core::node::NodeCreationContext) {
        if let Err(error) = self.register_output(ctx) {
            self.set_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID, &error);
        }
        if let Err(error) = self.register_input(ctx) {
            self.set_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID, &error);
        }
        self.request_output_service();
        self.request_input_service();
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

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.keyboard")
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), KEYBOARD_SCRIPT_METHODS)
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(method) = match method {
            "tapKey" => Some("tapKey"),
            "pressKey" => Some("pressKey"),
            "releaseKey" => Some("releaseKey"),
            _ => None,
        } {
            self.capture_keyboard_script(ctx, method, args)?;
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
        self.request_output_service();
        if enabled {
            self.input.retry_at = None;
            self.request_input_service();
            self.request_output_service();
            self.refresh_data_capabilities(ctx);
            return;
        }

        self.stop_input_backend();
        self.stop_output_backend();
        self.sync_device_options(ctx);
        self.clear_backend_warning(ctx, KEYBOARD_INPUT_WARNING_ID);
        self.clear_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID);
        self.clear_backend_warning(ctx, "keyboard_input_pressure");
        self.clear_device_warning(ctx, KEYBOARD_SELECTION_WARNING_ID);
        self.base.set_connected(ctx, false);
        self.reset_values(ctx);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.accept_keyboard_output(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[cfg(test)]
mod tests;
