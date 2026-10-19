mod transport;

use crate::app::module::common::{
    streaming::{delivery::InputDelivery, input::StreamInputConfig},
    worker_wake::ModuleWake,
};
use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::PendingDrainState;
use std::collections::VecDeque;

use crate::app::{
    module::common::streaming::{
        commands::{ScriptSend, StreamSend},
        script as streaming_script,
    },
    StreamingModuleBase,
};

use self::transport::{
    StreamingWorkerEvent, WebSocketClientConnectionStatus, WebSocketClientTransportConfig,
    WebSocketClientTransportHandle,
};

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
        path: String = "/" (
            label = "Path",
            description = "WebSocket request path used during the opening handshake."
        );
        [base_children];
    }
)]
pub struct WebSocketClientModule {
    stream: StreamingModuleBase,
    transport: Option<WebSocketClientTransportHandle>,
    retiring_transport: Option<WebSocketClientTransportHandle>,
    last_transport_config: Option<WebSocketClientTransportConfig>,
    transport_dirty: bool,
    wake: Option<ModuleWake>,
    retry_at: Option<std::time::Instant>,
    pending_inputs: VecDeque<InputDelivery>,
}

impl WebSocketClientModule {
    pub fn create() -> Self {
        Self::new(
            StreamingModuleBase::create(),
            None,
            None,
            None,
            true,
            None,
            None,
            VecDeque::new(),
        )
    }

    fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }

    fn retire_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            debug_assert!(self.retiring_transport.is_none());
            transport.finish_admission();
            self.retiring_transport = Some(transport);
        }
        self.last_transport_config = None;
    }

    fn transport_refresh_due(&self) -> bool {
        self.transport_dirty
            && self.retiring_transport.is_none()
            && self.retry_at.is_none_or(|at| std::time::Instant::now() >= at)
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if let Some(at) = self.retry_at.filter(|at| std::time::Instant::now() < *at) {
            if let Some(wake) = &self.wake {
                wake.later(at.saturating_duration_since(std::time::Instant::now()));
            }
            return;
        }
        self.retry_at = None;
        self.transport_dirty = false;

        if !self.module_enabled(snapshot) {
            self.retire_transport();
            self.last_transport_config = None;
            self.clear_target_warning(ctx);
            self.stream.set_connected(ctx, false);
            return;
        }

        let config = match self.transport_config(snapshot) {
            Ok(config) => config,
            Err(error) => {
                logerror!(format!("Invalid WebSocket module configuration: {}", error));
                self.retire_transport();
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.stream.set_connected(ctx, false);
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
                self.set_target_warning(ctx, &error);
                self.stream.set_connected(ctx, false);
                return;
            }
            self.last_transport_config = Some(config);
            return;
        }

        self.retire_transport();
        self.stream.set_connected(ctx, false);
        if self.retiring_transport.is_some() {
            self.transport_dirty = true;
            return;
        }
        let Some(wake) = &self.wake else {
            return;
        };

        match WebSocketClientTransportHandle::spawn(config.clone(), wake.notifier.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_target_warning(ctx);
                self.stream.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!(format!("Failed to start WebSocket transport: {}", error));
                self.transport = None;
                self.last_transport_config = None;
                self.transport_dirty = true;
                self.retry_at = Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
                wake.later(std::time::Duration::from_millis(250));
                self.set_target_warning(ctx, error.as_str());
                self.stream.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(&self, snapshot: &ProcessTreeSnapshot) -> Result<WebSocketClientTransportConfig, String> {
        let Some(ParamValue::Str(host)) = snapshot
            .node(self.remote_host.id())
            .and_then(|node| node.param_value.as_ref())
        else {
            return Err("WebSocket remote host is unavailable".into());
        };
        if host.trim().is_empty() || host.len() > 1024 {
            return Err("WebSocket remote host must contain 1–1024 bytes".to_string());
        }
        let remote_host = host.trim().to_string();

        let Some(ParamValue::Int(port)) = snapshot
            .node(self.remote_port.id())
            .and_then(|node| node.param_value.as_ref())
        else {
            return Err("WebSocket remote port is unavailable".into());
        };
        let remote_port = u16::try_from(*port)
            .map_err(|_| "WebSocket remote port 'connection/remote_port' must be between 0 and 65535".to_string())?;

        let Some(ParamValue::Str(path)) = snapshot.node(self.path.id()).and_then(|node| node.param_value.as_ref())
        else {
            return Err("WebSocket path is unavailable".into());
        };
        if path.len() > 8192 {
            return Err("WebSocket path exceeds 8192 UTF-8 bytes".into());
        }
        let path = crate::app::module::common::streaming::websocket::normalize_websocket_path(path);
        let config = WebSocketClientTransportConfig {
            remote_host,
            remote_port,
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
        // Retain at most one native drain until its callback receipts complete.
        if !self.pending_inputs.is_empty() {
            return;
        }
        let retiring = self.retiring_transport.is_some();
        let (worker_events, worker_disconnected) = {
            let Some(transport) = self.retiring_transport.as_ref().or(self.transport.as_ref()) else {
                return;
            };

            let mut worker_events = Vec::new();
            let worker_disconnected =
                transport.drain_events(&mut worker_events).state == PendingDrainState::Disconnected;

            (worker_events, worker_disconnected)
        };

        let mut received_bytes = false;
        for event in worker_events {
            match event {
                StreamingWorkerEvent::Input(input) => {
                    received_bytes = true;
                    if let Some(message) = &input.log_message {
                        golden_core::log!(origin = self.id(); message.as_str());
                    }
                    if let Err(error) = &input.values {
                        logerror!(format!("Failed to parse WebSocket input: {}", error));
                    }
                    Self::report_rejected_input(input.rejected_lines);
                    self.pending_inputs.push_back(InputDelivery::new(input));
                }
                StreamingWorkerEvent::RejectedInput(count) => Self::report_rejected_input(count),
                StreamingWorkerEvent::Error(error) => {
                    logerror!(format!("WebSocket transport error: {}", error));
                }
                StreamingWorkerEvent::OutgoingLog(message) => {
                    golden_core::log!(origin = self.id(); message.as_str());
                }
                StreamingWorkerEvent::Status(_) if retiring => {}
                StreamingWorkerEvent::Status(status) => match status {
                    WebSocketClientConnectionStatus::Connected { remote_address } => {
                        golden_core::logsuccess!(
                            origin = self.id();
                            format!("Connected WebSocket stream to {remote_address}.")
                        );
                        self.stream.set_connected(ctx, true);
                        self.clear_target_warning(ctx);
                    }
                    WebSocketClientConnectionStatus::Recovering { message, .. } => {
                        logerror!(format!("WebSocket transport recovering: {}", message));
                        self.stream.set_connected(ctx, false);
                        self.set_target_warning(ctx, &message);
                    }
                },
            }
        }

        if worker_disconnected {
            if retiring {
                self.retiring_transport = None;
            } else {
                logerror!(
                    "WebSocket transport worker stopped; outstanding commands have an unknown outcome and are not replayed."
                );
                self.transport = None;
                self.retry_at = Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
                if let Some(wake) = &self.wake {
                    wake.later(std::time::Duration::from_millis(250));
                }
            }
            self.last_transport_config = None;
            self.stream.set_connected(ctx, false);
            self.transport_dirty = true;
        }

        if received_bytes {
            self.stream.emit_incoming_traffic(ctx);
        }
    }

    fn report_rejected_input(count: u64) {
        if count > 0 {
            logerror!(
                "Rejected {} WebSocket lines at an input/configuration/connection boundary",
                count
            );
        }
    }

    fn queue_send_request(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        request: StreamSend<'_>,
    ) -> Result<String, String> {
        if !self.node_data().effective_enabled {
            return Err("Stream module is disabled; command was not admitted".into());
        }

        if !self.module_enabled(snapshot) {
            return Err("WebSocket module is disabled; command was not admitted".into());
        }
        let applied = self.transport_config(snapshot)?;
        if !self
            .last_transport_config
            .as_ref()
            .is_some_and(|last| last.same_endpoint(&applied))
        {
            return Err("WebSocket endpoint configuration is pending; command was not admitted".into());
        }
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "WebSocket transport is not available".to_string())?;
        transport.send(
            request.frame_kind(),
            request.input(),
            self.stream.log_outgoing_enabled(),
        )?;
        self.stream.emit_outgoing_traffic(ctx);

        Ok(format!("Queued WebSocket {}", request.description()))
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
        if let Err(error) = self.queue_send_request(ctx, snapshot.as_ref(), (&command.request).into()) {
            logerror!(format!("Prepared stream command was not admitted: {}", error));
        }
    }

    fn on_param_change_inner(&mut self, param: NodeId) {
        if self.param_affects_transport(param) {
            self.transport_dirty = true;
            self.retry_at = None;
        }
    }

    fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.remote_host.is_bound() && self.remote_host.id() == param)
            || (self.remote_port.is_bound() && self.remote_port.id() == param)
            || (self.path.is_bound() && self.path.id() == param)
            || self.stream.affects_input_configuration(param)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, _snapshot: &ProcessTreeSnapshot) {
        self.stream
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, true));
    }

    fn set_target_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        let Some(target_id) = self.stream.connection_id() else {
            return;
        };
        NodeHandle::new(target_id).set_warning_with(ctx, Some(WEBSOCKET_CLIENT_TARGET_WARNING_ID), message, None);
    }

    fn clear_target_warning(&self, ctx: &mut ProcessCtx) {
        let Some(target_id) = self.stream.connection_id() else {
            return;
        };
        NodeHandle::new(target_id).clear_warning(ctx, Some(WEBSOCKET_CLIENT_TARGET_WARNING_ID));
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
        if let Some(mut transport) = self.retiring_transport.take() {
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
        let source = match ctx.node_waker() {
            Ok(source) => source,
            Err(error) => {
                logerror!(format!("Failed to register WebSocket worker wake: {}", error));
                return;
            }
        };
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.stop_transport();
            self.stream.discard_pending_input();
            self.pending_inputs.clear();
            self.wake = match ModuleWake::new(source) {
                Ok(wake) => Some(wake),
                Err(error) => {
                    logerror!(format!("Failed to register WebSocket retry deadline: {}", error));
                    return;
                }
            };
        }
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.stream.refresh_config_cache(snapshot);
        self.refresh_transport(ctx, snapshot);
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
        self.drain_transport_events(ctx);
        if let Some(input) = self.pending_inputs.front_mut() {
            if input.service(ctx, &mut self.stream) {
                self.pending_inputs.pop_front();
            }
        }

        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            self.stream.refresh_config_cache(snapshot);
            self.refresh_data_capabilities(ctx, snapshot);
            if self.transport_refresh_due() {
                self.refresh_transport(ctx, snapshot);
            }
            self.stream.process_pending(ctx, snapshot);
        }
        if self.stream.has_pending_messages()
            || !self.pending_inputs.is_empty()
            || self.transport.as_ref().is_some_and(|transport| transport.has_pending())
            || self
                .retiring_transport
                .as_ref()
                .is_some_and(|transport| transport.has_pending())
        {
            self.request_service();
        } else if self.transport_dirty && self.retiring_transport.is_none() {
            if let Some(wake) = &self.wake {
                wake.later(self.retry_at.map_or(std::time::Duration::from_millis(1), |at| {
                    at.saturating_duration_since(std::time::Instant::now())
                }));
            }
        }
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
        self.wake = None;
        self.retry_at = None;
        self.stream.discard_pending_input();
        self.pending_inputs.clear();
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.transport_refresh_due() || self.stream.has_pending_messages()
    }
    fn worker_wake_when_disabled(&self) -> bool {
        self.retiring_transport.is_some()
            || self.transport_dirty
            || !self.pending_inputs.is_empty()
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
        if let Some(request) = ScriptSend::capture(ctx, method, args) {
            let request = request?;
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return Err(format!("method '{method}' is unavailable without a tree snapshot"));
            };
            self.queue_send_request(ctx, snapshot_arc.as_ref(), (&request).into())?;
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

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if patch.enabled.is_some() && node != self.id() {
            self.transport_dirty = true;
            self.retry_at = None;
            self.request_service();
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.retry_at = None;
        if enabled {
            self.transport_dirty = true;
            self.request_service();
        } else {
            self.retire_transport();
            self.last_transport_config = None;
            self.clear_target_warning(ctx);
            self.stream.set_connected(ctx, false);
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

#[cfg(test)]
mod tests;
