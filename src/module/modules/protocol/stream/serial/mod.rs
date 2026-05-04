use std::sync::mpsc::TryRecvError;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::{
        serial::{
            serial_port_name_for_variant, serial_port_options, sync_serial_port_enum_options, SerialConnectionConfig,
            SerialConnectionEvent, SerialConnectionHandle, SerialConnectionStatus, SerialDiscoveryRegistration,
            SerialDiscoverySnapshot, NO_SERIAL_PORT_VARIANT,
        },
        streaming::{
            commands::StreamingSendRequest,
            module_helpers::{format_bytes_for_log, streaming_command_type_supported},
        },
    },
    StreamingModuleBase,
};

const SERIAL_MODULE_UPDATE_RATE_HZ: u32 = 120;
const SERIAL_PORT_OPTIONS_WARNING_ID: &str = "serial_port_options";
const SERIAL_PORT_CONNECTION_WARNING_ID: &str = "serial_port_connection";

#[node("serial_module", label = "Serial")]
#[children(
    folder(connection) {
        port_name: Enum = NO_SERIAL_PORT_VARIANT (
            label = "Port",
            description = "Serial port to connect to. Ports are detected automatically and labeled for this OS.",
            enum_options = ["none (No Port)"]
        );
        baud_rate: i32 = 115200 [1..2147483647] (
            label = "Baud Rate",
            description = "Serial baud rate.",
            widget = "text"
        );
        dtr: bool = false (
            label = "DTR",
            description = "Data Terminal Ready."
        );
        rts: bool = false (
            label = "RTS",
            description = "Request To Send."
        );
    }
    node command_tester: crate::app::ModuleCommandTester = crate::app::ModuleCommandTester::create(
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES,
    ) (
        label = "Command Tester",
        description = "Create and trigger ad-hoc streaming commands through this module."
    );
)]
pub struct SerialModule {
    stream: StreamingModuleBase,
    port_discovery: Option<SerialDiscoveryRegistration>,
    last_port_snapshot_version: u64,
    last_port_registration_error: Option<String>,
    transport: Option<SerialConnectionHandle>,
    last_transport_config: Option<SerialConnectionConfig>,
    transport_dirty: bool,
}

impl SerialModule {
    pub fn create() -> Self {
        Self::new(StreamingModuleBase::create(), None, 0, None, None, None, true)
    }

    fn ensure_port_discovery_registration(&mut self, ctx: &mut ProcessCtx) {
        if self.port_discovery.is_some() {
            return;
        }

        match SerialDiscoveryRegistration::register() {
            Ok(registration) => {
                self.port_discovery = Some(registration);
                self.last_port_snapshot_version = 0;
                self.last_port_registration_error = None;
                self.sync_port_state_from_manager(ctx, true);
            }
            Err(error) => {
                if self.last_port_registration_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Failed to start serial discovery: {}", error));
                }
                self.last_port_registration_error = Some(error.clone());
                self.set_port_warning(ctx, SERIAL_PORT_OPTIONS_WARNING_ID, error.as_str());
            }
        }
    }

    fn sync_port_state_from_manager(&mut self, ctx: &mut ProcessCtx, force: bool) {
        let Some(port_discovery) = &self.port_discovery else {
            return;
        };

        let snapshot_version = port_discovery.snapshot_version();
        if !force && snapshot_version == self.last_port_snapshot_version {
            return;
        }

        self.apply_port_discovery_snapshot(ctx, port_discovery.snapshot());
    }

    fn apply_port_discovery_snapshot(&mut self, ctx: &mut ProcessCtx, snapshot: SerialDiscoverySnapshot) {
        self.last_port_snapshot_version = snapshot.version;

        if !self.port_name.is_bound() {
            return;
        }

        sync_serial_port_enum_options(ctx, self.port_name.id(), serial_port_options(snapshot.ports.as_slice()));
        if let Some(error) = snapshot.error.as_deref() {
            self.set_port_warning(ctx, SERIAL_PORT_OPTIONS_WARNING_ID, error);
        } else {
            self.clear_port_warning(ctx, SERIAL_PORT_OPTIONS_WARNING_ID);
        }
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;

        if !self.module_enabled(snapshot) {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
            self.stream.set_connected(ctx, false);
            return;
        }

        let config = match self.transport_config(snapshot) {
            Ok(Some(config)) => config,
            Ok(None) => {
                self.stop_transport();
                self.last_transport_config = None;
                self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
                self.stream.set_connected(ctx, false);
                return;
            }
            Err(error) => {
                logerror!("Invalid serial module configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID, error.as_str());
                self.stream.set_connected(ctx, false);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            return;
        }

        self.stop_transport();

        match SerialConnectionHandle::spawn(config.clone()) {
            Ok(handle) => {
                golden_core::log!(
                    origin = self.id();
                    format!(
                        "Starting serial connection to {} @ {} baud.",
                        config.port_name,
                        config.baud_rate
                    )
                );
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
                self.stream.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!("Failed to start serial transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID, error.as_str());
                self.stream.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(&self, _snapshot: &ProcessTreeSnapshot) -> Result<Option<SerialConnectionConfig>, String> {
        let Some(port_name) = serial_port_name_for_variant(self.port_name.get_ref().as_str()) else {
            return Ok(None);
        };

        let baud_rate = u32::try_from(self.baud_rate.get())
            .map_err(|_| "serial baud rate 'connection/port/baud_rate' must be positive".to_string())?;

        Ok(Some(SerialConnectionConfig {
            port_name,
            baud_rate,
            dtr: self.dtr.get(),
            rts: self.rts.get(),
        }))
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
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

        let processing_enabled = self.stream.processing_enabled(snapshot).unwrap_or(true);
        let mut received_bytes = false;
        for event in worker_events {
            match event {
                SerialConnectionEvent::Bytes(bytes) if !processing_enabled => {
                    if self.stream.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Received serial {} (processing disabled)", format_bytes_for_log(bytes.as_slice()))
                        );
                    }
                }
                SerialConnectionEvent::Bytes(bytes) => match self.stream.parse_bytes(bytes.as_slice(), snapshot) {
                    Ok(messages) => {
                        received_bytes = true;
                        if self.stream.log_incoming_enabled() {
                            golden_core::log!(
                                origin = self.id();
                                format!("Received serial {}", format_bytes_for_log(bytes.as_slice()))
                            );
                        }
                        self.stream.push_messages(messages);
                    }
                    Err(error) => {
                        logerror!("Failed to parse serial input: {}", error);
                    }
                },
                SerialConnectionEvent::Warning(error) => {
                    logerror!("Serial transport warning: {}", error);
                    self.set_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID, error.as_str());
                }
                SerialConnectionEvent::Status(status) => match status {
                    SerialConnectionStatus::Connected { port_name } => {
                        if let Some(config) = self.last_transport_config.as_ref() {
                            golden_core::logsuccess!(
                                origin = self.id();
                                format!(
                                    "Connected serial port {} @ {} baud.",
                                    port_name,
                                    config.baud_rate
                                )
                            );
                        } else {
                            golden_core::logsuccess!(
                                origin = self.id();
                                format!("Connected serial port {}.", port_name)
                            );
                        }
                        self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
                        self.stream.set_connected(ctx, true);
                    }
                    SerialConnectionStatus::Recovering { message, .. } => {
                        logerror!("Serial transport recovering: {}", message);
                        self.set_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID, message.as_str());
                        self.stream.set_connected(ctx, false);
                    }
                },
            }
        }

        if worker_disconnected {
            logerror!("Serial transport worker stopped unexpectedly; restarting.");
            self.stop_transport();
            self.last_transport_config = None;
            self.set_port_warning(
                ctx,
                SERIAL_PORT_CONNECTION_WARNING_ID,
                "Serial transport worker stopped unexpectedly. Restarting.",
            );
            self.stream.set_connected(ctx, false);
            self.transport_dirty = true;
        }

        if received_bytes {
            self.stream.emit_incoming_traffic(ctx);
        }
    }

    fn queue_send_request(
        &self,
        ctx: &mut ProcessCtx,
        _snapshot: &ProcessTreeSnapshot,
        request: &StreamingSendRequest,
    ) -> Result<String, String> {
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "serial transport is not available".to_string())?;
        transport.send(request.bytes.clone())?;
        self.stream.emit_outgoing_traffic(ctx);

        if self.stream.log_outgoing_enabled() {
            golden_core::log!(
                origin = self.id();
                format!("Sent serial {}", format_bytes_for_log(request.bytes.as_slice()))
            );
        }

        Ok(format!("Queued serial {}", request.description))
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !streaming_command_type_supported(request.command_type.as_str()) {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if let Err(error) = serde_json::from_value::<StreamingSendRequest>(request.payload)
            .map_err(|error| format!("invalid serial command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, snapshot, &payload))
        {
            logerror!(format!(
                "Failed to handle serial command {:?}: {error}",
                request.command_id
            ));
        }
    }

    fn on_param_change_inner(&mut self, param: NodeId) {
        if self.stream.take_ignored_param_change(param) {
            return;
        }

        if self.param_affects_transport(param) {
            self.transport_dirty = true;
        }
    }

    fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.port_name.is_bound() && self.port_name.id() == param)
            || (self.baud_rate.is_bound() && self.baud_rate.id() == param)
            || (self.dtr.is_bound() && self.dtr.id() == param)
            || (self.rts.is_bound() && self.rts.id() == param)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.stream
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
    }

    fn set_port_warning(&self, ctx: &mut ProcessCtx, warning_id: &str, message: &str) {
        if !self.port_name.is_bound() {
            return;
        }
        self.port_name.set_warning_with(ctx, Some(warning_id), message, None);
    }

    fn clear_port_warning(&self, ctx: &mut ProcessCtx, warning_id: &str) {
        if !self.port_name.is_bound() {
            return;
        }
        self.port_name.clear_warning(ctx, Some(warning_id));
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }
}

#[golden_core::item(
    "module",
    node = "serial_module",
    via = stream,
    from_struct,
    menu_path = ["Generic"]
)]
impl Node for SerialModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.stream.init(ctx);
        self.transport_dirty = true;
        self.refresh_data_capabilities(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.ensure_port_discovery_registration(ctx);

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.refresh_transport(ctx, snapshot);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.drain_transport_events(ctx, snapshot);
        self.ensure_port_discovery_registration(ctx);
        self.sync_port_state_from_manager(ctx, false);

        let needs_snapshot = self.transport_dirty || self.stream.has_pending_messages();
        if !needs_snapshot {
            return;
        }

        self.refresh_data_capabilities(ctx);

        if self.transport_dirty {
            self.refresh_transport(ctx, snapshot);
        }

        self.stream.process_pending(ctx, snapshot);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.port_discovery = None;
        self.last_port_snapshot_version = 0;
        self.last_port_registration_error = None;
        self.stop_transport();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(SERIAL_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        self.on_param_change_inner(param);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if node != self.id() {
            return;
        }

        if let Some(enabled) = patch.enabled {
            if enabled {
                self.transport_dirty = true;
            } else {
                self.stop_transport();
                self.last_transport_config = None;
                self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
                self.stream.set_connected(ctx, false);
                self.transport_dirty = false;
            }
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[cfg(test)]
mod tests;
