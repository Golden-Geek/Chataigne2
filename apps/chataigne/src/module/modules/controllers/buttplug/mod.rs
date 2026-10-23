mod output;
mod publication;
mod service;
pub(crate) use output::dispatch as queue_buttplug_command;
mod transport;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor, NodeWarning},
    parameter::{Enum, ParamValue, ParameterEnumOption},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::buttplug::{
    BUTTPLUG_DEVICE_VARIANT_PREFIX, BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE, BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE,
    BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE, BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE,
    BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE, BUTTPLUG_TARGET_ALL, BUTTPLUG_TARGET_NONE,
};

use self::transport::{
    ButtplugConnectionStatus, ButtplugDeviceInfo, ButtplugTransportConfig, ButtplugTransportHandle, ButtplugWorkerEvent,
};

const BUTTPLUG_TARGET_WARNING_ID: &str = "buttplug_target_transport";
const BUTTPLUG_SELECTION_WARNING_ID: &str = "buttplug_selection";
const BUTTPLUG_SAFETY_WARNING_ID: &str = "buttplug_safety";
pub(crate) const BUTTPLUG_SAFETY_MANIFEST_URL: &str = "https://buttplug.io/docs/dev-guide/intro/buttplug-ethics/";

const BUTTPLUG_DEVICE_ADDED_CALLBACK: &str = "buttplugDeviceAdded";
const BUTTPLUG_DEVICE_REMOVED_CALLBACK: &str = "buttplugDeviceRemoved";
const BUTTPLUG_SCANNING_FINISHED_CALLBACK: &str = "buttplugScanningFinished";

const BUTTPLUG_SCRIPT_METHODS: &[&str] = &[
    "startScanning",
    "stopScanning",
    "stopAllDevices",
    "stopAll",
    "stopDevice",
    "setOutput",
    "vibrate",
    "rotate",
    "oscillate",
    "position",
    "positionWithDuration",
];

const BUTTPLUG_MODULE_COMMAND_TYPES: &[&str] = &[
    BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE,
    BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE,
    BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE,
    BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE,
    BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE,
];

#[node(
    "buttplug_module",
    label = "Buttplug",
    warnings = buttplug_safety_warnings(),
    show_child_warnings_max_depth = 4
)]
#[children(
    folder(connection) {
        remote_host: String = "127.0.0.1".to_string() (
            label = "Remote Host",
            description = "Buttplug server hostname or IP address. Intiface Central commonly listens on localhost."
        );
        remote_port: i32 = 12345 [0..65535] (
            label = "Remote Port",
            description = "Buttplug server WebSocket port.",
            widget = "text"
        );
        path: String = "/".to_string() (
            label = "Path",
            description = "WebSocket request path used by the Buttplug server."
        );
        secure: bool = false (
            label = "Secure",
            description = "Use wss:// instead of ws:// for the Buttplug WebSocket connection."
        );
        bypass_certificate_verification: bool = false (
            label = "Bypass Certificate Verification",
            description = "Allow self-signed certificates when Secure is enabled."
        );
        client_name: String = "Chataigne2".to_string() (
            label = "Client Name",
            description = "Name presented to the Buttplug server during handshake."
        );
        device: Enum = BUTTPLUG_TARGET_ALL (
            label = "Device",
            description = "Default target used by script calls and commands set to selected.",
            enum_options = ["all (All Devices)", "none (No Device)"]
        );
        auto_scan: bool = true (
            label = "Auto Scan",
            description = "Start Buttplug device scanning automatically after connecting."
        );
        [base_children];
    }
    folder(parameters) {
        max_output: f64 = 1.0 [0.0..1.0] (
            label = "Max Output",
            description = "Safety clamp applied to outgoing normalized output values."
        );
        [base_children];
    }
    folder(values) {
        folder(info, label = "Info") {
            server_name: String = String::new() (
                label = "Server Name",
                description = "Name reported by the connected Buttplug server.",
                read_only = true
            );
            connected_devices: i32 = 0 [0..2147483647] (
                label = "Connected Devices",
                description = "Number of Buttplug devices currently visible to the server.",
                read_only = true
            );
            scanning: bool = false (
                label = "Scanning",
                description = "Whether a Buttplug scan is currently active.",
                read_only = true
            );
            last_event: String = String::new() (
                label = "Last Event",
                description = "Last Buttplug connection or device event processed by this module.",
                read_only = true
            );
        }
        [base_children];
    }
)]
pub struct ButtplugModule {
    base: crate::app::ModuleBase,
    transport: Option<Box<ButtplugTransportHandle>>,
    last_transport_config: Option<Box<ButtplugTransportConfig>>,
    transport_dirty: bool,
    known_devices: Option<golden_io::RetainedPayload<transport::Publication>>,
    service: service::Service,
}

impl ButtplugModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            None,
            true,
            None,
            service::Service::new(),
        )
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn transport_config(&self) -> Result<ButtplugTransportConfig, String> {
        if self.remote_host.get_ref().len() > 1024
            || self.path.get_ref().len() > 4095
            || self.client_name.get_ref().len() > 256
        {
            return Err("Buttplug host/path/client name exceeds 1024 / 4095 / 256 bytes".into());
        }
        let remote_host = self.remote_host.get_ref().trim().to_string();
        if remote_host.is_empty() {
            return Err("Buttplug remote host cannot be empty".to_string());
        }

        let remote_port = u16::try_from(self.remote_port.get())
            .map_err(|_| "Buttplug remote port 'connection/remote_port' must be between 0 and 65535".to_string())?;
        let client_name = self.client_name.get_ref().trim();

        Ok(ButtplugTransportConfig {
            remote_host,
            remote_port,
            path: normalize_buttplug_path(self.path.get_ref().as_str()),
            secure: self.secure.get(),
            bypass_certificate_verification: self.bypass_certificate_verification.get(),
            client_name: if client_name.is_empty() {
                "Chataigne2".to_string()
            } else {
                client_name.to_string()
            },
            auto_scan: self.auto_scan.get(),
        })
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if self.param_affects_transport(param) {
            self.transport_dirty = true;
            self.request_service();
        }
        if self.device.is_bound() && self.device.id() == param {
            self.refresh_selection_warning(ctx);
            if let Some(transport) = &self.transport {
                if let Err(error) = transport.select(self.device.get_ref().as_str()) {
                    self.set_target_warning(ctx, &error);
                }
            }
        }
    }

    fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.remote_host.is_bound() && self.remote_host.id() == param)
            || (self.remote_port.is_bound() && self.remote_port.id() == param)
            || (self.path.is_bound() && self.path.id() == param)
            || (self.secure.is_bound() && self.secure.id() == param)
            || (self.bypass_certificate_verification.is_bound() && self.bypass_certificate_verification.id() == param)
            || (self.client_name.is_bound() && self.client_name.id() == param)
            || (self.auto_scan.is_bound() && self.auto_scan.id() == param)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
    }

    fn refresh_device_values(&mut self, ctx: &mut ProcessCtx) {
        self.set_int_param(
            ctx,
            ButtplugIntParam::ConnectedDevices,
            clamp_usize_to_i32(self.devices().len()),
        );
    }

    fn sync_device_options(&self, ctx: &mut ProcessCtx) {
        if self.device.is_bound() {
            sync_buttplug_device_enum_options(ctx, self.device.id(), buttplug_device_options(&[]));
        }
    }

    fn refresh_selection_warning(&self, ctx: &mut ProcessCtx) {
        let selection = self.device.get_ref();
        if selection.as_str().len() > 1024 {
            self.set_device_warning(
                ctx,
                BUTTPLUG_SELECTION_WARNING_ID,
                "Buttplug selection exceeds 1024 bytes",
            );
            return;
        }
        if specific_device_selected(selection.as_str())
            && !self
                .devices()
                .iter()
                .any(|device| device.variant_id == selection.as_str())
        {
            self.set_device_warning(
                ctx,
                BUTTPLUG_SELECTION_WARNING_ID,
                format!(
                    "Selected Buttplug device '{}' is not connected.",
                    human_device_variant(selection.as_str())
                )
                .as_str(),
            );
        } else {
            self.clear_device_warning(ctx, BUTTPLUG_SELECTION_WARNING_ID);
        }
    }

    fn set_target_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.remote_host.is_bound() {
            NodeHandle::new(self.remote_host.id()).set_warning_with(
                ctx,
                Some(BUTTPLUG_TARGET_WARNING_ID),
                message,
                None,
            );
        }
    }

    fn clear_target_warning(&self, ctx: &mut ProcessCtx) {
        if self.remote_host.is_bound() {
            NodeHandle::new(self.remote_host.id()).clear_warning(ctx, Some(BUTTPLUG_TARGET_WARNING_ID));
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

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            let queued = transport.queued_events();
            if queued != 0 {
                logerror!(format!(
                    "Buttplug retired {queued} native input records; publication was cancelled"
                ));
            }
            transport.stop();
        }
        self.known_devices = None;
        if !self.service.outgoing.is_empty() {
            logerror!(format!(
                "Buttplug retired {} captured requests; they were not sent",
                self.service.outgoing.len()
            ));
        }
        self.service.outgoing.clear();
        self.service.output_retry_at = None;
        self.service.retry_at = None;
        if let Some(wake) = &self.service.wake {
            wake.cancel_deadline();
        }
    }

    fn set_bool_param(&mut self, ctx: &mut ProcessCtx, param: ButtplugBoolParam, value: bool) {
        match param {
            ButtplugBoolParam::Scanning if self.scanning.is_bound() && self.scanning.get() != value => {
                self.scanning.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_int_param(&mut self, ctx: &mut ProcessCtx, param: ButtplugIntParam, value: i32) {
        match param {
            ButtplugIntParam::ConnectedDevices
                if self.connected_devices.is_bound() && self.connected_devices.get() != value =>
            {
                self.connected_devices.set(ctx, value);
            }
            _ => {}
        }
    }

    fn set_string_param(&mut self, ctx: &mut ProcessCtx, param: ButtplugStringParam, value: &str) {
        match param {
            ButtplugStringParam::ServerName if self.server_name.is_bound() && self.server_name.get_ref() != value => {
                self.server_name.set(ctx, value.to_string());
            }
            ButtplugStringParam::LastEvent if self.last_event.is_bound() && self.last_event.get_ref() != value => {
                self.last_event.set(ctx, value.to_string());
            }
            _ => {}
        }
    }

    fn set_last_event(&mut self, ctx: &mut ProcessCtx, value: String) {
        self.set_string_param(ctx, ButtplugStringParam::LastEvent, value.as_str());
    }
}

#[golden_core::item(
    "module",
    node = "buttplug_module",
    via = base,
    from_struct,
    menu_path = ["Controllers"]
)]
impl Node for ButtplugModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, BUTTPLUG_MODULE_COMMAND_TYPES);
        self.refresh_data_capabilities(ctx);
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if let Err(error) = self.register_service(ctx) {
            self.set_target_warning(ctx, &error);
        }
        self.request_service();
    }
    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn worker_wake_when_disabled(&self) -> bool {
        !self.service.pending.is_empty()
    }
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.buttplug")
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), BUTTPLUG_SCRIPT_METHODS)
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(method) = BUTTPLUG_SCRIPT_METHODS.iter().copied().find(|value| *value == method) {
            self.capture_script(ctx, method, args)?;
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

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if node != self.id() && patch.enabled.is_some() {
            self.transport_dirty = true;
            self.request_service();
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.transport_dirty = true;
            self.request_service();
        } else {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_target_warning(ctx);
            self.base.set_connected(ctx, false);
            self.set_bool_param(ctx, ButtplugBoolParam::Scanning, false);
            self.sync_device_options(ctx);
            self.refresh_device_values(ctx);
            self.transport_dirty = false;
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.accept_output(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[derive(Clone, Copy)]
enum ButtplugBoolParam {
    Scanning,
}

#[derive(Clone, Copy)]
enum ButtplugIntParam {
    ConnectedDevices,
}

#[derive(Clone, Copy)]
enum ButtplugStringParam {
    ServerName,
    LastEvent,
}

fn normalize_buttplug_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn buttplug_safety_warnings() -> Vec<NodeWarning> {
    vec![NodeWarning::new(
        "Safety warning: Buttplug can control intimate hardware. Keep a physical stop path, use explicit consent, and review the Buttplug safety manifest.",
    )
    .with_id(BUTTPLUG_SAFETY_WARNING_ID)
    .with_detail(format!(
        "Buttplug safety manifest: {BUTTPLUG_SAFETY_MANIFEST_URL}"
    ))]
}

fn buttplug_device_options(devices: &[ButtplugDeviceInfo]) -> Vec<ParameterEnumOption> {
    let mut options = vec![
        enum_option(BUTTPLUG_TARGET_ALL, "All Devices", 0),
        enum_option(BUTTPLUG_TARGET_NONE, "No Device", 1),
    ];

    options.extend(devices.iter().enumerate().map(|(index, device)| {
        let mut option = enum_option(
            device.variant_id.as_str(),
            device_label(device).as_str(),
            10 + index as i32,
        );
        option.tags = device.outputs.clone();
        option
    }));
    options
}

// Disabling an endpoint needs only the two fixed choices and one bounded missing selection.
fn sync_buttplug_device_enum_options(ctx: &mut ProcessCtx, param_id: NodeId, mut options: Vec<ParameterEnumOption>) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    let Some(node) = snapshot.node(param_id) else {
        return;
    };
    let Some(ParamValue::Enum(current)) = &node.param_value else {
        return;
    };
    if current.len() > 1024 {
        return;
    }
    if specific_device_selected(current) {
        options.push(missing_device_option(current));
    }
    ctx.edits.push(golden_core::edit::Edit::SetParamEnumOptions {
        node: param_id,
        expected_uuid: node.uuid,
        expected_value: ParamValue::Enum(current.clone()),
        options: options.into(),
    });
}

fn specific_device_selected(selection: &str) -> bool {
    let trimmed = selection.trim();
    !trimmed.is_empty() && trimmed != BUTTPLUG_TARGET_ALL && trimmed != BUTTPLUG_TARGET_NONE
}

fn missing_device_option(variant_id: &str) -> ParameterEnumOption {
    let mut option = enum_option(
        variant_id,
        format!("Missing: {}", human_device_variant(variant_id)).as_str(),
        5,
    );
    option.tags = vec!["missing".to_string()];
    option
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

fn human_device_variant(variant_id: &str) -> String {
    variant_id
        .strip_prefix(BUTTPLUG_DEVICE_VARIANT_PREFIX)
        .map(|index| format!("Device {index}"))
        .unwrap_or_else(|| variant_id.to_string())
}

fn device_label(device: &ButtplugDeviceInfo) -> String {
    device.display_name.clone().unwrap_or_else(|| device.name.clone())
}

fn device_arg(device: &ButtplugDeviceInfo) -> serde_json::Value {
    serde_json::json!({
        "id": device.variant_id,
        "index": device.index,
        "name": device.name,
        "displayName": device.display_name,
        "outputs": device.outputs,
    })
}

fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests;
