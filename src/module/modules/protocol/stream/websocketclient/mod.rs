mod transport;

#[cfg(test)]
mod transport_tests;

use std::sync::mpsc::TryRecvError;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::streaming::{
        commands::{streaming_script_send_request, StreamingSendFrameKind, StreamingSendRequest},
        module_helpers::{format_bytes_for_log, streaming_command_type_supported},
        script as streaming_script,
        websocket::normalize_websocket_path,
    },
    StreamingModuleBase,
};

use self::transport::{
    StreamingWorkerEvent, WebSocketClientConnectionStatus, WebSocketClientTransportConfig,
    WebSocketClientTransportHandle,
};

const WEBSOCKET_CLIENT_MODULE_UPDATE_RATE_HZ: u32 = 120;
const WEBSOCKET_CLIENT_TARGET_WARNING_ID: &str = "websocket_client_target_transport";

#[node("websocket_client_module", label = "WebSocket Client")]
#[children(
    folder(connection) {
        remote_host: String = "127.0.0.1".to_string() (
            label = "Remote Host",
            description = "WebSocket server hostname or IP address."
        );
        remote_port: i32 = 9002 [0..65535] (
            label = "Remote Port",
            description = "WebSocket server port to connect to.",
            widget = "text"
        );
        path: String = "/".to_string() (
            label = "Path",
            description = "WebSocket request path used during the opening handshake."
        );
        [base_children];
    }
)]
pub struct WebSocketClientModule {
    stream: StreamingModuleBase,
    transport: Option<WebSocketClientTransportHandle>,
    last_transport_config: Option<WebSocketClientTransportConfig>,
    transport_dirty: bool,
}

impl WebSocketClientModule {
    pub fn create() -> Self {
        Self::new(StreamingModuleBase::create(), None, None, true)
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
            self.stream.set_connected(ctx, false);
            return;
        }

        let config = match self.transport_config() {
            Ok(config) => config,
            Err(error) => {
                logerror!("Invalid WebSocket client configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.stream.set_connected(ctx, false);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            return;
        }

        self.stop_transport();

        match WebSocketClientTransportHandle::spawn(config.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_target_warning(ctx);
                self.stream.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!("Failed to start WebSocket client transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.stream.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(&self) -> Result<WebSocketClientTransportConfig, String> {
        let remote_host = self.remote_host.get_ref().trim().to_string();
        if remote_host.is_empty() {
            return Err("WebSocket remote host cannot be empty".to_string());
        }

        let remote_port = u16::try_from(self.remote_port.get()).map_err(|_| {
            "WebSocket remote port 'connection/remote_port' must be between 0 and 65535".to_string()
        })?;

        Ok(WebSocketClientTransportConfig {
            remote_host,
            remote_port,
            path: normalize_websocket_path(self.path.get_ref().as_str()),
            receive_enabled: true,
            send_enabled: true,
        })
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        let (worker_events, worker_disconnected) = {
            let Some(transport) = &self.transport else {
                return;
            };

            transport.clear_pending();
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

        let processing_enabled = self.stream.processing_enabled_cached();
        let mut received_bytes = false;
        for event in worker_events {
            match event {
                StreamingWorkerEvent::Bytes { frame_kind, bytes } if !processing_enabled => {
                    streaming_script::emit_stream_bytes_callbacks(
                        ctx,
                        self.id(),
                        bytes.as_slice(),
                        None,
                        frame_kind == StreamingSendFrameKind::Text,
                    );
                    if self.stream.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!(
                                "Received WebSocket {} (processing disabled)",
                                format_bytes_for_log(bytes.as_slice())
                            )
                        );
                    }
                }
                StreamingWorkerEvent::Bytes { frame_kind, bytes } => {
                    streaming_script::emit_stream_bytes_callbacks(
                        ctx,
                        self.id(),
                        bytes.as_slice(),
                        None,
                        frame_kind == StreamingSendFrameKind::Text,
                    );
                    match self.stream.parse_bytes_cached(bytes.as_slice()) {
                        Ok(messages) => {
                            received_bytes = true;
                            if self.stream.log_incoming_enabled() {
                                golden_core::log!(
                                    origin = self.id();
                                    format!("Received WebSocket {}", format_bytes_for_log(bytes.as_slice()))
                                );
                            }
                            self.stream.push_messages(messages);
                        }
                        Err(error) => {
                            logerror!("Failed to parse WebSocket input: {}", error);
                        }
                    }
                }
                StreamingWorkerEvent::Error(error) => {
                    logerror!("WebSocket client transport error: {}", error);
                }
                StreamingWorkerEvent::Status(status) => match status {
                    WebSocketClientConnectionStatus::Connected { remote_address } => {
                        golden_core::logsuccess!(
                            origin = self.id();
                            format!("Connected WebSocket client to {remote_address}.")
                        );
                        // clear_target_warning uses a bound handle — no snapshot needed.
                        self.clear_target_warning(ctx);
                        self.stream.set_connected(ctx, true);
                    }
                    WebSocketClientConnectionStatus::Recovering { message, .. } => {
                        logerror!("WebSocket client recovering: {}", message);
                        self.set_target_warning(ctx, message.as_str());
                        self.stream.set_connected(ctx, false);
                    }
                },
            }
        }

        if worker_disconnected {
            logerror!("WebSocket client worker stopped unexpectedly; restarting.");
            self.stop_transport();
            self.last_transport_config = None;
            self.set_target_warning(
                ctx,
                "WebSocket client worker stopped unexpectedly. Restarting.",
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
            .ok_or_else(|| "WebSocket client transport is not available".to_string())?;
        transport.send(request.frame_kind, request.bytes.clone())?;
        self.stream.emit_outgoing_traffic(ctx);

        if self.stream.log_outgoing_enabled() {
            golden_core::log!(
                origin = self.id();
                format!("Sent WebSocket {}", format_bytes_for_log(request.bytes.as_slice()))
            );
        }

        Ok(format!("Queued WebSocket {}", request.description))
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
            .map_err(|error| format!("invalid WebSocket client command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, snapshot, &payload))
        {
            logerror!(format!(
                "Failed to handle WebSocket client command {:?}: {error}",
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
        (self.remote_host.is_bound() && self.remote_host.id() == param)
            || (self.remote_port.is_bound() && self.remote_port.id() == param)
            || (self.path.is_bound() && self.path.id() == param)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, _snapshot: &ProcessTreeSnapshot) {
        self.stream
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
    }

    fn set_target_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        NodeHandle::new(self.remote_host.id()).set_warning_with(
            ctx,
            Some(WEBSOCKET_CLIENT_TARGET_WARNING_ID),
            message,
            None,
        );
    }

    fn clear_target_warning(&self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.remote_host.id()).clear_warning(ctx, Some(WEBSOCKET_CLIENT_TARGET_WARNING_ID));
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }
}

#[golden_core::item(
    "module",
    node = "websocket_client_module",
    via = stream,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for WebSocketClientModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.stream.init(ctx);
        self.transport_dirty = true;

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.refresh_transport(ctx, snapshot);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        // Drain transport bytes using only cached config — no snapshot needed.
        self.drain_transport_events(ctx);

        let needs_snapshot = self.transport_dirty || self.stream.has_pending_messages();
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

        self.stream.process_pending(ctx, snapshot);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn needs_update(&self) -> bool {
        self.transport_dirty
            || self.stream.has_pending_messages()
            || self.transport.as_ref().is_some_and(|t| t.has_pending())
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.transport_dirty || self.stream.has_pending_messages()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(WEBSOCKET_CLIENT_MODULE_UPDATE_RATE_HZ)
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
            self.clear_target_warning(ctx);
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
