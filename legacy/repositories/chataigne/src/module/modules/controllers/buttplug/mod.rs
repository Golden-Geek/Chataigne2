mod transport;

use std::sync::mpsc::TryRecvError;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor, NodeWarning},
    parameter::{Enum, ParamValue, Parameter, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::buttplug::{
    ButtplugControlRequest, ButtplugSetOutputRequest, ButtplugTargetRequest, BUTTPLUG_DEVICE_VARIANT_PREFIX,
    BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE, BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE,
    BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE, BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE,
    BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE, BUTTPLUG_TARGET_ALL, BUTTPLUG_TARGET_NONE,
    BUTTPLUG_TARGET_SELECTED,
};

use self::transport::{
    ButtplugConnectionStatus, ButtplugDeviceInfo, ButtplugTransportConfig, ButtplugTransportHandle,
    ButtplugWorkerCommand, ButtplugWorkerEvent,
};

const BUTTPLUG_MODULE_UPDATE_RATE_HZ: u32 = 120;
const BUTTPLUG_TARGET_WARNING_ID: &str = "buttplug_target_transport";
const BUTTPLUG_SELECTION_WARNING_ID: &str = "buttplug_selection";
const BUTTPLUG_SAFETY_WARNING_ID: &str = "buttplug_safety";
pub(crate) const BUTTPLUG_SAFETY_MANIFEST_URL: &str =
    "https://buttplug.io/docs/dev-guide/intro/buttplug-ethics/";

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
    known_devices: Vec<ButtplugDeviceInfo>,
}

impl ButtplugModule {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::new(), None, None, true, Vec::new())
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;

        if !self.module_enabled(snapshot) {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_target_warning(ctx);
            self.base.set_connected(ctx, false);
            self.set_bool_param(ctx, ButtplugBoolParam::Scanning, false);
            self.sync_device_options(ctx);
            self.refresh_device_values(ctx);
            return;
        }

        let config = match self.transport_config() {
            Ok(config) => config,
            Err(error) => {
                logerror!("Invalid Buttplug module configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
                self.sync_device_options(ctx);
                self.refresh_device_values(ctx);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_deref() == Some(&config) {
            return;
        }

        self.stop_transport();

        match ButtplugTransportHandle::spawn(config.clone()) {
            Ok(handle) => {
                self.transport = Some(Box::new(handle));
                self.last_transport_config = Some(Box::new(config));
                self.clear_target_warning(ctx);
                self.base.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!("Failed to start Buttplug transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
                self.sync_device_options(ctx);
                self.refresh_device_values(ctx);
            }
        }
    }

    fn transport_config(&self) -> Result<ButtplugTransportConfig, String> {
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

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        let (worker_events, worker_disconnected) = {
            let Some(transport) = &self.transport else {
                return;
            };

            let mut worker_events = Vec::new();
            let mut worker_disconnected = false;
            loop {
                match transport.try_recv() {
                    Ok(event) => worker_events.push(event),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        worker_disconnected = true;
                        break;
                    }
                }
            }

            (worker_events, worker_disconnected)
        };

        for event in worker_events {
            self.handle_worker_event(ctx, event);
        }

        if worker_disconnected {
            logerror!("Buttplug transport worker stopped unexpectedly; restarting.");
            self.stop_transport();
            self.last_transport_config = None;
            self.set_target_warning(ctx, "Buttplug transport worker stopped unexpectedly. Restarting.");
            self.base.set_connected(ctx, false);
            self.transport_dirty = true;
        }
    }

    fn handle_worker_event(&mut self, ctx: &mut ProcessCtx, event: ButtplugWorkerEvent) {
        match event {
            ButtplugWorkerEvent::Status(status) => self.handle_connection_status(ctx, status),
            ButtplugWorkerEvent::Devices(devices) => {
                self.known_devices = devices;
                self.sync_device_options(ctx);
                self.refresh_device_values(ctx);
                self.refresh_selection_warning(ctx);
            }
            ButtplugWorkerEvent::DeviceAdded(device) => {
                self.set_last_event(ctx, format!("Device added: {}", device_label(&device)));
                self.emit_buttplug_callback(ctx, BUTTPLUG_DEVICE_ADDED_CALLBACK, vec![device_arg(&device)]);
                if self.base.log_incoming_enabled() {
                    golden_core::log!(origin = self.id(); format!("Buttplug device added: {}", device_label(&device)));
                }
                self.base.emit_incoming_traffic(ctx);
            }
            ButtplugWorkerEvent::DeviceRemoved(device) => {
                self.set_last_event(ctx, format!("Device removed: {}", device_label(&device)));
                self.emit_buttplug_callback(ctx, BUTTPLUG_DEVICE_REMOVED_CALLBACK, vec![device_arg(&device)]);
                if self.base.log_incoming_enabled() {
                    golden_core::log!(origin = self.id(); format!("Buttplug device removed: {}", device_label(&device)));
                }
                self.base.emit_incoming_traffic(ctx);
            }
            ButtplugWorkerEvent::Scanning(scanning) => {
                self.set_bool_param(ctx, ButtplugBoolParam::Scanning, scanning);
            }
            ButtplugWorkerEvent::ScanningFinished => {
                self.set_last_event(ctx, "Scanning finished".to_string());
                self.emit_buttplug_callback(ctx, BUTTPLUG_SCANNING_FINISHED_CALLBACK, Vec::new());
                self.base.emit_incoming_traffic(ctx);
            }
            ButtplugWorkerEvent::CommandResult(message) => {
                if self.base.log_outgoing_enabled() {
                    golden_core::log!(origin = self.id(); format!("Buttplug {message}."));
                }
            }
            ButtplugWorkerEvent::Error(error) => {
                logerror!("Buttplug transport error: {}", error);
                self.set_target_warning(ctx, error.as_str());
            }
        }
    }

    fn handle_connection_status(&mut self, ctx: &mut ProcessCtx, status: ButtplugConnectionStatus) {
        match status {
            ButtplugConnectionStatus::Connected { server_name } => {
                golden_core::logsuccess!(origin = self.id(); format!("Connected Buttplug server '{server_name}'."));
                self.clear_target_warning(ctx);
                self.base.set_connected(ctx, true);
                self.set_string_param(ctx, ButtplugStringParam::ServerName, server_name.as_str());
                self.set_last_event(ctx, format!("Connected to {server_name}"));
            }
            ButtplugConnectionStatus::Recovering { message } => {
                logerror!("Buttplug transport recovering: {}", message);
                self.set_target_warning(ctx, message.as_str());
                self.base.set_connected(ctx, false);
                self.set_bool_param(ctx, ButtplugBoolParam::Scanning, false);
                self.known_devices.clear();
                self.sync_device_options(ctx);
                self.refresh_device_values(ctx);
                self.set_last_event(ctx, format!("Recovering: {message}"));
            }
        }
    }

    fn queue_runtime_command(
        &self,
        ctx: &mut ProcessCtx,
        command: ButtplugWorkerCommand,
        description: &str,
    ) -> Result<String, String> {
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "Buttplug transport is not available".to_string())?;
        transport.send(command)?;
        self.base.emit_outgoing_traffic(ctx);
        if self.base.log_outgoing_enabled() {
            golden_core::log!(origin = self.id(); format!("Queued Buttplug {description}."));
        }
        Ok(format!("Queued Buttplug {description}"))
    }

    fn queue_set_output(&self, ctx: &mut ProcessCtx, mut request: ButtplugSetOutputRequest) -> Result<String, String> {
        request.target = self.resolve_target_alias(request.target.as_str());
        request.value = request.value.clamp(0.0, self.max_output.get().clamp(0.0, 1.0));
        let description = request.description.clone();
        self.queue_runtime_command(ctx, ButtplugWorkerCommand::SetOutput(request), description.as_str())
    }

    fn queue_stop_device(&self, ctx: &mut ProcessCtx, request: ButtplugTargetRequest) -> Result<String, String> {
        self.queue_runtime_command(
            ctx,
            ButtplugWorkerCommand::StopDevice {
                target: self.resolve_target_alias(request.target.as_str()),
            },
            request.description.as_str(),
        )
    }

    fn on_custom_event_inner(&self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !BUTTPLUG_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str()) {
            return;
        }

        let result = match request.command_type.as_str() {
            BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE => serde_json::from_value::<ButtplugSetOutputRequest>(request.payload)
                .map_err(|error| format!("invalid Buttplug set-output command payload: {error}"))
                .and_then(|payload| self.queue_set_output(ctx, payload)),
            BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE => serde_json::from_value::<ButtplugTargetRequest>(request.payload)
                .map_err(|error| format!("invalid Buttplug stop-device command payload: {error}"))
                .and_then(|payload| self.queue_stop_device(ctx, payload)),
            BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE => {
                let description = serde_json::from_value::<ButtplugControlRequest>(request.payload)
                    .map(|payload| payload.description)
                    .unwrap_or_else(|_| "stop all devices".to_string());
                self.queue_runtime_command(ctx, ButtplugWorkerCommand::StopAllDevices, description.as_str())
            }
            BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE => {
                let description = serde_json::from_value::<ButtplugControlRequest>(request.payload)
                    .map(|payload| payload.description)
                    .unwrap_or_else(|_| "start scanning".to_string());
                self.queue_runtime_command(ctx, ButtplugWorkerCommand::StartScanning, description.as_str())
            }
            BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE => {
                let description = serde_json::from_value::<ButtplugControlRequest>(request.payload)
                    .map(|payload| payload.description)
                    .unwrap_or_else(|_| "stop scanning".to_string());
                self.queue_runtime_command(ctx, ButtplugWorkerCommand::StopScanning, description.as_str())
            }
            _ => Ok(String::new()),
        };

        if let Err(error) = result {
            logerror!(format!(
                "Failed to handle Buttplug command {:?}: {error}",
                request.command_id
            ));
        }
    }

    fn handle_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let result = match method {
            "startScanning" => self
                .queue_runtime_command(ctx, ButtplugWorkerCommand::StartScanning, "start scanning")
                .map(|_| ()),
            "stopScanning" => self
                .queue_runtime_command(ctx, ButtplugWorkerCommand::StopScanning, "stop scanning")
                .map(|_| ()),
            "stopAllDevices" | "stopAll" => self
                .queue_runtime_command(ctx, ButtplugWorkerCommand::StopAllDevices, "stop all devices")
                .map(|_| ()),
            "stopDevice" => self
                .queue_stop_device(
                    ctx,
                    ButtplugTargetRequest {
                        target: script_target_arg(args.get(0), BUTTPLUG_TARGET_SELECTED),
                        description: "stop device".to_string(),
                    },
                )
                .map(|_| ()),
            "setOutput" => script_set_output_request(args)?
                .and_then(|request| self.queue_set_output(ctx, request))
                .map(|_| ()),
            "vibrate" | "rotate" | "oscillate" | "position" => {
                script_named_output_request(method, args)?
                    .and_then(|request| self.queue_set_output(ctx, request))
                    .map(|_| ())
            }
            "positionWithDuration" => script_position_with_duration_request(args)?
                .and_then(|request| self.queue_set_output(ctx, request))
                .map(|_| ()),
            _ => return None,
        };

        Some(result)
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if self.param_affects_transport(param) {
            self.transport_dirty = true;
        }
        if self.device.is_bound() && self.device.id() == param {
            self.refresh_selection_warning(ctx);
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
            clamp_usize_to_i32(self.known_devices.len()),
        );
    }

    fn sync_device_options(&self, ctx: &mut ProcessCtx) {
        if self.device.is_bound() {
            sync_buttplug_device_enum_options(
                ctx,
                self.device.id(),
                buttplug_device_options(self.known_devices.as_slice()),
            );
        }
    }

    fn refresh_selection_warning(&self, ctx: &mut ProcessCtx) {
        let selection = self.device.get_ref();
        if specific_device_selected(selection.as_str())
            && !self
                .known_devices
                .iter()
                .any(|device| device.variant_id == selection.as_str())
        {
            self.set_device_warning(
                ctx,
                BUTTPLUG_SELECTION_WARNING_ID,
                format!("Selected Buttplug device '{}' is not connected.", human_device_variant(selection.as_str()))
                    .as_str(),
            );
        } else {
            self.clear_device_warning(ctx, BUTTPLUG_SELECTION_WARNING_ID);
        }
    }

    fn resolve_target_alias(&self, target: &str) -> String {
        let target = target.trim();
        if target.is_empty() || target.eq_ignore_ascii_case(BUTTPLUG_TARGET_SELECTED) {
            self.device.get_ref().as_str().to_string()
        } else {
            target.to_string()
        }
    }

    fn set_target_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.remote_host.is_bound() {
            NodeHandle::new(self.remote_host.id()).set_warning_with(ctx, Some(BUTTPLUG_TARGET_WARNING_ID), message, None);
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
            transport.stop();
        }
        self.known_devices.clear();
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
            ButtplugStringParam::ServerName
                if self.server_name.is_bound() && self.server_name.get_ref() != value =>
            {
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

    fn emit_buttplug_callback(
        &self,
        ctx: &mut ProcessCtx,
        callback: &str,
        args: Vec<serde_json::Value>,
    ) {
        crate::app::module::script_api::emit_script_callback(ctx, self.id(), callback, args);
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
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        self.refresh_transport(ctx, snapshot_arc.as_ref());
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.drain_transport_events(ctx);

        if !self.transport_dirty {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx);
        self.refresh_transport(ctx, snapshot);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.transport_dirty
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(BUTTPLUG_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            BUTTPLUG_SCRIPT_METHODS,
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

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if node != self.id() && patch.enabled.is_some() {
            self.transport_dirty = true;
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.transport_dirty = true;
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
        self.on_custom_event_inner(ctx, event);
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

fn script_set_output_request(args: &[ParamValue]) -> Option<Result<ButtplugSetOutputRequest, String>> {
    let Some(output) = args.first().and_then(ParamValue::as_str) else {
        return Some(Err("method 'setOutput' expects an output type string".to_string()));
    };
    let Some(value) = args.get(1).and_then(param_value_as_number) else {
        return Some(Err("method 'setOutput' expects a numeric value".to_string()));
    };

    Some(Ok(ButtplugSetOutputRequest {
        target: script_target_arg(args.get(2), BUTTPLUG_TARGET_SELECTED),
        output: output.clone(),
        value,
        duration_ms: script_duration_arg(args.get(3), 1000),
        description: format!("set {output}"),
    }))
}

fn script_named_output_request(method: &str, args: &[ParamValue]) -> Option<Result<ButtplugSetOutputRequest, String>> {
    let Some(value) = args.first().and_then(param_value_as_number) else {
        return Some(Err(format!("method '{method}' expects a numeric value")));
    };

    Some(Ok(ButtplugSetOutputRequest {
        target: script_target_arg(args.get(1), BUTTPLUG_TARGET_SELECTED),
        output: method.to_string(),
        value,
        duration_ms: script_duration_arg(args.get(2), 1000),
        description: method.to_string(),
    }))
}

fn script_position_with_duration_request(args: &[ParamValue]) -> Option<Result<ButtplugSetOutputRequest, String>> {
    let Some(value) = args.first().and_then(param_value_as_number) else {
        return Some(Err(
            "method 'positionWithDuration' expects a numeric value".to_string(),
        ));
    };

    Some(Ok(ButtplugSetOutputRequest {
        target: script_target_arg(args.get(2), BUTTPLUG_TARGET_SELECTED),
        output: crate::app::module::common::buttplug::BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION.to_string(),
        value,
        duration_ms: script_duration_arg(args.get(1), 1000),
        description: "position with duration".to_string(),
    }))
}

fn script_target_arg(value: Option<&ParamValue>, default: &str) -> String {
    value
        .and_then(ParamValue::as_str)
        .map(|target| target.trim().to_string())
        .filter(|target| !target.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn script_duration_arg(value: Option<&ParamValue>, default: u32) -> u32 {
    value
        .and_then(ParamValue::as_int)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn param_value_as_number(value: &ParamValue) -> Option<f64> {
    value.as_float().or_else(|| value.as_int().map(f64::from))
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
        let mut option = enum_option(device.variant_id.as_str(), device_label(device).as_str(), 10 + index as i32);
        option.tags = device.outputs.clone();
        option
    }));
    options
}

fn sync_buttplug_device_enum_options(ctx: &mut ProcessCtx, param_id: NodeId, options: Vec<ParameterEnumOption>) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("Buttplug device target is not a parameter".to_string());
        };

        let mut next_options = options.clone();
        let current_variant = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .unwrap_or_else(|| BUTTPLUG_TARGET_ALL.to_string());

        if specific_device_selected(current_variant.as_str())
            && !next_options
                .iter()
                .any(|option| option.variant_id == current_variant)
        {
            next_options.insert(2, missing_device_option(current_variant.as_str()));
        }

        let next_value = if next_options
            .iter()
            .any(|option| option.variant_id == current_variant)
        {
            ParamValue::Enum(current_variant.clone())
        } else {
            ParamValue::Enum(BUTTPLUG_TARGET_ALL.to_string())
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
