mod transport;

#[cfg(test)]
mod transport_tests;

use std::collections::BTreeMap;

use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::streaming::{
        commands::StreamingSendRequest,
        module_helpers::{format_bytes_for_log, streaming_command_type_supported},
        websocket::normalize_websocket_path,
    },
    StreamingModuleBase,
};

use self::transport::{
    WebSocketServerTransportConfig, WebSocketServerTransportHandle, WebSocketServerWorkerEvent,
};

const WEBSOCKET_SERVER_MODULE_UPDATE_RATE_HZ: u32 = 120;
const WEBSOCKET_SERVER_PORT_WARNING_ID: &str = "websocket_server_port_transport";
const WEBSOCKET_SERVER_BIND_HOST: &str = "0.0.0.0";

#[node("websocket_server_module", label = "WebSocket Server")]
#[children(
    folder(connection) {
        port: i32 = 9002 [0..65535] (
            label = "Port",
            description = "WebSocket server port used to listen for incoming client connections.",
            widget = "text"
        );
        path: String = "/".to_string() (
            label = "Path",
            description = "Only accept WebSocket upgrade requests for this path. Leave empty to accept any path."
        );
        connected_clients: i32 = 0 [0..2147483647] (
            label = "Connected Clients",
            description = "Number of currently connected WebSocket clients.",
            read_only = true,
            widget = "text"
        );
        folder(clients, label = "Clients") {}
        [base_children];
    }
)]
pub struct WebSocketServerModule {
    stream: StreamingModuleBase,
    transport: Option<WebSocketServerTransportHandle>,
    last_transport_config: Option<WebSocketServerTransportConfig>,
    transport_dirty: bool,
    client_infos: BTreeMap<String, String>,
    client_list_dirty: bool,
}

impl WebSocketServerModule {
    pub fn create() -> Self {
        Self::new(
            StreamingModuleBase::create(),
            None,
            None,
            true,
            BTreeMap::new(),
            true,
        )
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;

        if !self.module_enabled(snapshot) {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_port_warning(ctx);
            self.clear_client_state(ctx, snapshot);
            self.stream.set_connected(ctx, false);
            return;
        }

        let config = match self.transport_config() {
            Ok(config) => config,
            Err(error) => {
                logerror!("Invalid WebSocket server configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_port_warning(ctx, error.as_str());
                self.clear_client_state(ctx, snapshot);
                self.stream.set_connected(ctx, false);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            self.clear_port_warning(ctx);
            self.stream.set_connected(ctx, true);
            return;
        }

        self.stop_transport();
        self.clear_client_state(ctx, snapshot);

        match WebSocketServerTransportHandle::spawn(config.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_port_warning(ctx);
                self.stream.set_connected(ctx, true);
            }
            Err(error) => {
                logerror!("Failed to start WebSocket server transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_port_warning(ctx, error.as_str());
                self.stream.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(&self) -> Result<WebSocketServerTransportConfig, String> {
        let bind_port = u16::try_from(self.port.get()).map_err(|_| {
            "WebSocket server port 'connection/port' must be between 0 and 65535".to_string()
        })?;

        let path = self.path.get_ref().trim();
        let path = if path.is_empty() {
            String::new()
        } else {
            normalize_websocket_path(path)
        };

        Ok(WebSocketServerTransportConfig {
            bind_host: WEBSOCKET_SERVER_BIND_HOST.to_string(),
            bind_port,
            path,
            receive_enabled: true,
            send_enabled: true,
        })
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let mut worker_events = Vec::new();
        let Some(transport) = &self.transport else {
            return;
        };

        while let Ok(event) = transport.try_recv() {
            worker_events.push(event);
        }

        let processing_enabled = self.stream.processing_enabled(snapshot).unwrap_or(true);
        let mut received_bytes = false;
        for event in worker_events {
            match event {
                WebSocketServerWorkerEvent::ClientConnected { client_id, info } => {
                    if self.stream.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Accepted WebSocket client {client_id} ({info})")
                        );
                    }
                    self.client_infos.insert(client_id, info);
                    self.client_list_dirty = true;
                }
                WebSocketServerWorkerEvent::ClientDisconnected { client_id, reason } => {
                    if let Some(reason) = reason {
                        logerror!(format!("WebSocket client {client_id} disconnected: {reason}"));
                    }
                    self.client_infos.remove(client_id.as_str());
                    self.client_list_dirty = true;
                }
                WebSocketServerWorkerEvent::Bytes { client_id, bytes } if !processing_enabled => {
                    if self.stream.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!(
                                "Received WebSocket from {} {} (processing disabled)",
                                client_id,
                                format_bytes_for_log(bytes.as_slice())
                            )
                        );
                    }
                }
                WebSocketServerWorkerEvent::Bytes { client_id, bytes } => {
                    match self.stream.parse_bytes(bytes.as_slice(), snapshot) {
                        Ok(messages) => {
                            received_bytes = true;
                            if self.stream.log_incoming_enabled() {
                                golden_core::log!(
                                    origin = self.id();
                                    format!(
                                        "Received WebSocket from {} {}",
                                        client_id,
                                        format_bytes_for_log(bytes.as_slice())
                                    )
                                );
                            }
                            self.stream.push_messages(messages);
                        }
                        Err(error) => {
                            logerror!("Failed to parse WebSocket input from {client_id}: {}", error);
                        }
                    }
                }
                WebSocketServerWorkerEvent::Error(error) => {
                    logerror!("WebSocket server transport error: {}", error);
                }
                WebSocketServerWorkerEvent::Stopped(error) => {
                    logerror!("WebSocket server transport stopped: {}", error);
                    self.transport_dirty = true;
                }
            }
        }

        if self.client_list_dirty {
            self.sync_client_nodes(ctx, snapshot);
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
        let connected_clients = self.client_infos.len();
        if connected_clients == 0 {
            return Err("no WebSocket clients are connected".to_string());
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "WebSocket server transport is not available".to_string())?;
        transport.send(request.frame_kind, request.bytes.clone())?;
        self.stream.emit_outgoing_traffic(ctx);

        if self.stream.log_outgoing_enabled() {
            golden_core::log!(
                origin = self.id();
                format!(
                    "Sent WebSocket {} to {} client(s)",
                    format_bytes_for_log(request.bytes.as_slice()),
                    connected_clients
                )
            );
        }

        Ok(format!(
            "Queued WebSocket {} for {} client(s)",
            request.description,
            connected_clients
        ))
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
            .map_err(|error| format!("invalid WebSocket server command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, snapshot, &payload))
        {
            logerror!(format!(
                "Failed to handle WebSocket server command {:?}: {error}",
                request.command_id
            ));
        }
    }

    fn on_param_change_inner(&mut self, param: NodeId) {
        if self.stream.take_ignored_param_change(param) {
            return;
        }

        if (self.port.is_bound() && self.port.id() == param)
            || (self.path.is_bound() && self.path.id() == param)
        {
            self.transport_dirty = true;
        }
    }

    fn clients_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let connection_id = self.stream.connection_id()?;
        snapshot.find_child_by_decl_id(connection_id, "clients")
    }

    fn sync_client_nodes(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.client_list_dirty = false;
        let connected_clients = i32::try_from(self.client_infos.len()).unwrap_or(i32::MAX);
        self.connected_clients.set(ctx, connected_clients);

        let Some(clients_id) = self.clients_node_id(snapshot) else {
            return;
        };

        for (client_id, info) in &self.client_infos {
            match snapshot.find_child(clients_id, client_id.as_str()) {
                Some(existing_id) => {
                    let Some(existing_snapshot) = snapshot.node(existing_id) else {
                        continue;
                    };

                    match existing_snapshot.param_value.as_ref() {
                        Some(ParamValue::Str(existing_info)) if existing_info == info => {}
                        Some(ParamValue::Str(_)) => {
                            ctx.set_param(existing_id, ParamValue::Str(info.clone()));
                        }
                        _ => {
                            ctx.replace_node_boxed(existing_id, Box::new(create_client_info_param(client_id, info)));
                        }
                    }
                }
                None => {
                    ctx.add_child_boxed(clients_id, Box::new(create_client_info_param(client_id, info)), None);
                }
            }
        }

        for child_id in snapshot.child_ids(clients_id) {
            let Some(child_snapshot) = snapshot.node(child_id) else {
                continue;
            };

            if !self.client_infos.contains_key(child_snapshot.label.as_str()) {
                ctx.edits.push(Edit::RemoveNode { node: child_id });
            }
        }
    }

    fn clear_client_state(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.client_infos.clear();
        self.client_list_dirty = true;
        self.sync_client_nodes(ctx, snapshot);
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, _snapshot: &ProcessTreeSnapshot) {
        self.stream
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
    }

    fn set_port_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        NodeHandle::new(self.port.id()).set_warning_with(ctx, Some(WEBSOCKET_SERVER_PORT_WARNING_ID), message, None);
    }

    fn clear_port_warning(&self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.port.id()).clear_warning(ctx, Some(WEBSOCKET_SERVER_PORT_WARNING_ID));
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }
}

#[golden_core::item(
    "module",
    node = "websocket_server_module",
    via = stream,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for WebSocketServerModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.stream.init(ctx);
        self.transport_dirty = true;
        self.client_list_dirty = true;

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);
        self.sync_client_nodes(ctx, snapshot);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.refresh_transport(ctx, snapshot);
        self.sync_client_nodes(ctx, snapshot);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.drain_transport_events(ctx, snapshot);

        let needs_snapshot = self.transport_dirty || self.stream.has_pending_messages() || self.client_list_dirty;
        if !needs_snapshot {
            return;
        }

        self.refresh_data_capabilities(ctx, snapshot);

        if self.transport_dirty {
            self.refresh_transport(ctx, snapshot);
        }

        if self.client_list_dirty {
            self.sync_client_nodes(ctx, snapshot);
        }

        self.stream.process_pending(ctx, snapshot);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(WEBSOCKET_SERVER_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        self.on_param_change_inner(param);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if let Some(enabled) = patch.enabled {
            if node == self.id() {
                if enabled {
                    self.transport_dirty = true;
                } else {
                    self.stop_transport();
                    self.last_transport_config = None;
                    if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
                        let snapshot = snapshot_arc.as_ref();
                        self.clear_port_warning(ctx);
                        self.clear_client_state(ctx, snapshot);
                    }
                    self.stream.set_connected(ctx, false);
                    self.transport_dirty = false;
                }
                return;
            }

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

fn create_client_info_param(client_id: &str, info: &str) -> Parameter {
    let mut parameter = Parameter::new(client_id, ParamValue::Str(info.to_string()), ParameterChangeCheck::ValueChange);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter.node_data_mut().meta.user_permissions = golden_core::node::NodeUserPermissions::none();
    parameter
}