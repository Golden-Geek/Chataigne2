mod delivery;
mod input;
mod lifecycle;
mod transport;
mod wakeup;

use delivery::InputDelivery;
use std::{collections::VecDeque, time::Duration};
use wakeup::UdpWake;

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

use self::lifecycle::{UdpTransportEvent, UdpTransports};
use self::transport::{UdpStreamingOutboundPacket, UdpStreamingTransportConfig};

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
    wake: Option<UdpWake>,
    transports: UdpTransports,
    transport_dirty: bool,
    pending_inputs: VecDeque<InputDelivery>,
    input_target: Option<NodeId>,
}

impl UdpModule {
    pub fn create() -> Self {
        Self::new(
            StreamingModuleBase::create(),
            None,
            UdpTransports::default(),
            true,
            VecDeque::new(),
            None,
        )
    }

    fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_interface_options(
        &self,
        ctx: &mut ProcessCtx,
        options: Result<Vec<golden_core::parameter::ParameterEnumOption>, String>,
    ) {
        if !self.network_interface.is_bound() {
            return;
        }

        match options {
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
        self.stream.refresh_config_cache(snapshot);
        self.input_target = self.stream.connection_child_node_id(snapshot, UDP_INPUT_DECL_ID);

        if !self.module_enabled(snapshot) {
            self.transports.pause_admission();
            self.clear_input_warning(ctx, snapshot);
            self.stream.set_connected(ctx, false);
            return;
        }

        let binding = match self.transport_binding(snapshot) {
            Ok(binding) => binding,
            Err(error) => {
                logerror!("Invalid UDP module configuration: {}", error);
                self.transports.pause_admission();
                self.set_input_warning(ctx, snapshot, error.as_str());
                self.stream.set_connected(ctx, false);
                return;
            }
        };

        if !binding.receive_enabled && !binding.send_enabled {
            self.transports.pause_admission();
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
            input: input::UdpInputConfig {
                module: self.id(),
                parse: self.stream.parse_config_cached(),
                processing_enabled: self.stream.processing_enabled_cached(),
                log_incoming: self.stream.log_incoming_enabled(),
            },
        };

        let Some(wake) = &self.wake else {
            return;
        };
        match self.transports.configure(config, wake.notifier.clone()) {
            Ok(configured) => {
                self.transport_dirty = !configured;
                self.clear_input_warning(ctx, snapshot);
                self.stream.set_connected(ctx, self.transports.connected());
            }
            Err(error) => {
                logerror!("Failed to start UDP transport: {}", error);
                self.transport_dirty = true;
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

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        if !self.pending_inputs.is_empty() {
            return;
        }
        let poll = self.transports.poll();

        let mut received_bytes = false;
        for event in poll.events {
            match event {
                UdpTransportEvent::Input(input) => {
                    received_bytes = true;
                    if let Some(message) = &input.log_message {
                        golden_core::log!(origin = self.id(); message.as_str());
                    }
                    if let Err(error) = &input.values {
                        logerror!("Failed to parse UDP input: {}", error);
                    }
                    if input.rejected_lines > 0 {
                        logerror!(
                            "Rejected {} UDP lines at an input/configuration admission boundary",
                            input.rejected_lines
                        );
                    }
                    self.pending_inputs.push_back(InputDelivery::new(input));
                }
                UdpTransportEvent::Interfaces(options) => self.refresh_interface_options(ctx, options),
                UdpTransportEvent::Error(error) => {
                    logerror!("UDP transport error: {}", error);
                }
            }
        }

        if received_bytes {
            self.stream.emit_incoming_traffic(ctx);
        }

        self.stream.set_connected(ctx, self.transports.connected());
        if let Some(error) = poll.failure {
            logerror!("UDP transport failed: {}", error);
            if let Some(input) = self.input_target {
                NodeHandle::new(input).set_warning_with(ctx, Some(UDP_INPUT_WARNING_ID), error, None);
            }
        }
        if poll.disconnected {
            self.transport_dirty = true;
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

        let mut queued = 0usize;
        let mut errors = Vec::new();
        for output in outputs {
            let packet = UdpStreamingOutboundPacket {
                bytes: request.bytes.clone(),
                remote_host: output.remote_host.clone(),
                remote_port: output.remote_port,
            };
            match self.transports.send(packet) {
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
        if self.param_affects_transport(param) || self.stream.affects_input_configuration(param) {
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
        self.transports.cancel_all();
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

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);
        self.ensure_default_output(ctx, snapshot);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let source = match ctx.node_waker() {
            Ok(source) => source,
            Err(error) => {
                logerror!("Failed to register UDP worker wake: {}", error);
                return;
            }
        };
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.wake = None;
            self.stop_transport();
            self.pending_inputs.clear();
            self.stream.discard_pending_input();
            match UdpWake::new(source) {
                Ok(wake) => self.wake = Some(wake),
                Err(error) => {
                    logerror!("Failed to register UDP retry deadline: {}", error);
                    return;
                }
            }
        }
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
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
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.stream.refresh_config_cache(&snapshot);
            self.refresh_data_capabilities(ctx, &snapshot);
            if self.transport_dirty {
                self.refresh_transport(ctx, &snapshot);
            }
            self.stream.process_pending(ctx, &snapshot);
        }
        if self.stream.has_pending_messages() || self.transports.has_pending() || !self.pending_inputs.is_empty() {
            self.request_service();
        }
        if self.transport_dirty && !self.transports.waiting_for_retirement() {
            if let Some(wake) = &self.wake {
                wake.later(self.transports.retry_in().unwrap_or(Duration::from_millis(1)));
            }
        }
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
        self.wake = None;
        self.pending_inputs.clear();
        self.stream.discard_pending_input();
        self.input_target = None;
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.transport_dirty || self.stream.has_pending_messages()
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
                self.request_service();
            }
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.transport_dirty = true;
            self.request_service();
        } else {
            self.transports.pause_admission();
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
