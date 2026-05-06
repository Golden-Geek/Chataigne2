use std::collections::HashSet;

mod osc_message;
mod osc_runtime;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeData, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use self::osc_runtime::{OscOutboundMessage, OscTransportConfig, OscTransportHandle, OscWorkerEvent};
use crate::app::module::ModuleDataCapabilities;

pub(crate) use self::osc_message::{OscDecodedMessage, OscValuePayload};

const NETWORK_INTERFACE_WARNING_ID: &str = "network_interface_options";
const OSC_RECEIVER_WARNING_ID: &str = "receiver_transport";
const OSC_INTERFACE_REFRESH_INTERVAL_SECS: f64 = 1.0;
const OSC_MODULE_UPDATE_RATE_HZ: u32 = 120;
const OSC_OUTPUT_NODE_TYPE: &str = "osc_output";
const OSC_MODULE_COMMAND_TYPES: &[&str] = &[crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE];
const VALUE_LABEL_PREFIX: &str = "value ";
const OSC_MESSAGE_RECEIVED_CALLBACK: &str = "messageReceived";
const OSC_SCRIPT_METHODS: &[&str] = &["sendMessage", "sendOSC", "sendOsc"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OscTransportBinding {
    pub interface_variant: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
}

pub(crate) enum OscIncomingApplyResult {
    Applied,
    Retry,
    Ignored,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OscOutputTarget {
    remote_host: String,
    remote_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OscQueueError {
    queued: usize,
    message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) struct OscSendCustomMessageRequest {
    pub address: String,
    pub arguments: Vec<ParamValue>,
}

#[node("osc_module_base", label = "OSC Module")]
#[children(
    folder(connection) {
        network_interface: Enum = "any" (
            label = "Network Interface",
            description = "Network interface used to receive OSC and as the source binding for outgoing traffic.",
            enum_options = ["any (default)"]
        );
        folder(input, label = "Input", can_be_disabled = true) {
            port: i32 = 9000 [0..65535] (
                label = "Port",
                description = "UDP port used to receive OSC messages when the receiver is enabled.",
                widget = "text"
            );
        }
        node outputs: crate::app::OscOutputManager = crate::app::OscOutputManager::new() (
            label = "Outputs",
            description = "OSC destinations used by this module for outgoing traffic.",
            can_be_disabled = true
        );
    }
    folder(parameters) {
        folder(processing, label = "Processing") {
            auto_add: bool = true (
                label = "Auto Add",
                description = "Automatically create missing OSC value nodes from incoming addresses."
            );
            auto_feedback: bool = false (
                label = "Auto Feedback",
                description = "Only send OSC when data in values changes if auto feedback is checked."
            );
        }
    }
)]

pub struct OscModuleBase {
    base: crate::app::ModuleBase,
    interface_refresh_elapsed: f64,
    transport: Option<OscTransportHandle>,
    last_transport_config: Option<OscTransportConfig>,
    transport_dirty: bool,
    ignored_param_changes: HashSet<NodeId>,
    pending_outbound_nodes: HashSet<NodeId>,
    pending_incoming_messages: Vec<OscDecodedMessage>,
}

impl OscModuleBase {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            OSC_INTERFACE_REFRESH_INTERVAL_SECS,
            None,
            None,
            true,
            HashSet::new(),
            HashSet::new(),
            Vec::new(),
        )
    }

    pub(crate) fn process_pending_incoming<F>(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        mut apply_message: F,
    ) -> bool
    where
        F: FnMut(&mut Self, &mut ProcessCtx, &ProcessTreeSnapshot, &OscDecodedMessage) -> OscIncomingApplyResult,
    {
        if self.pending_incoming_messages.is_empty() {
            return false;
        }

        let mut remaining = Vec::new();
        let mut messages = std::mem::take(&mut self.pending_incoming_messages).into_iter();

        while let Some(message) = messages.next() {
            match apply_message(self, ctx, snapshot, &message) {
                OscIncomingApplyResult::Applied | OscIncomingApplyResult::Ignored => {
                    self.emit_osc_message_received_callback(ctx, &message);
                }
                OscIncomingApplyResult::Retry => {
                    remaining.push(message);
                    remaining.extend(messages);
                    self.pending_incoming_messages = remaining;
                    return true;
                }
            }
        }

        self.pending_incoming_messages = remaining;
        false
    }

    pub(crate) fn enqueue_incoming_message(&mut self, message: OscDecodedMessage) {
        self.log_incoming_message(&message);
        self.pending_incoming_messages.push(message);
    }

    pub(crate) fn has_pending_incoming_messages(&self) -> bool {
        !self.pending_incoming_messages.is_empty()
    }

    pub(crate) fn auto_add_enabled(&self) -> bool {
        self.auto_add.get()
    }

    pub(crate) fn values_id(&self) -> Option<NodeId> {
        self.base.values_id()
    }

    pub(crate) fn set_internal_param(&mut self, ctx: &mut ProcessCtx, param_id: NodeId, value: ParamValue) {
        self.ignored_param_changes.insert(param_id);
        ctx.set_param(param_id, value);
    }

    pub(crate) fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }

    pub(crate) fn script_descriptor_for_node(&self, node_data: &NodeData, node_type: &str) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(node_data, node_type, OSC_SCRIPT_METHODS)
    }

    #[cfg(test)]
    pub(crate) fn set_transport_dirty(&mut self, transport_dirty: bool) {
        self.transport_dirty = transport_dirty;
    }

    fn interface_refresh_due(&self) -> bool {
        self.interface_refresh_elapsed >= OSC_INTERFACE_REFRESH_INTERVAL_SECS
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
                    .clear_warning(ctx, Some(NETWORK_INTERFACE_WARNING_ID));
            }
            Err(error) => {
                self.network_interface
                    .set_warning_with(ctx, Some(NETWORK_INTERFACE_WARNING_ID), error.as_str(), None);
            }
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
            self.clear_receiver_warning(ctx, snapshot);
            self.base.set_connected(ctx, false);
            return;
        }

        match self.transport_binding(snapshot) {
            Ok(binding) => {
                let connected = self.connected_for_binding(snapshot, &binding);
                let config = OscTransportConfig {
                    bind_interface_host:
                        crate::app::module::common::network_interfaces::bind_host_for_interface_variant(
                            binding.interface_variant.as_str(),
                        ),
                    bind_port: binding.bind_port,
                    receive_enabled: binding.receive_enabled,
                };

                if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
                    self.clear_receiver_warning(ctx, snapshot);
                    self.base.set_connected(ctx, connected);
                    return;
                }

                self.stop_transport();

                match OscTransportHandle::spawn(config.clone()) {
                    Ok(handle) => {
                        self.transport = Some(handle);
                        self.last_transport_config = Some(config);
                        self.clear_receiver_warning(ctx, snapshot);
                        self.base.set_connected(ctx, connected);
                        if binding.receive_enabled {
                            self.log_receiver_bound(
                                crate::app::module::common::network_interfaces::bind_host_for_interface_variant(
                                    binding.interface_variant.as_str(),
                                )
                                .as_str(),
                                binding.bind_port,
                            );
                        }
                    }
                    Err(error) => {
                        logerror!(format!("Failed to bind OSC at {}:{}, {}", binding.interface_variant.as_str(), binding.bind_port, error));
                        self.transport = None;
                        self.last_transport_config = None;
                        if binding.receive_enabled {
                            self.set_receiver_warning(ctx, snapshot, error.as_str());
                        } else {
                            self.clear_receiver_warning(ctx, snapshot);
                        }
                        self.base.set_connected(ctx, false);
                    }
                }
            }
            Err(error) => {
                logerror!("Invalid OSC transport configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                if self.receiver_enabled(snapshot).unwrap_or(false) {
                    self.set_receiver_warning(ctx, snapshot, error.as_str());
                } else {
                    self.clear_receiver_warning(ctx, snapshot);
                }
                self.base.set_connected(ctx, false);
            }
        }
    }

    fn transport_binding(&self, snapshot: &ProcessTreeSnapshot) -> Result<OscTransportBinding, String> {
        let connection_id = self
            .base
            .connection_id()
            .ok_or_else(|| "missing OSC connection folder 'connection'".to_string())?;
        let receiver_id = snapshot
            .find_child_by_decl_id(connection_id, "input")
            .ok_or_else(|| "missing OSC receiver folder 'connection/input'".to_string())?;
        let receive_enabled = snapshot
            .node(receiver_id)
            .map(|node| node.enabled)
            .ok_or_else(|| "missing OSC receiver folder 'connection/input'".to_string())?;

        let bind_port = if receive_enabled {
            u16::try_from(self.port.get())
                .map_err(|_| "OSC port 'parameters/receiver/port' must be between 0 and 65535".to_string())?
        } else {
            0
        };

        Ok(OscTransportBinding {
            interface_variant: self.network_interface.get_ref().as_str().to_string(),
            bind_port,
            receive_enabled,
        })
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        let mut worker_events = Vec::new();
        let Some(transport) = &self.transport else {
            return;
        };

        while let Ok(event) = transport.try_recv() {
            worker_events.push(event);
        }

        let mut received_message = false;
        for event in worker_events {
            match event {
                OscWorkerEvent::Message(message) => {
                    received_message = true;
                    self.enqueue_incoming_message(message);
                }
                OscWorkerEvent::Error(error) => {
                    logerror!("OSC transport error: {}", error);
                }
            }
        }

        if received_message {
            self.base.emit_incoming_traffic(ctx);
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

        ctx.add_user_item_boxed(outputs_id, Box::new(crate::app::OscOutput::new()), None);
    }

    fn flush_outbound_values(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        if self.pending_outbound_nodes.is_empty() {
            return;
        }

        let outputs = self.collect_enabled_outputs(snapshot);
        if outputs.is_empty() {
            self.pending_outbound_nodes.clear();
            return;
        }

        let changed_nodes = std::mem::take(&mut self.pending_outbound_nodes);
        let target_nodes: HashSet<NodeId> = changed_nodes
            .into_iter()
            .filter_map(|node_id| resolve_outgoing_target(snapshot, node_id))
            .collect();

        let Some(values_id) = self.base.values_id() else {
            return;
        };

        let mut sent_message = false;
        for target_id in target_nodes {
            let Some(address) = outgoing_address_for_values_node(snapshot, values_id, target_id) else {
                continue;
            };
            let Some(payload) = outgoing_payload_for_target(snapshot, target_id) else {
                continue;
            };

            match self.queue_message_for_outputs(outputs.as_slice(), address.as_str(), &payload) {
                Ok(queued) => {
                    sent_message = sent_message || queued > 0;
                }
                Err(error) => {
                    sent_message = sent_message || error.queued > 0;
                    logerror!("Failed to send OSC message {} - {}", address, error.message);
                }
            }
        }

        if sent_message {
            self.base.emit_outgoing_traffic(ctx);
        }
    }

    fn collect_enabled_outputs(&self, snapshot: &ProcessTreeSnapshot) -> Vec<OscOutputTarget> {
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

    fn output_target(&self, snapshot: &ProcessTreeSnapshot, output_id: NodeId) -> Option<OscOutputTarget> {
        if !snapshot.node(output_id)?.enabled {
            return None;
        }

        let remote_host = child_string_param(snapshot, output_id, "remote_host")?;
        if remote_host.trim().is_empty() {
            return None;
        }

        let remote_port =
            child_int_param(snapshot, output_id, "remote_port").and_then(|value| u16::try_from(value).ok())?;

        Some(OscOutputTarget {
            remote_host,
            remote_port,
        })
    }

    fn log_incoming_message(&self, message: &OscDecodedMessage) {
        if !self.base.log_incoming_enabled() {
            return;
        }

        golden_core::log!(
            origin = self.id();
            format!("Received OSC {} -> {}", message.address, format_osc_payload(&message.payload))
        );
    }

    fn log_receiver_bound(&self, bind_host: &str, bind_port: u16) {
        golden_core::logsuccess!(
            origin = self.id();
            format!("Now receiving OSC on {}:{}", bind_host, bind_port)
        );
    }

    fn log_outgoing_message(&self, address: &str, payload: &OscValuePayload, remote_host: &str, remote_port: u16) {
        if !self.base.log_outgoing_enabled() {
            return;
        }

        golden_core::log!(
            origin = self.id();
            format!(
                "Sent OSC {} -> {}:{} ({})",
                address,
                remote_host,
                remote_port,
                format_osc_payload(payload)
            )
        );
    }

    fn queue_message_for_outputs(
        &self,
        outputs: &[OscOutputTarget],
        address: &str,
        payload: &OscValuePayload,
    ) -> Result<usize, OscQueueError> {
        if outputs.is_empty() {
            return Err(OscQueueError {
                queued: 0,
                message: "no enabled OSC outputs are configured".to_string(),
            });
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| OscQueueError {
                queued: 0,
                message: "OSC transport is not available".to_string(),
            })?;
        self::osc_message::encode_packet(address, payload)
            .map_err(|error| OscQueueError {
                queued: 0,
                message: format!("cannot encode OSC message '{}': {error}", address.trim()),
            })?;

        let mut queued = 0usize;
        let mut errors = Vec::new();

        for output in outputs {
            let message = OscOutboundMessage {
                address: address.to_string(),
                payload: payload.clone(),
                remote_host: output.remote_host.clone(),
                remote_port: output.remote_port,
            };

            match transport.send(message) {
                Ok(()) => {
                    queued = queued.saturating_add(1);
                    self.log_outgoing_message(address, payload, output.remote_host.as_str(), output.remote_port);
                }
                Err(error) => {
                    errors.push(format!("{}:{} - {}", output.remote_host, output.remote_port, error));
                }
            }
        }

        if errors.is_empty() {
            return Ok(queued);
        }

        if queued == 0 {
            return Err(OscQueueError {
                queued,
                message: errors.join("; "),
            });
        }

        Err(OscQueueError {
            queued,
            message: format!(
                "queued {} output(s), but {} output(s) failed: {}",
                queued,
                errors.len(),
                errors.join("; ")
            ),
        })
    }

    fn queue_custom_message(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        request: &OscSendCustomMessageRequest,
    ) -> Result<String, String> {
        let outputs = self.collect_enabled_outputs(snapshot);
        let payload = OscValuePayload::Arguments(request.arguments.clone());
        let queued = match self.queue_message_for_outputs(outputs.as_slice(), request.address.as_str(), &payload) {
            Ok(queued) => queued,
            Err(error) => {
                if error.queued > 0 {
                    self.base.emit_outgoing_traffic(ctx);
                }
                return Err(error.message);
            }
        };
        if queued > 0 {
            self.base.emit_outgoing_traffic(ctx);
        }
        Ok(format!("Queued OSC {} for {} output(s)", request.address, queued))
    }

    fn emit_osc_message_received_callback(&self, ctx: &mut ProcessCtx, message: &OscDecodedMessage) {
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            OSC_MESSAGE_RECEIVED_CALLBACK,
            vec![
                serde_json::json!(message.address.as_str()),
                osc_payload_script_arg(&message.payload),
                serde_json::json!({
                    "address": message.address.as_str(),
                    "payload": osc_payload_script_arg(&message.payload),
                }),
            ],
        );
    }

    fn handle_script_send_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        match method {
            "sendMessage" | "sendOSC" | "sendOsc" => {}
            _ => return None,
        }

        let Some(address) = args.first().and_then(ParamValue::as_str) else {
            return Some(Err(format!("method '{method}' expects an OSC address string")));
        };
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return Some(Err(format!("method '{method}' is unavailable without a tree snapshot")));
        };
        let request = OscSendCustomMessageRequest {
            address,
            arguments: args.iter().skip(1).cloned().collect(),
        };
        Some(self.queue_custom_message(ctx, snapshot_arc.as_ref(), &request).map(|_| ()))
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !OSC_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str()) {
            return;
        }
        let command_id = request.command_id;
        let payload = request.payload;

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if let Err(error) = serde_json::from_value::<OscSendCustomMessageRequest>(payload)
            .map_err(|error| format!("invalid OSC command payload: {error}"))
            .and_then(|payload| self.queue_custom_message(ctx, snapshot, &payload))
        {
            logerror!(format!("Failed to handle OSC command {:?}: {error}", command_id));
        }
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if self.ignored_param_changes.remove(&param) {
            return;
        }

        if self.param_affects_transport(param) {
            self.transport_dirty = true;
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if self.auto_feedback.get()
            && self
                .base
                .values_id()
                .is_some_and(|values_id| is_descendant_of_node(snapshot, param, values_id))
        {
            self.pending_outbound_nodes.insert(param);
        }
    }

    fn on_meta_changed_inner(&mut self, _ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if let Some(enabled) = patch.enabled {
            if node != self.id() {
                let _ = enabled;
                self.transport_dirty = true;
            }
        }
    }

    fn on_effective_enabled_changed_inner(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.transport_dirty = true;
        } else {
            self.stop_transport();
            self.last_transport_config = None;
            if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
                self.clear_receiver_warning(ctx, snapshot_arc.as_ref());
            }
            self.base.set_connected(ctx, false);
            self.transport_dirty = false;
        }
    }

    fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.network_interface.is_bound() && self.network_interface.id() == param)
            || (self.port.is_bound() && self.port.id() == param)
    }

    fn receiver_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let connection_id = self.base.connection_id()?;
        snapshot.find_child_by_decl_id(connection_id, "input")
    }

    fn receiver_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let receiver_id = self.receiver_node_id(snapshot)?;
        snapshot.node(receiver_id).map(|node| node.enabled)
    }

    fn outputs_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        self.outputs.current_id().or_else(|| {
            let connection_id = self.base.connection_id()?;
            snapshot.find_child_by_decl_id(connection_id, "outputs")
        })
    }

    fn outputs_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let outputs_id = self.outputs_node_id(snapshot)?;
        snapshot.node(outputs_id).map(|node| node.enabled)
    }

    fn data_capabilities(&self, snapshot: &ProcessTreeSnapshot) -> ModuleDataCapabilities {
        ModuleDataCapabilities::new(
            self.receiver_enabled(snapshot).unwrap_or(false),
            self.outputs_enabled(snapshot).unwrap_or(false),
        )
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.base.set_data_capabilities(ctx, self.data_capabilities(snapshot));
    }

    fn connected_for_binding(&self, snapshot: &ProcessTreeSnapshot, binding: &OscTransportBinding) -> bool {
        binding.receive_enabled || self.outputs_enabled(snapshot).unwrap_or(false)
    }

    fn set_receiver_warning(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, message: &str) {
        let Some(receiver_id) = self.receiver_node_id(snapshot) else {
            return;
        };
        NodeHandle::new(receiver_id).set_warning_with(ctx, Some(OSC_RECEIVER_WARNING_ID), message, None);
    }

    fn clear_receiver_warning(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(receiver_id) = self.receiver_node_id(snapshot) else {
            return;
        };
        NodeHandle::new(receiver_id).clear_warning(ctx, Some(OSC_RECEIVER_WARNING_ID));
    }
}

fn osc_payload_script_arg(payload: &OscValuePayload) -> serde_json::Value {
    match payload {
        OscValuePayload::Single(value) => value.to_script_json(),
        OscValuePayload::Multi(values) | OscValuePayload::Arguments(values) => {
            crate::app::module::script_api::param_values_arg(values.as_slice())
        }
    }
}

#[node("osc_module_base", via = base, from_struct)]
impl Node for OscModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, OSC_MODULE_COMMAND_TYPES);
        self.transport_dirty = true;
        crate::app::module::enable_module_authoring(self.node_data_mut());

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
        self.drain_transport_events(ctx);
        self.interface_refresh_elapsed += ctx.delta_time.as_secs_f64();

        let refresh_interface_options = self.interface_refresh_due();
        let needs_snapshot = refresh_interface_options || self.transport_dirty || !self.pending_outbound_nodes.is_empty();
        if !needs_snapshot {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if refresh_interface_options {
            self.refresh_interface_options(ctx);
            self.interface_refresh_elapsed = 0.0;
        }

        self.refresh_data_capabilities(ctx, snapshot);

        if self.transport_dirty {
            self.refresh_transport(ctx, snapshot);
        }

        self.flush_outbound_values(ctx, snapshot);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(OSC_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        self.script_descriptor_for_node(self.node_data(), self.get_type())
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.handle_script_send_method(ctx, method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot_arc.as_ref(), param, &_old_value);
        }
        self.on_param_change_inner(ctx, param);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        self.on_meta_changed_inner(ctx, node, patch);
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.on_effective_enabled_changed_inner(ctx, enabled);
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

        if child_snapshot.node_type == OSC_OUTPUT_NODE_TYPE {
            output.push(child_id);
        } else if child_snapshot.node_type == "folder" {
            collect_outputs_recursive(snapshot, child_id, output);
        }
    }
}

fn resolve_outgoing_target(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> Option<NodeId> {
    let node = snapshot.node(node_id)?;
    if node.param_value.is_none() {
        return None;
    }

    let parent_id = node.parent?;
    let parent = snapshot.node(parent_id)?;
    if parent.node_type == "folder"
        && indexed_value_label_index(node.label.as_str()).is_some()
        && has_indexed_value_children(snapshot, parent_id)
    {
        return Some(parent_id);
    }

    Some(node_id)
}

fn outgoing_address_for_values_node(
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    target_id: NodeId,
) -> Option<String> {
    let mut segments = Vec::new();
    let mut current = Some(target_id);

    while let Some(node_id) = current {
        if node_id == values_id {
            break;
        }

        let node = snapshot.node(node_id)?;
        let segment = node.label.trim().trim_matches('/');
        if !segment.is_empty() {
            segments.push(segment.to_string());
        }
        current = node.parent;
    }

    if current != Some(values_id) || segments.is_empty() {
        return None;
    }

    segments.reverse();
    Some(format!("/{}", segments.join("/")))
}

fn outgoing_payload_for_target(snapshot: &ProcessTreeSnapshot, target_id: NodeId) -> Option<OscValuePayload> {
    let target = snapshot.node(target_id)?;
    if let Some(value) = target.param_value.clone() {
        return Some(OscValuePayload::Single(value));
    }

    if target.node_type != "folder" {
        return None;
    }

    let mut indexed_values = snapshot
        .child_ids(target_id)
        .into_iter()
        .filter_map(|child_id| {
            let child = snapshot.node(child_id)?;
            let index = indexed_value_label_index(child.label.as_str())?;
            let value = child.param_value.clone()?;
            Some((index, value))
        })
        .collect::<Vec<_>>();
    indexed_values.sort_by_key(|(index, _)| *index);

    if indexed_values.is_empty() {
        return None;
    }

    Some(OscValuePayload::Multi(
        indexed_values.into_iter().map(|(_, value)| value).collect(),
    ))
}

fn child_string_param(snapshot: &ProcessTreeSnapshot, parent: NodeId, child_name: &str) -> Option<String> {
    snapshot.find_child_by_decl_id(parent, child_name).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

fn child_int_param(snapshot: &ProcessTreeSnapshot, parent: NodeId, child_name: &str) -> Option<i32> {
    snapshot.find_child_by_decl_id(parent, child_name).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_int)
    })
}

fn format_osc_payload(payload: &OscValuePayload) -> String {
    match payload {
        OscValuePayload::Single(value) => format_param_value(value),
        OscValuePayload::Multi(values) => format!(
            "[{}]",
            values.iter().map(format_param_value).collect::<Vec<_>>().join(", ")
        ),
        OscValuePayload::Arguments(values) => format!(
            "[{}]",
            values.iter().map(format_param_value).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn format_param_value(value: &ParamValue) -> String {
    match value {
        ParamValue::Trigger() => "trigger".to_string(),
        ParamValue::Int(value) => value.to_string(),
        ParamValue::Float(value) => value.to_string(),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => value.clone(),
        ParamValue::Bool(value) => value.to_string(),
        ParamValue::Vec2(x, y) => format!("({}, {})", x, y),
        ParamValue::Vec3(x, y, z) => format!("({}, {}, {})", x, y, z),
        ParamValue::Color(r, g, b, a) => format!("rgba({}, {}, {}, {})", r, g, b, a),
        ParamValue::CssValue(value) => format!("{value:?}"),
        ParamValue::Reference(reference) => format!("{reference:?}"),
    }
}

fn indexed_value_label_index(label: &str) -> Option<usize> {
    let suffix = label.strip_prefix(VALUE_LABEL_PREFIX)?;
    suffix.parse::<usize>().ok()?.checked_sub(1)
}

fn has_indexed_value_children(snapshot: &ProcessTreeSnapshot, folder_id: NodeId) -> bool {
    snapshot.child_ids(folder_id).into_iter().any(|child_id| {
        snapshot
            .node(child_id)
            .is_some_and(|child| indexed_value_label_index(child.label.as_str()).is_some())
    })
}

fn is_descendant_of_node(snapshot: &ProcessTreeSnapshot, start: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(start);
    while let Some(node_id) = current {
        if node_id == ancestor {
            return true;
        }
        current = snapshot.node(node_id).and_then(|node| node.parent);
    }

    false
}
