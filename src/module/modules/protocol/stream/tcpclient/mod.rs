mod transport;

use std::sync::mpsc::TryRecvError;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::streaming::{
        commands::StreamingSendRequest,
        module_helpers::{format_bytes_for_log, streaming_command_type_supported},
    },
    StreamingModuleBase,
};

use self::transport::{
    StreamingWorkerEvent, TcpStreamingConnectionStatus, TcpStreamingTransportConfig,
    TcpStreamingTransportHandle,
};

const TCP_CLIENT_MODULE_UPDATE_RATE_HZ: u32 = 120;
const TCP_CLIENT_TARGET_WARNING_ID: &str = "tcp_client_target_transport";

#[node("tcp_client_module", label = "TCP Client")]
#[children(
    folder(connection) {
        remote_host: String = "127.0.0.1".to_string() (
            label = "Remote Host",
            description = "TCP server hostname or IP address."
        );
        remote_port: i32 = 9002 [0..65535] (
            label = "Remote Port",
            description = "TCP server port to connect to.",
            widget = "text"
        );
        [base_children];
    }
)]
pub struct TcpClientModule {
    stream: StreamingModuleBase,
    transport: Option<TcpStreamingTransportHandle>,
    last_transport_config: Option<TcpStreamingTransportConfig>,
    transport_dirty: bool,
}

impl TcpClientModule {
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
            self.clear_target_warning(ctx, snapshot);
            self.stream.set_connected(ctx, false);
            return;
        }

        let config = match self.transport_config() {
            Ok(config) => config,
            Err(error) => {
                logerror!("Invalid TCP module configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_target_warning(ctx, snapshot, error.as_str());
                self.stream.set_connected(ctx, false);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            return;
        }

        self.stop_transport();

        match TcpStreamingTransportHandle::spawn(config.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_target_warning(ctx, snapshot);
                self.stream.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!("Failed to start TCP transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_target_warning(ctx, snapshot, error.as_str());
                self.stream.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(&self) -> Result<TcpStreamingTransportConfig, String> {
        let remote_host = self.remote_host.get_ref().clone();
        if remote_host.trim().is_empty() {
            return Err("TCP remote host cannot be empty".to_string());
        }

        let remote_port = u16::try_from(self.remote_port.get())
            .map_err(|_| "TCP remote port 'connection/target/remote_port' must be between 0 and 65535".to_string())?;

        Ok(TcpStreamingTransportConfig {
            remote_host,
            remote_port,
            receive_enabled: true,
            send_enabled: true,
        })
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
                StreamingWorkerEvent::Bytes(bytes) if !processing_enabled => {
                    if self.stream.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!(
                                "Received TCP {} (processing disabled)",
                                format_bytes_for_log(bytes.as_slice())
                            )
                        );
                    }
                }
                StreamingWorkerEvent::Bytes(bytes) => match self.stream.parse_bytes(bytes.as_slice(), snapshot) {
                    Ok(messages) => {
                        received_bytes = true;
                        if self.stream.log_incoming_enabled() {
                            golden_core::log!(
                                origin = self.id();
                                format!("Received TCP {}", format_bytes_for_log(bytes.as_slice()))
                            );
                        }
                        self.stream.push_messages(messages);
                    }
                    Err(error) => {
                        logerror!("Failed to parse TCP input: {}", error);
                    }
                },
                StreamingWorkerEvent::Error(error) => {
                    logerror!("TCP transport error: {}", error);
                }
                StreamingWorkerEvent::Status(status) => match status {
                    TcpStreamingConnectionStatus::Connected { remote_address } => {
                        golden_core::logsuccess!(
                            origin = self.id();
                            format!("Connected TCP stream to {remote_address}.")
                        );
                        self.clear_target_warning(ctx, snapshot);
                        self.stream.set_connected(ctx, true);
                    }
                    TcpStreamingConnectionStatus::Recovering { message, .. } => {
                        logerror!("TCP transport recovering: {}", message);
                        self.set_target_warning(ctx, snapshot, message.as_str());
                        self.stream.set_connected(ctx, false);
                    }
                },
            }
        }

        if worker_disconnected {
            logerror!("TCP transport worker stopped unexpectedly; restarting.");
            self.stop_transport();
            self.last_transport_config = None;
            self.set_target_warning(
                ctx,
                snapshot,
                "TCP transport worker stopped unexpectedly. Restarting.",
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
            .ok_or_else(|| "TCP transport is not available".to_string())?;
        transport.send(request.bytes.clone())?;
        self.stream.emit_outgoing_traffic(ctx);

        if self.stream.log_outgoing_enabled() {
            golden_core::log!(
                origin = self.id();
                format!("Sent TCP {}", format_bytes_for_log(request.bytes.as_slice()))
            );
        }

        Ok(format!("Queued TCP {}", request.description))
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
            .map_err(|error| format!("invalid TCP command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, snapshot, &payload))
        {
            logerror!(format!(
                "Failed to handle TCP command {:?}: {error}",
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
    }

    fn target_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        self.stream.connection_child_node_id(snapshot, "target")
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, _snapshot: &ProcessTreeSnapshot) {
        self.stream
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
    }

    fn set_target_warning(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, message: &str) {
        let Some(target_id) = self.target_node_id(snapshot) else {
            return;
        };
        NodeHandle::new(target_id).set_warning_with(ctx, Some(TCP_CLIENT_TARGET_WARNING_ID), message, None);
    }

    fn clear_target_warning(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(target_id) = self.target_node_id(snapshot) else {
            return;
        };
        NodeHandle::new(target_id).clear_warning(ctx, Some(TCP_CLIENT_TARGET_WARNING_ID));
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }
}

#[golden_core::item(
    "module",
    node = "tcp_client_module",
    via = stream,
    from_struct,
    menu_path = ["Generic"]
)]
impl Node for TcpClientModule {
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
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.drain_transport_events(ctx, snapshot);

        let needs_snapshot = self.transport_dirty || self.stream.has_pending_messages();
        if !needs_snapshot {
            return;
        }

        self.refresh_data_capabilities(ctx, snapshot);

        if self.transport_dirty {
            self.refresh_transport(ctx, snapshot);
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
        NodeExecutionRule::periodic(TCP_CLIENT_MODULE_UPDATE_RATE_HZ)
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
                        self.clear_target_warning(ctx, snapshot_arc.as_ref());
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

#[cfg(test)]
mod tests;
