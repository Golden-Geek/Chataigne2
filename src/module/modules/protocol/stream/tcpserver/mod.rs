mod transport;

use std::collections::BTreeMap;

use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::streaming::{
        commands::{streaming_script_send_request, StreamingSendRequest},
        module_helpers::{format_bytes_for_log, streaming_command_type_supported},
        script as streaming_script,
    },
    StreamingModuleBase,
};

use self::transport::{TcpServerTransportConfig, TcpServerTransportHandle, TcpServerWorkerEvent};

const TCP_SERVER_MODULE_UPDATE_RATE_HZ: u32 = 120;
const TCP_SERVER_PORT_WARNING_ID: &str = "tcp_server_port_transport";
const TCP_SERVER_BIND_HOST: &str = "0.0.0.0";

#[node("tcp_server_module", label = "TCP Server")]
#[children(
    folder(connection) {
        port: i32 = 9002 [0..65535] (
            label = "Port",
            description = "TCP port used to listen for incoming client connections.",
            widget = "text"
        );
        connected_clients: i32 = 0 [0..2147483647] (
            label = "Connected Clients",
            description = "Number of currently connected TCP clients.",
            read_only = true,
            widget = "text"
        );
        folder(clients, label = "Clients") {}
        [base_children];
    }
)]
pub struct TcpServerModule {
    stream: StreamingModuleBase,
    transport: Option<TcpServerTransportHandle>,
    last_transport_config: Option<TcpServerTransportConfig>,
    transport_dirty: bool,
    client_infos: BTreeMap<String, String>,
    client_list_dirty: bool,
}

impl TcpServerModule {
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
                logerror!("Invalid TCP server module configuration: {}", error);
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

        match TcpServerTransportHandle::spawn(config.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_port_warning(ctx);
                self.stream.set_connected(ctx, true);
            }
            Err(error) => {
                logerror!("Failed to start TCP server transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_port_warning(ctx, error.as_str());
                self.stream.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(&self) -> Result<TcpServerTransportConfig, String> {
        let bind_port = u16::try_from(self.port.get())
            .map_err(|_| "TCP server port 'connection/port' must be between 0 and 65535".to_string())?;

        Ok(TcpServerTransportConfig {
            bind_host: TCP_SERVER_BIND_HOST.to_string(),
            bind_port,
            receive_enabled: true,
            send_enabled: true,
        })
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        let mut worker_events = Vec::new();
        let Some(transport) = &self.transport else {
            return;
        };

        transport.clear_pending();
        while let Ok(event) = transport.try_recv() {
            worker_events.push(event);
        }

        let processing_enabled = self.stream.processing_enabled_cached();
        let mut received_bytes = false;
        for event in worker_events {
            match event {
                TcpServerWorkerEvent::ClientConnected { client_id, info } => {
                    streaming_script::emit_client_connected(ctx, self.id(), client_id.as_str(), info.clone());
                    if self.stream.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Accepted TCP client {client_id} ({info})")
                        );
                    }
                    self.client_infos.insert(client_id, info);
                    self.client_list_dirty = true;
                }
                TcpServerWorkerEvent::ClientDisconnected { client_id, reason } => {
                    streaming_script::emit_client_disconnected(ctx, self.id(), client_id.as_str(), reason.as_deref());
                    if let Some(reason) = reason {
                        logerror!(format!("TCP client {client_id} disconnected: {reason}"));
                    }
                    self.client_infos.remove(client_id.as_str());
                    self.client_list_dirty = true;
                }
                TcpServerWorkerEvent::Bytes { client_id, bytes } if !processing_enabled => {
                    streaming_script::emit_stream_bytes_callbacks(
                        ctx,
                        self.id(),
                        bytes.as_slice(),
                        Some(client_id.as_str()),
                        true,
                    );
                    if self.stream.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!(
                                "Received TCP from {} {} (processing disabled)",
                                client_id,
                                format_bytes_for_log(bytes.as_slice())
                            )
                        );
                    }
                }
                TcpServerWorkerEvent::Bytes { client_id, bytes } => {
                    streaming_script::emit_stream_bytes_callbacks(
                        ctx,
                        self.id(),
                        bytes.as_slice(),
                        Some(client_id.as_str()),
                        true,
                    );
                    match self.stream.parse_bytes_cached(bytes.as_slice()) {
                        Ok(messages) => {
                            received_bytes = true;
                            if self.stream.log_incoming_enabled() {
                                golden_core::log!(
                                    origin = self.id();
                                    format!(
                                        "Received TCP from {} {}",
                                        client_id,
                                        format_bytes_for_log(bytes.as_slice())
                                    )
                                );
                            }
                            self.stream.push_messages(messages);
                        }
                        Err(error) => {
                            logerror!("Failed to parse TCP input from {client_id}: {}", error);
                        }
                    }
                }
                TcpServerWorkerEvent::Error(error) => {
                    logerror!("TCP server transport error: {}", error);
                }
                TcpServerWorkerEvent::Stopped(error) => {
                    logerror!("TCP server transport stopped: {}", error);
                    self.transport_dirty = true;
                }
            }
        }

        // client_list_dirty sync is deferred to update() where snapshot is available.

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
            return Err("no TCP clients are connected".to_string());
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "TCP server transport is not available".to_string())?;
        transport.send(request.bytes.clone())?;
        self.stream.emit_outgoing_traffic(ctx);

        if self.stream.log_outgoing_enabled() {
            golden_core::log!(
                origin = self.id();
                format!(
                    "Sent TCP {} to {} client(s)",
                    format_bytes_for_log(request.bytes.as_slice()),
                    connected_clients
                )
            );
        }

        Ok(format!(
            "Queued TCP {} for {} client(s)",
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
            .map_err(|error| format!("invalid TCP server command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, snapshot, &payload))
        {
            logerror!(format!(
                "Failed to handle TCP server command {:?}: {error}",
                request.command_id
            ));
        }
    }

    fn on_param_change_inner(&mut self, param: NodeId) {
        if self.stream.take_ignored_param_change(param) {
            return;
        }

        if self.port.is_bound() && self.port.id() == param {
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
        NodeHandle::new(self.port.id()).set_warning_with(ctx, Some(TCP_SERVER_PORT_WARNING_ID), message, None);
    }

    fn clear_port_warning(&self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.port.id()).clear_warning(ctx, Some(TCP_SERVER_PORT_WARNING_ID));
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }
}

#[golden_core::item(
    "module",
    node = "tcp_server_module",
    via = stream,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for TcpServerModule {
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
        // Drain transport bytes using only cached config — no snapshot needed.
        self.drain_transport_events(ctx);

        let needs_snapshot =
            self.transport_dirty || self.stream.has_pending_messages() || self.client_list_dirty;
        if !needs_snapshot {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        // Refresh cached config while we have the snapshot so the next drain is accurate.
        self.stream.refresh_config_cache(snapshot);
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

    fn needs_update(&self) -> bool {
        self.transport_dirty
            || self.stream.has_pending_messages()
            || self.client_list_dirty
            || self.transport.as_ref().is_some_and(|t| t.has_pending())
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.transport_dirty
            || self.stream.has_pending_messages()
            || self.client_list_dirty
            || self.transport.as_ref().is_some_and(|t| t.has_pending())
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(TCP_SERVER_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        streaming_script::descriptor_for_node(self.node_data(), self.get_type())
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(request) = streaming_script_send_request(method, args) {
            let request = request?;
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return Err(format!("method '{method}' is unavailable without a tree snapshot"));
            };
            self.queue_send_request(ctx, snapshot_arc.as_ref(), &request)?;
            return Ok(true);
        }

        self.stream.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.stream
                .emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &old_value);
        }
        self.on_param_change_inner(param);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if let Some(enabled) = patch.enabled {
            if node != self.id() {
                let _ = ctx;
                let _ = enabled;
                self.transport_dirty = true;
            }
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
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
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn create_client_info_param(label: &str, info: &str) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Str(info.to_string()), ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    parameter.node_data_mut().meta.description = Some("Connected TCP client information.".to_string());
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}

#[cfg(test)]
mod tests;
