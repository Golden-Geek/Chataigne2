mod transport;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::streaming::{
        commands::{streaming_script_send_request, StreamingSendRequest},
        module_helpers::{child_int_param, child_string_param, format_bytes_for_log, streaming_command_type_supported},
        script as streaming_script,
    },
    StreamingModuleBase,
};

use self::transport::{
    StreamingWorkerEvent, UdpStreamingOutboundPacket, UdpStreamingTransportConfig, UdpStreamingTransportHandle,
};

const UDP_INTERFACE_REFRESH_INTERVAL_SECS: f64 = 1.0;
const UDP_MODULE_UPDATE_RATE_HZ: u32 = 120;
const UDP_INPUT_WARNING_ID: &str = "udp_input_transport";
const UDP_INTERFACE_WARNING_ID: &str = "udp_network_interface_options";
const UDP_OUTPUT_NODE_TYPE: &str = "udp_output";
const UDP_INPUT_DECL_ID: &str = "input";

#[derive(Clone, Debug, PartialEq, Eq)]
struct UdpOutputTarget {
    remote_host: String,
    remote_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UdpTransportBinding {
    interface_variant: String,
    bind_port: u16,
    receive_enabled: bool,
    send_enabled: bool,
}

#[node("udp_output", label = "Output")]
#[children(
    remote_host: String = "127.0.0.1".to_string() (
        label = "Remote Host",
        description = "Remote UDP destination hostname or IP address."
    );
    remote_port: i32 = 8000 [0..65535] (
        label = "Remote Port",
        description = "UDP port used for outgoing packets.",
        widget = "text"
    );
)]
pub struct UdpOutput {}

#[golden_core::item("udp_output", node = "udp_output", from_struct)]
impl Node for UdpOutput {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

#[node("udp_output_manager", label = "Outputs")]
pub struct UdpOutputManager {}

#[node("udp_output_manager", from_struct)]
impl Node for UdpOutputManager {
    golden_core::define_user_item_factory_methods! {
        accepts = ["udp_output", "folder"];
        items = [
            {
                node_type: "udp_output",
                item_kind: "udp_output",
                label: "Output",
                select_when_created: false,
                create: |_this: &Self| UdpOutput::new()
            }
        ];
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

#[node("udp_module", label = "UDP")]
#[children(
    folder(connection) {
        network_interface: Enum = "any" (
            label = "Network Interface",
            description = "Network interface used to receive UDP packets and as the source binding for outgoing traffic.",
            enum_options = ["any (default)"]
        );
        folder(input, label = "Input", can_be_disabled = true) {
            port: i32 = 9001 [0..65535] (
                label = "Port",
                description = "UDP port used to receive packets when Input is enabled.",
                widget = "text"
            );
        }
        node outputs: UdpOutputManager = UdpOutputManager::new() (
            label = "Outputs",
            description = "UDP destinations used by this module for outgoing command traffic.",
            can_be_disabled = true
        );
        [base_children];
    }
)]
pub struct UdpModule {
    stream: StreamingModuleBase,
    interface_refresh_elapsed: f64,
    transport: Option<UdpStreamingTransportHandle>,
    last_transport_config: Option<UdpStreamingTransportConfig>,
    transport_dirty: bool,
}

impl UdpModule {
    pub fn create() -> Self {
        Self::new(StreamingModuleBase::create(), 0.0, None, None, true)
    }

    fn interface_refresh_due(&self) -> bool {
        self.interface_refresh_elapsed >= UDP_INTERFACE_REFRESH_INTERVAL_SECS
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_interface_options(&self, ctx: &mut ProcessCtx) {
        if !self.network_interface.is_bound() {
            return;
        }

        match crate::app::module::common::network_interfaces::available_interface_options() {
            Ok(options) => {
                crate::app::module::common::network_interfaces::sync_interface_enum_options(
                    ctx,
                    self.network_interface.id(),
                    options,
                );
                self.network_interface
                    .clear_warning(ctx, Some(UDP_INTERFACE_WARNING_ID));
            }
            Err(error) => {
                self.network_interface
                    .set_warning_with(ctx, Some(UDP_INTERFACE_WARNING_ID), error.as_str(), None);
            }
        }
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;

        if !self.module_enabled(snapshot) {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_input_warning(ctx, snapshot);
            self.stream.set_connected(ctx, false);
            return;
        }

        let binding = match self.transport_binding(snapshot) {
            Ok(binding) => binding,
            Err(error) => {
                logerror!("Invalid UDP module configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_input_warning(ctx, snapshot, error.as_str());
                self.stream.set_connected(ctx, false);
                return;
            }
        };

        if !binding.receive_enabled && !binding.send_enabled {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_input_warning(ctx, snapshot);
            self.stream.set_connected(ctx, false);
            return;
        }

        let config = UdpStreamingTransportConfig {
            bind_interface_host: crate::app::module::common::network_interfaces::bind_host_for_interface_variant(
                binding.interface_variant.as_str(),
            ),
            bind_port: binding.bind_port,
            receive_enabled: binding.receive_enabled,
        };

        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            self.clear_input_warning(ctx, snapshot);
            self.stream.set_connected(ctx, true);
            return;
        }

        self.stop_transport();

        match UdpStreamingTransportHandle::spawn(config.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_input_warning(ctx, snapshot);
                self.stream.set_connected(ctx, true);
            }
            Err(error) => {
                logerror!("Failed to start UDP transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_input_warning(ctx, snapshot, error.as_str());
                self.stream.set_connected(ctx, false);
            }
        }
    }

    fn transport_binding(&self, snapshot: &ProcessTreeSnapshot) -> Result<UdpTransportBinding, String> {
        let input_id = self
            .stream
            .connection_child_node_id(snapshot, UDP_INPUT_DECL_ID)
            .ok_or_else(|| "missing UDP input folder 'connection/input'".to_string())?;
        let receive_enabled = snapshot
            .node(input_id)
            .map(|node| node.enabled)
            .ok_or_else(|| "missing UDP input folder 'connection/input'".to_string())?;

        let bind_port = if receive_enabled {
            u16::try_from(self.port.get())
                .map_err(|_| "UDP port 'connection/input/port' must be between 0 and 65535".to_string())?
        } else {
            0
        };

        Ok(UdpTransportBinding {
            interface_variant: self.network_interface.get_ref().as_str().to_string(),
            bind_port,
            receive_enabled,
            send_enabled: self.outputs_enabled(snapshot).unwrap_or(false),
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
                StreamingWorkerEvent::Bytes(bytes) if !processing_enabled => {
                    streaming_script::emit_stream_bytes_callbacks(
                        ctx,
                        self.id(),
                        bytes.as_slice(),
                        None,
                        true,
                    );
                    if self.stream.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Received UDP {} (processing disabled)", format_bytes_for_log(bytes.as_slice()))
                        );
                    }
                }
                StreamingWorkerEvent::Bytes(bytes) => {
                    streaming_script::emit_stream_bytes_callbacks(
                        ctx,
                        self.id(),
                        bytes.as_slice(),
                        None,
                        true,
                    );
                    match self.stream.parse_bytes(bytes.as_slice(), snapshot) {
                        Ok(messages) => {
                            received_bytes = true;
                            if self.stream.log_incoming_enabled() {
                                golden_core::log!(
                                    origin = self.id();
                                    format!("Received UDP {}", format_bytes_for_log(bytes.as_slice()))
                                );
                            }
                            self.stream.push_messages(messages);
                        }
                        Err(error) => {
                            logerror!("Failed to parse UDP input: {}", error);
                        }
                    }
                }
                StreamingWorkerEvent::Error(error) => {
                    logerror!("UDP transport error: {}", error);
                }
            }
        }

        if received_bytes {
            self.stream.emit_incoming_traffic(ctx);
        }
    }

    fn ensure_default_output(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(outputs_id) = self.outputs.current_id() else {
            return;
        };

        let mut output_ids = Vec::new();
        collect_outputs_recursive(snapshot, outputs_id, &mut output_ids);
        if !output_ids.is_empty() {
            return;
        }

        ctx.add_user_item_boxed(outputs_id, Box::new(UdpOutput::new()), None);
    }

    fn collect_enabled_outputs(&self, snapshot: &ProcessTreeSnapshot) -> Vec<UdpOutputTarget> {
        if !self.outputs_enabled(snapshot).unwrap_or(false) {
            return Vec::new();
        }

        let Some(outputs_id) = self.outputs.current_id() else {
            return Vec::new();
        };

        let mut output_ids = Vec::new();
        collect_outputs_recursive(snapshot, outputs_id, &mut output_ids);

        output_ids
            .into_iter()
            .filter_map(|output_id| self.output_target(snapshot, output_id))
            .collect()
    }

    fn output_target(&self, snapshot: &ProcessTreeSnapshot, output_id: NodeId) -> Option<UdpOutputTarget> {
        if !snapshot.node(output_id)?.enabled {
            return None;
        }

        let remote_host = child_string_param(snapshot, output_id, "remote_host")?;
        if remote_host.trim().is_empty() {
            return None;
        }

        let remote_port =
            child_int_param(snapshot, output_id, "remote_port").and_then(|value| u16::try_from(value).ok())?;

        Some(UdpOutputTarget {
            remote_host,
            remote_port,
        })
    }

    fn queue_send_request(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        request: &StreamingSendRequest,
    ) -> Result<String, String> {
        let outputs = self.collect_enabled_outputs(snapshot);
        if outputs.is_empty() {
            return Err("no enabled UDP outputs are configured".to_string());
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "UDP transport is not available".to_string())?;

        let mut queued = 0usize;
        let mut errors = Vec::new();
        for output in outputs {
            let packet = UdpStreamingOutboundPacket {
                bytes: request.bytes.clone(),
                remote_host: output.remote_host.clone(),
                remote_port: output.remote_port,
            };
            match transport.send(packet) {
                Ok(()) => {
                    queued = queued.saturating_add(1);
                    if self.stream.log_outgoing_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!(
                                "Sent UDP {} to {}:{}",
                                format_bytes_for_log(request.bytes.as_slice()),
                                output.remote_host,
                                output.remote_port
                            )
                        );
                    }
                }
                Err(error) => errors.push(error),
            }
        }

        if queued > 0 {
            self.stream.emit_outgoing_traffic(ctx);
        }

        if errors.is_empty() {
            Ok(format!("Queued UDP {} for {} output(s)", request.description, queued))
        } else {
            Err(errors.join("; "))
        }
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
            .map_err(|error| format!("invalid UDP command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, snapshot, &payload))
        {
            logerror!(format!(
                "Failed to handle UDP command {:?}: {error}",
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
        (self.network_interface.is_bound() && self.network_interface.id() == param)
            || (self.port.is_bound() && self.port.id() == param)
    }

    fn input_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        self.stream.connection_child_enabled(snapshot, UDP_INPUT_DECL_ID)
    }

    fn outputs_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        self.outputs.current_id().or_else(|| {
            let connection_id = self.stream.connection_id()?;
            snapshot.find_child_by_decl_id(connection_id, "outputs")
        })
    }

    fn outputs_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let outputs_id = self.outputs_node_id(snapshot)?;
        snapshot.node(outputs_id).map(|node| node.enabled)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.stream.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                self.input_enabled(snapshot).unwrap_or(false),
                self.outputs_enabled(snapshot).unwrap_or(false),
            ),
        );
    }

    fn set_input_warning(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, message: &str) {
        let Some(input_id) = self.stream.connection_child_node_id(snapshot, UDP_INPUT_DECL_ID) else {
            return;
        };
        NodeHandle::new(input_id).set_warning_with(ctx, Some(UDP_INPUT_WARNING_ID), message, None);
    }

    fn clear_input_warning(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(input_id) = self.stream.connection_child_node_id(snapshot, UDP_INPUT_DECL_ID) else {
            return;
        };
        NodeHandle::new(input_id).clear_warning(ctx, Some(UDP_INPUT_WARNING_ID));
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }
}

#[golden_core::item(
    "module",
    node = "udp_module",
    via = stream,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for UdpModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.stream.init(ctx);
        self.transport_dirty = true;

        self.refresh_interface_options(ctx);
        self.interface_refresh_elapsed = 0.0;

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);
        self.ensure_default_output(ctx, snapshot);
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
        self.interface_refresh_elapsed += ctx.delta_time.as_secs_f64();

        let refresh_interface_options = self.interface_refresh_due();
        let needs_snapshot = refresh_interface_options || self.transport_dirty || self.stream.has_pending_messages();
        if !needs_snapshot {
            return;
        }

        if refresh_interface_options {
            self.refresh_interface_options(ctx);
            self.interface_refresh_elapsed = 0.0;
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
        self.transport.is_some() || self.transport_dirty || self.stream.has_pending_messages() || self.interface_refresh_due()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(UDP_MODULE_UPDATE_RATE_HZ)
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
                self.clear_input_warning(ctx, snapshot_arc.as_ref());
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

fn collect_outputs_recursive(snapshot: &ProcessTreeSnapshot, parent: NodeId, output: &mut Vec<NodeId>) {
    for child_id in snapshot.child_ids(parent) {
        let Some(child_snapshot) = snapshot.node(child_id) else {
            continue;
        };

        if child_snapshot.node_type == UDP_OUTPUT_NODE_TYPE {
            output.push(child_id);
        } else if child_snapshot.node_type == "folder" {
            collect_outputs_recursive(snapshot, child_id, output);
        }
    }
}

#[cfg(test)]
mod tests;
