mod feedback;
mod transport;

use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};

use crate::app::module::{
    common::{
        streaming::{delivery::InputDelivery, input::StreamInputConfig, server_feedback::ServerFeedback},
        worker_wake::ModuleWake,
    },
    script_api::{CallbackDelivery, NativeCallbackPayload},
};
use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::PendingDrainState;

use crate::app::{
    module::common::streaming::{
        commands::{streaming_script_send_request, StreamingSendRequest},
        script as streaming_script,
        websocket::normalize_websocket_path,
    },
    StreamingModuleBase,
};

use self::transport::{WebSocketServerTransportConfig, WebSocketServerTransportHandle, WebSocketServerWorkerEvent};

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
    retry_at: Option<Instant>,
    retiring_transport: Option<WebSocketServerTransportHandle>,
    wake: Option<ModuleWake>,
    pending_deliveries: VecDeque<ServerDelivery>,
    feedback: ServerFeedback,
}

enum ServerDelivery {
    Input(InputDelivery),
    Callback(CallbackDelivery),
}
impl ServerDelivery {
    fn service(&mut self, ctx: &mut ProcessCtx, stream: &mut StreamingModuleBase) -> bool {
        match self {
            Self::Input(input) => input.service(ctx, stream),
            Self::Callback(callback) => callback.service(ctx),
        }
    }
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
            None,
            None,
            None,
            VecDeque::new(),
            ServerFeedback::default(),
        )
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;
        self.retry_at = None;

        if !self.module_enabled(snapshot) {
            self.retire_transport();
            self.last_transport_config = None;
            self.clear_port_warning();
            self.set_connected(false);
            self.clear_client_state(ctx, snapshot);
            return;
        }

        let config = match self.transport_config(snapshot) {
            Ok(config) => config,
            Err(error) => {
                logerror!("Invalid WebSocket server module configuration: {}", error);
                self.retire_transport();
                self.last_transport_config = None;
                self.set_port_warning(error.as_str());
                self.set_connected(false);
                self.clear_client_state(ctx, snapshot);
                return;
            }
        };

        if let Some(transport) = self.transport.as_ref().filter(|_| {
            self.last_transport_config
                .as_ref()
                .is_some_and(|last| last.same_endpoint(&config))
        }) {
            if let Err(error) = transport.update_input(config.input.clone()) {
                self.retire_transport();
                self.set_port_warning(&error);
                self.set_connected(false);
                self.clear_client_state(ctx, snapshot);
                return;
            }
            self.last_transport_config = Some(config);
            return;
        }

        self.retire_transport();
        self.set_connected(false);
        self.clear_client_state(ctx, snapshot);
        if self.retiring_transport.is_some() {
            self.transport_dirty = true;
            return;
        }
        let Some(notifier) = self.wake.as_ref().map(|wake| wake.notifier.clone()) else {
            return;
        };

        match WebSocketServerTransportHandle::spawn(config.clone(), notifier) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.set_connected(false);
            }
            Err(error) => {
                logerror!("Failed to start WebSocket server transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_port_warning(error.as_str());
                self.set_connected(false);
                self.transport_dirty = true;
                self.retry_at = Some(Instant::now() + Duration::from_millis(250));
                if let Some(wake) = &self.wake {
                    wake.later(Duration::from_millis(250));
                }
            }
        }
    }

    fn transport_refresh_due(&self) -> bool {
        self.transport_dirty && self.retiring_transport.is_none() && self.retry_at.is_none_or(|at| Instant::now() >= at)
    }

    fn transport_config(&self, snapshot: &ProcessTreeSnapshot) -> Result<WebSocketServerTransportConfig, String> {
        let port = snapshot
            .node(self.port.id())
            .and_then(|node| node.param_value.as_ref())
            .and_then(|value| match value {
                ParamValue::Int(port) => Some(*port),
                _ => None,
            })
            .ok_or("WebSocket server port is unavailable")?;
        let bind_port = u16::try_from(port)
            .map_err(|_| "WebSocket server port 'connection/port' must be between 0 and 65535".to_string())?;

        let path = snapshot
            .node(self.path.id())
            .and_then(|node| node.param_value.as_ref())
            .and_then(|value| {
                if let ParamValue::Str(path) = value {
                    Some(path.as_str())
                } else {
                    None
                }
            })
            .ok_or("WebSocket server path is unavailable")?;
        if path.len() > 8192 {
            return Err("WebSocket server path must fit 8192 bytes".into());
        }
        let path = path.trim();
        if !path.starts_with('/') && path.len() == 8192 {
            return Err("WebSocket server path must fit 8192 bytes including its leading slash".into());
        }
        let path = if path.is_empty() {
            String::new()
        } else {
            normalize_websocket_path(path)
        };

        let config = WebSocketServerTransportConfig {
            bind_host: WEBSOCKET_SERVER_BIND_HOST.to_string(),
            bind_port,
            path,
            receive_enabled: true,
            send_enabled: true,
            input: StreamInputConfig {
                module: self.id(),
                parse: self.stream.parse_config_cached(),
                processing_enabled: self.stream.processing_enabled_cached(),
                log_incoming: self.stream.log_incoming_enabled(),
            },
        };
        config.validate()?;
        Ok(config)
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        // One bounded native drain is retained until its callback receipts complete.
        if !self.pending_deliveries.is_empty() {
            return;
        }
        let retiring = self.retiring_transport.is_some();
        let Some(transport) = self.retiring_transport.as_ref().or(self.transport.as_ref()) else {
            return;
        };
        let mut events = Vec::new();
        let disconnected = transport.drain_events(&mut events).state == PendingDrainState::Disconnected;
        for event in events {
            match event {
                WebSocketServerWorkerEvent::Listening(_) if retiring => {}
                WebSocketServerWorkerEvent::Listening(address) => {
                    golden_core::logsuccess!(origin = self.id(); format!("WebSocket server listening on {address}."));
                    self.clear_port_warning();
                    self.set_connected(true);
                }
                WebSocketServerWorkerEvent::Recovering(_) if retiring => {}
                WebSocketServerWorkerEvent::Recovering(error) => {
                    self.set_port_warning(&error);
                    self.set_connected(false);
                }
                WebSocketServerWorkerEvent::ClientConnected {
                    client_id,
                    info,
                    callback,
                } => {
                    self.retain_callback(callback);
                    if !retiring {
                        self.client_infos.insert(client_id, info);
                        self.client_list_dirty = true;
                    }
                }
                WebSocketServerWorkerEvent::ClientDisconnected {
                    client_id,
                    reason,
                    rejected_lines,
                    callback,
                } => {
                    self.retain_callback(callback);
                    if let Some(reason) = reason {
                        logerror!(format!("WebSocket client {client_id} disconnected: {reason}"));
                    }
                    Self::report_rejected_input(rejected_lines);
                    if !retiring {
                        self.client_infos.remove(&client_id);
                        self.client_list_dirty = true;
                    }
                }
                WebSocketServerWorkerEvent::Input(input) => {
                    self.stream.emit_incoming_traffic(ctx);
                    if let Some(message) = &input.log_message {
                        golden_core::log!(origin = self.id(); message.as_str());
                    }
                    if let Err(error) = &input.values {
                        logerror!("Failed to parse WebSocket server input: {}", error);
                    }
                    Self::report_rejected_input(input.rejected_lines);
                    self.pending_deliveries
                        .push_back(ServerDelivery::Input(InputDelivery::new(input)));
                }
                WebSocketServerWorkerEvent::OutgoingLog(message) => {
                    golden_core::log!(origin = self.id(); message.as_str());
                }
                WebSocketServerWorkerEvent::Error(error) => {
                    logerror!("WebSocket server transport error: {}", error);
                }
            }
        }
        if disconnected {
            if retiring {
                self.retiring_transport = None;
            } else {
                logerror!(
                    "WebSocket server worker stopped; outstanding native outcomes are unknown and are not replayed."
                );
                self.transport = None;
                self.retry_at = Some(Instant::now() + Duration::from_millis(250));
            }
            self.last_transport_config = None;
            self.set_connected(false);
            self.transport_dirty = true;
        }
    }

    fn report_rejected_input(count: u64) {
        if count > 0 {
            logerror!(
                "Rejected {} WebSocket server lines at an input/configuration/connection boundary",
                count
            );
        }
    }

    fn queue_send_request(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        request: &StreamingSendRequest,
    ) -> Result<String, String> {
        if !self.node_data().effective_enabled {
            return Err("Stream module is disabled; command was not admitted".into());
        }

        if !self.module_enabled(snapshot)
            || self.transport_dirty
            || !self.last_transport_config.as_ref().is_some_and(|last| {
                self.transport_config(snapshot)
                    .is_ok_and(|applied| last.same_endpoint(&applied))
            })
        {
            return Err("WebSocket server configuration is pending or disabled; command was not admitted".into());
        }
        let connected_clients = self.client_infos.len();
        if connected_clients == 0 {
            return Err("no WebSocket clients are connected".to_string());
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "WebSocket server transport is not available".to_string())?;
        transport.send(request.frame_kind, &request.bytes, self.stream.log_outgoing_enabled())?;
        self.stream.emit_outgoing_traffic(ctx);

        Ok(format!(
            "Queued WebSocket {} for {} client(s)",
            request.description, connected_clients
        ))
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(command) =
            crate::app::module::common::streaming::commands::preparation::prepared_command(&event, self.id())
        else {
            return;
        };
        if !command.is_live() {
            logerror!("Prepared stream command source or target retired; command was not sent");
            return;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        if let Err(error) = self.queue_send_request(ctx, snapshot.as_ref(), &command.request) {
            logerror!("Prepared stream command was not admitted: {}", error);
        }
    }

    fn on_param_change_inner(&mut self, param: NodeId) {
        if (self.port.is_bound() && self.port.id() == param)
            || (self.path.is_bound() && self.path.id() == param)
            || self.stream.affects_input_configuration(param)
        {
            self.transport_dirty = true;
            self.retry_at = None;
        }
    }

    fn clients_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let connection_id = self.stream.connection_id()?;
        snapshot.find_child_by_decl_id(connection_id, "clients")
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

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
        if let Some(mut transport) = self.retiring_transport.take() {
            transport.stop();
        }
        self.last_transport_config = None;
    }

    fn retire_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            assert!(
                self.retiring_transport.is_none(),
                "one WebSocket server retirement owner"
            );
            transport.finish_admission();
            self.retiring_transport = Some(transport);
        }
        self.last_transport_config = None;
    }

    fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }

    fn arm_service(&self) {
        if !self.pending_deliveries.is_empty()
            || self.stream.has_pending_messages()
            || self.client_list_dirty
            || self.feedback.is_pending()
            || self.transport.as_ref().is_some_and(|transport| transport.has_pending())
            || self
                .retiring_transport
                .as_ref()
                .is_some_and(|transport| transport.has_pending())
        {
            self.request_service();
        } else if self.transport_dirty && self.retiring_transport.is_none() {
            if let Some(wake) = &self.wake {
                wake.later(self.retry_at.map_or(Duration::from_millis(1), |at| {
                    at.saturating_duration_since(Instant::now())
                }));
            }
        }
    }

    fn retain_callback(&mut self, callback: Result<NativeCallbackPayload, String>) {
        match callback {
            Ok(callback) => self
                .pending_deliveries
                .push_back(ServerDelivery::Callback(CallbackDelivery::new(
                    callback.event(self.id()),
                ))),
            Err(error) => {
                logerror!("WebSocket server callback was not admitted: {}", error);
            }
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
        let source = match ctx.node_waker() {
            Ok(source) => source,
            Err(error) => {
                logerror!("Failed to register WebSocket server worker wake: {}", error);
                return;
            }
        };
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.stop_transport();
            self.feedback = ServerFeedback::default();
            self.stream.discard_pending_input();
            self.pending_deliveries.clear();
            self.client_infos.clear();
            self.client_list_dirty = true;
            self.retry_at = None;
            self.transport_dirty = true;
            self.wake = match ModuleWake::new(source) {
                Ok(wake) => Some(wake),
                Err(error) => {
                    logerror!("Failed to register WebSocket server retry deadline: {}", error);
                    return;
                }
            };
        }
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.stream.refresh_config_cache(&snapshot);
            if self.transport_refresh_due() {
                self.refresh_transport(ctx, &snapshot);
            }
            if self.client_list_dirty {
                self.sync_client_nodes(ctx, &snapshot);
            }
        }
        self.arm_service();
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.settle_feedback();
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
        self.drain_transport_events(ctx);
        if let Some(delivery) = self.pending_deliveries.front_mut() {
            if delivery.service(ctx, &mut self.stream) {
                self.pending_deliveries.pop_front();
            }
        }
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.stream.refresh_config_cache(&snapshot);
            self.refresh_data_capabilities(ctx, &snapshot);
            if self.transport_refresh_due() {
                self.refresh_transport(ctx, &snapshot);
            }
            if self.client_list_dirty {
                self.sync_client_nodes(ctx, &snapshot);
            }
            self.stream.process_pending(ctx, &snapshot);
        }
        self.arm_service();
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
        self.wake = None;
        self.retry_at = None;
        self.pending_deliveries.clear();
        self.feedback = ServerFeedback::default();
        self.stream.discard_pending_input();
        self.client_infos.clear();
        self.client_list_dirty = false;
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.transport_refresh_due()
            || self.stream.has_pending_messages()
            || self.client_list_dirty
            || self.feedback.failed()
    }

    fn worker_wake_when_disabled(&self) -> bool {
        self.retiring_transport.is_some()
            || self.transport_dirty
            || self.client_list_dirty
            || self.feedback.is_pending()
            || !self.pending_deliveries.is_empty()
            || self.stream.has_pending_messages()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive()
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
        if self.transport_dirty {
            self.request_service();
        }
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if let Some(enabled) = patch.enabled {
            if node != self.id() {
                let _ = ctx;
                let _ = enabled;
                self.transport_dirty = true;
                self.retry_at = None;
                self.request_service();
            }
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.retry_at = None;
        if enabled {
            self.transport_dirty = true;
        } else {
            self.retire_transport();
            self.last_transport_config = None;
            self.set_connected(false);
            if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
                let snapshot = snapshot_arc.as_ref();
                self.clear_port_warning();
                self.clear_client_state(ctx, snapshot);
            }
            self.transport_dirty = false;
        }
        self.request_service();
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn create_client_info_param(client_id: &str, info: &str) -> Parameter {
    let mut parameter = Parameter::new(
        client_id,
        ParamValue::Str(info.to_string()),
        ParameterChangeCheck::ValueChange,
    );
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter.node_data_mut().meta.user_permissions = golden_core::node::NodeUserPermissions::none();
    parameter
}

#[cfg(test)]
mod tests;
