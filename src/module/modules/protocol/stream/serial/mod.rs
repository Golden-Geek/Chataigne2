mod transport;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeHandle, NodeId, NodeMetaPatch},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::streaming::{
    commands::StreamingSendRequest,
    module_helpers::{
        format_bytes_for_log, streaming_command_type_supported, streaming_parse_config, StreamingIncomingQueue,
    },
    parser::{StreamingParseConfig, StreamingParser},
};

use self::transport::{SerialStreamingTransportConfig, SerialStreamingTransportHandle, StreamingWorkerEvent};

const SERIAL_MODULE_UPDATE_RATE_HZ: u32 = 120;
const SERIAL_PORT_WARNING_ID: &str = "serial_port_transport";

#[node("serial_module", label = "Serial")]
#[children(
    folder(parameters, label = "Parameters", reuse = true) {
        auto_add: bool = true (
            label = "Auto Add",
            description = "Automatically create missing value nodes from incoming serial data."
        );
        folder(port, label = "Port") {
            port_name: String = String::new() (
                label = "Port",
                description = "Serial port name, such as COM3 on Windows or /dev/ttyUSB0 on Linux."
            );
            baud_rate: i32 = 115200 [1..2147483647] (
                label = "Baud Rate",
                description = "Serial baud rate.",
                widget = "text"
            );
        }
        folder(receiver, label = "Receiver", can_be_disabled = true) {
            parse_mode: Enum = "line" (
                label = "Parse Mode",
                description = "How incoming bytes are converted into values.",
                enum_options = ["line (default)", "raw"]
            );
            line_delimiter: String = "\\n".to_string() (
                label = "Line Delimiter",
                description = "Delimiter that terminates one incoming line. Escape sequences such as \\n, \\r, \\t, and \\xNN are supported."
            );
            value_separator: String = ",".to_string() (
                label = "Value Separator",
                description = "Separator used to split one line into values. Leave empty to split on whitespace."
            );
            first_element: Enum = "name" (
                label = "First Element",
                description = "Whether the first line element is a parameter name or a value.",
                enum_options = ["name (default)", "value"]
            );
            hierarchy_from_name: bool = true (
                label = "Name Hierarchy",
                description = "Split parameter names such as my.received.value into nested value folders."
            );
            hierarchy_delimiter: String = ".".to_string() (
                label = "Hierarchy Delimiter",
                description = "Delimiter used to split incoming parameter names into nested folders."
            );
        }
        folder(sender, label = "Sender", can_be_disabled = true) {}
    }
    node command_tester: crate::app::StreamingCommandTester = crate::app::StreamingCommandTester::create() (
        label = "Command Tester",
        description = "Create and trigger ad-hoc streaming commands through this module."
    );
)]
pub struct SerialModule {
    base: crate::app::ModuleBase,
    parser: StreamingParser,
    incoming: StreamingIncomingQueue,
    transport: Option<SerialStreamingTransportHandle>,
    last_transport_config: Option<SerialStreamingTransportConfig>,
    transport_dirty: bool,
}

impl SerialModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            StreamingParser::default(),
            StreamingIncomingQueue::new(),
            None,
            None,
            true,
        )
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;

        let config = match self.transport_config(snapshot) {
            Ok(Some(config)) => config,
            Ok(None) => {
                self.stop_transport();
                self.last_transport_config = None;
                self.clear_port_warning(ctx, snapshot);
                self.base.set_connected(ctx, false);
                return;
            }
            Err(error) => {
                logerror!("Invalid serial module configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_port_warning(ctx, snapshot, error.as_str());
                self.base.set_connected(ctx, false);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            self.clear_port_warning(ctx, snapshot);
            self.base.set_connected(ctx, true);
            return;
        }

        self.stop_transport();

        match SerialStreamingTransportHandle::spawn(config.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_port_warning(ctx, snapshot);
                self.base.set_connected(ctx, true);
            }
            Err(error) => {
                logerror!("Failed to start serial transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_port_warning(ctx, snapshot, error.as_str());
                self.base.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<Option<SerialStreamingTransportConfig>, String> {
        let receive_enabled = self.receiver_enabled(snapshot).unwrap_or(false);
        let send_enabled = self.sender_enabled(snapshot).unwrap_or(false);
        if !receive_enabled && !send_enabled {
            return Ok(None);
        }

        let port_name = self.port_name.get_ref().clone();
        if port_name.trim().is_empty() {
            return Err("serial port name cannot be empty".to_string());
        }

        let baud_rate = u32::try_from(self.baud_rate.get())
            .map_err(|_| "serial baud rate 'parameters/port/baud_rate' must be positive".to_string())?;

        Ok(Some(SerialStreamingTransportConfig {
            port_name,
            baud_rate,
            receive_enabled,
            send_enabled,
        }))
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        let mut worker_events = Vec::new();
        let Some(transport) = &self.transport else {
            return;
        };

        while let Ok(event) = transport.try_recv() {
            worker_events.push(event);
        }

        let parse_config = self.current_parse_config();
        let mut received_bytes = false;
        for event in worker_events {
            match event {
                StreamingWorkerEvent::Bytes(bytes) => match self.parser.push_bytes(bytes.as_slice(), &parse_config) {
                    Ok(messages) => {
                        received_bytes = true;
                        if self.base.log_incoming_enabled() {
                            golden_core::log!(
                                origin = self.id();
                                format!("Received serial {}", format_bytes_for_log(bytes.as_slice()))
                            );
                        }
                        self.incoming.push_messages(messages);
                    }
                    Err(error) => {
                        logerror!("Failed to parse serial input: {}", error);
                    }
                },
                StreamingWorkerEvent::Error(error) => {
                    logerror!("Serial transport error: {}", error);
                }
                StreamingWorkerEvent::Stopped(error) => {
                    logerror!("Serial transport stopped: {}", error);
                    self.transport_dirty = true;
                }
            }
        }

        if received_bytes {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn current_parse_config(&self) -> StreamingParseConfig {
        streaming_parse_config(
            self.parse_mode.get_ref().as_str(),
            self.line_delimiter.get_ref().as_str(),
            self.value_separator.get_ref().as_str(),
            self.first_element.get_ref().as_str(),
            self.hierarchy_from_name.get(),
            self.hierarchy_delimiter.get_ref().as_str(),
        )
    }

    fn queue_send_request(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        request: &StreamingSendRequest,
    ) -> Result<String, String> {
        if !self.sender_enabled(snapshot).unwrap_or(false) {
            return Err("serial sender is disabled".to_string());
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "serial transport is not available".to_string())?;
        transport.send(request.bytes.clone())?;
        self.base.emit_outgoing_traffic(ctx);

        if self.base.log_outgoing_enabled() {
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
        if self.incoming.take_ignored_param_change(param) {
            return;
        }

        if self.param_affects_transport(param) {
            self.transport_dirty = true;
        }
    }

    fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.port_name.is_bound() && self.port_name.id() == param)
            || (self.baud_rate.is_bound() && self.baud_rate.id() == param)
    }

    fn receiver_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "receiver")
    }

    fn receiver_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let receiver_id = self.receiver_node_id(snapshot)?;
        snapshot.node(receiver_id).map(|node| node.enabled)
    }

    fn sender_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "sender")
    }

    fn sender_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let sender_id = self.sender_node_id(snapshot)?;
        snapshot.node(sender_id).map(|node| node.enabled)
    }

    fn port_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "port")
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                self.receiver_enabled(snapshot).unwrap_or(false),
                self.sender_enabled(snapshot).unwrap_or(false),
            ),
        );
    }

    fn set_port_warning(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, message: &str) {
        let Some(port_id) = self.port_node_id(snapshot) else {
            return;
        };
        NodeHandle::new(port_id).set_warning_with(ctx, Some(SERIAL_PORT_WARNING_ID), message, None);
    }

    fn clear_port_warning(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(port_id) = self.port_node_id(snapshot) else {
            return;
        };
        NodeHandle::new(port_id).clear_warning(ctx, Some(SERIAL_PORT_WARNING_ID));
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
    via = base,
    from_struct,
    menu_path = ["Generic"]
)]
impl Node for SerialModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.transport_dirty = true;
        crate::app::module::enable_module_authoring(self.node_data_mut());

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);
        self.refresh_transport(ctx, snapshot);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.drain_transport_events(ctx);

        let needs_snapshot = self.transport_dirty || self.incoming.has_pending_messages();
        if !needs_snapshot {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);

        if self.transport_dirty {
            self.refresh_transport(ctx, snapshot);
        }

        self.incoming
            .process_pending(ctx, snapshot, self.base.values_id(), self.auto_add.get());
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
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

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if patch.enabled.is_some() && node != self.id() {
            self.transport_dirty = true;
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}
