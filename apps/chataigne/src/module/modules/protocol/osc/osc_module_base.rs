use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

mod osc_input;
mod osc_input_delivery;
mod osc_input_queue;
mod osc_message;
use osc_input::InputPlan;
use osc_input_delivery::InputDelivery;
mod osc_outbound;
mod osc_outbound_work;
pub(crate) use osc_outbound::{
    dispatch as queue_osc_command, OutboundRequest as OscOutboundRequest, QueueError as OscQueueError,
};
use osc_outbound::{OutboundRequest, QueueError};
mod osc_runtime;
mod osc_value_helpers;
#[cfg(test)]
mod tests;
use osc_value_helpers::*;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeData, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::PendingDrainState;

use self::osc_runtime::{OscTransportConfig, OscTransportHandle, OscWorkerEvent};
use crate::app::module::ModuleDataCapabilities;

use self::osc_message::OscValuePayload;

const NETWORK_INTERFACE_WARNING_ID: &str = "network_interface_options";
const OSC_RECEIVER_WARNING_ID: &str = "receiver_transport";
const OSC_INTERFACE_REFRESH_INTERVAL_SECS: f64 = 1.0;
const OSC_MODULE_UPDATE_RATE_HZ: u32 = 120;
const OSC_COMPILED_KERNEL: &str = "chataigne.runtime.osc";
const OSC_MODULE_COMMAND_TYPES: &[&str] = &[crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE];
const OSC_SCRIPT_METHODS: &[&str] = &["sendMessage", "sendOSC", "sendOsc"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OscTransportBinding {
    pub interface_variant: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
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
        node outputs: crate::app::OscOutputManager = crate::app::OscOutputManager::create_with_module_authoring() (
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
    retiring_transport: Option<OscTransportHandle>,
    retry_at: Option<Instant>,
    transport_ready: bool,
    last_transport_config: Option<OscTransportConfig>,
    transport_dirty: bool,
    ignored_param_changes: HashMap<NodeId, usize>,
    pending_outbound_nodes: HashSet<NodeId>,
    outbound_retry_at: Option<Instant>,
    incoming: Option<InputDelivery>,
}

impl OscModuleBase {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::create_with_command_types(OSC_MODULE_COMMAND_TYPES),
            OSC_INTERFACE_REFRESH_INTERVAL_SECS,
            None,
            None,
            None,
            false,
            None,
            true,
            HashMap::new(),
            HashSet::new(),
            None,
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn has_pending_incoming_messages(&self) -> bool {
        self.incoming.is_some()
    }

    #[cfg(test)]
    pub(crate) fn auto_add_enabled(&self) -> bool {
        self.auto_add.get()
    }

    pub(crate) fn stop_transport(&mut self) {
        self.transport_ready = false;
        for mut transport in self.transport.take().into_iter().chain(self.retiring_transport.take()) {
            transport.stop();
        }
        self.retry_at = None;
        self.incoming = None;
        self.ignored_param_changes.clear();
    }

    fn retire_transport(&mut self) {
        self.transport_ready = false;
        if let Some(mut transport) = self.transport.take() {
            transport.finish_admission();
            self.retiring_transport = Some(transport);
        }
        self.last_transport_config = None;
    }

    fn retry_transport(&mut self) {
        self.transport_dirty = true;
        self.retry_at = Some(Instant::now() + Duration::from_millis(250));
    }

    fn transport_retry_due(&self) -> bool {
        self.retry_at.is_none_or(|at| Instant::now() >= at)
    }

    pub(crate) fn script_descriptor_for_node(&self, node_data: &NodeData, node_type: &str) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(node_data, node_type, OSC_SCRIPT_METHODS)
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
        if !self.transport_retry_due() {
            return;
        }
        self.retry_at = None;
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
                    self.base.set_connected(
                        ctx,
                        connected
                            && self.transport_ready
                            && self.transport.as_ref().is_some_and(|transport| transport.is_ready()),
                    );
                    return;
                }

                self.retire_transport();
                if self.retiring_transport.is_some() {
                    self.transport_dirty = true;
                    self.base.set_connected(ctx, false);
                    return;
                }

                match OscTransportHandle::spawn(
                    config.clone(),
                    InputPlan::new(self.id(), self.base.log_incoming_enabled()),
                ) {
                    Ok(handle) => {
                        self.transport = Some(handle);
                        self.last_transport_config = Some(config);
                        self.clear_receiver_warning(ctx, snapshot);
                        self.base.set_connected(ctx, false);
                    }
                    Err(error) => {
                        logerror!(format!(
                            "Failed to bind OSC at {}:{}, {}",
                            binding.interface_variant.as_str(),
                            binding.bind_port,
                            error
                        ));
                        self.transport = None;
                        self.last_transport_config = None;
                        if binding.receive_enabled {
                            self.set_receiver_warning(ctx, snapshot, error.as_str());
                        } else {
                            self.clear_receiver_warning(ctx, snapshot);
                        }
                        self.base.set_connected(ctx, false);
                        self.retry_transport();
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
        if let Some(transport) = &self.transport {
            transport.log_incoming(self.base.log_incoming_enabled());
            if self.transport_ready && !transport.is_ready() {
                self.base.set_connected(ctx, false);
            }
        }
        if self.incoming.is_some() {
            return;
        }
        let mut worker_events = Vec::with_capacity(1);
        let retiring = self.transport.is_none();
        let transport = if retiring {
            self.retiring_transport.as_ref()
        } else {
            self.transport.as_ref()
        };
        let Some(transport) = transport else { return };
        let mut disconnected = false;
        for _ in 0..osc_runtime::OSC_EVENT_DRAIN_BUDGET.get() {
            let turn = transport.drain_event(&mut worker_events);
            disconnected = turn.state == PendingDrainState::Disconnected;
            let Some(event) = worker_events.pop() else { break };
            match event {
                OscWorkerEvent::Ready(address) => {
                    if !retiring {
                        self.transport_ready = true;
                        NodeHandle::new(self.id()).clear_warning(ctx, Some(OSC_RECEIVER_WARNING_ID));
                        self.transport_dirty = true;
                        if self
                            .last_transport_config
                            .as_ref()
                            .is_some_and(|config| config.receive_enabled)
                        {
                            self.log_receiver_bound(&address.ip().to_string(), address.port());
                        }
                    }
                }
                OscWorkerEvent::Input(input) => {
                    if let Some(log) = &input.log {
                        golden_core::log!(origin = self.id(); log);
                    }
                    self.base.emit_incoming_traffic(ctx);
                    self.incoming = Some(InputDelivery::new(input));
                    break;
                }
                #[cfg(test)]
                OscWorkerEvent::Message(_) => unreachable!("module input is worker-prepared"),
                OscWorkerEvent::Sent(log) => {
                    self.base.emit_outgoing_traffic(ctx);
                    if let Some(message) = log {
                        golden_core::log!(origin = self.id(); message);
                    }
                }
                OscWorkerEvent::Error(error) => {
                    logerror!("OSC transport error: {}", error);
                }
                OscWorkerEvent::Failed(error) => {
                    self.transport_ready = false;
                    NodeHandle::new(self.id()).set_warning_with(ctx, Some(OSC_RECEIVER_WARNING_ID), &error, None);
                    self.base.set_connected(ctx, false);
                    logerror!("OSC transport failed: {}", error);
                }
            }
        }
        if disconnected && self.incoming.is_none() {
            if retiring {
                self.retiring_transport = None;
                self.transport_dirty = true;
                self.retry_at = None;
            } else {
                self.stop_transport();
                self.last_transport_config = None;
                self.base.set_connected(ctx, false);
                self.retry_transport();
            }
        }
    }

    fn flush_outbound_values(&mut self, ctx: &mut ProcessCtx, snapshot: &Arc<ProcessTreeSnapshot>) {
        if self.outbound_retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        self.outbound_retry_at = None;
        let Some(root) = self.base.values_id() else { return };
        for _ in 0..16 {
            let Some(target) = self.pending_outbound_nodes.iter().next().copied() else {
                break;
            };
            let result = ctx
                .node_waker()
                .map_err(|error| QueueError::Invalid(error.to_string()))
                .and_then(|producer| {
                    OutboundRequest::values(
                        snapshot,
                        self.id(),
                        root,
                        target,
                        producer,
                        self.base.log_outgoing_enabled(),
                    )
                })
                .and_then(|request| self.enqueue_outbound(request));
            match result {
                Ok(()) => {
                    self.pending_outbound_nodes.remove(&target);
                }
                Err(QueueError::Full) => {
                    self.outbound_retry_at = Some(Instant::now() + Duration::from_millis(250));
                    break;
                }
                Err(error) => {
                    self.pending_outbound_nodes.remove(&target);
                    logerror!(format!("OSC feedback was not admitted: {error}"));
                }
            }
        }
    }

    pub(crate) fn enqueue_outbound(&self, request: OutboundRequest) -> Result<(), QueueError> {
        if !self.node_data().effective_enabled {
            return Err(QueueError::Closed);
        }
        self.transport.as_ref().ok_or(QueueError::Closed)?.enqueue(request)
    }

    fn log_receiver_bound(&self, bind_host: &str, bind_port: u16) {
        golden_core::logsuccess!(
            origin = self.id();
            format!("Now receiving OSC on {}:{}", bind_host, bind_port)
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

        Some((|| {
            let snapshot = ctx.tree_snapshot_arc().ok_or("OSC send requires a tree snapshot")?;
            let args = ctx.capture_script_args(args).map_err(str::to_string)?;
            let request = OutboundRequest::script(
                &snapshot,
                self.id(),
                args,
                ctx.node_waker().map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            self.enqueue_outbound(request).map_err(|error| error.to_string())
        })())
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if let Some(count) = self.ignored_param_changes.get_mut(&param) {
            *count -= 1;
            if *count == 0 {
                self.ignored_param_changes.remove(&param);
            }
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

        if self.auto_feedback.get() {
            let Some(root) = self.base.values_id() else { return };
            let Ok(target) = osc_outbound::feedback_target(snapshot, param) else {
                return;
            };
            match is_descendant_of_node(snapshot, param, root) {
                Ok(true)
                    if self.pending_outbound_nodes.contains(&target) || self.pending_outbound_nodes.len() < 256 =>
                {
                    self.pending_outbound_nodes.insert(target);
                }
                Ok(true) => {
                    logerror!("OSC feedback exceeds 256 pending nodes; value was not admitted");
                }
                Ok(false) => {}
                Err(error) => {
                    logerror!(error);
                }
            }
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
    }

    fn on_node_ready(&mut self, _ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.transport_dirty = true;
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.drain_transport_events(ctx);
        if let Some(delivery) = self.incoming.as_mut() {
            let snapshot = ctx.tree_snapshot_arc();
            if delivery.service(
                ctx,
                snapshot.as_deref(),
                self.base.values_id(),
                self.auto_add.get(),
                &mut self.ignored_param_changes,
            ) {
                self.incoming = None;
                self.ignored_param_changes.clear();
            }
        }
        self.interface_refresh_elapsed += ctx.delta_time.as_secs_f64();

        let refresh_interface_options = self.interface_refresh_due();
        let needs_snapshot = refresh_interface_options
            || self.transport_dirty
            || (!self.pending_outbound_nodes.is_empty()
                && self.outbound_retry_at.is_none_or(|at| Instant::now() >= at));
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

        self.flush_outbound_values(ctx, &snapshot_arc);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
        self.incoming = None;
        self.pending_outbound_nodes.clear();
        self.outbound_retry_at = None;
        self.ignored_param_changes.clear();
    }

    fn needs_update(&self) -> bool {
        self.transport_dirty && self.retiring_transport.is_none() && self.transport_retry_due()
            || self
                .retiring_transport
                .as_ref()
                .is_some_and(|transport| transport.has_pending())
            || (!self.pending_outbound_nodes.is_empty() && self.outbound_retry_at.is_none_or(|at| Instant::now() >= at))
            || self.incoming.is_some()
            || self.transport.as_ref().is_some_and(|t| t.has_pending())
            || self.interface_refresh_elapsed >= OSC_INTERFACE_REFRESH_INTERVAL_SECS
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        // transport.is_some() intentionally excluded: drain_transport_events() is
        // cheap and safe without a snapshot. Messages from this tick's drain land in
        // pending_incoming_messages and will be processed next tick when that flag
        // triggers the snapshot. Removing it avoids a build_process_tree_snapshot()
        // call (O(N_total_nodes)) every tick for every idle-but-connected OSC module.
        self.interface_refresh_due()
            || self.transport_dirty && self.retiring_transport.is_none() && self.transport_retry_due()
            || self
                .retiring_transport
                .as_ref()
                .is_some_and(|transport| transport.has_pending())
            || (!self.pending_outbound_nodes.is_empty() && self.outbound_retry_at.is_none_or(|at| Instant::now() >= at))
            || self.incoming.as_ref().is_some_and(InputDelivery::requires_snapshot)
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(OSC_MODULE_UPDATE_RATE_HZ).with_compiled_kernel(OSC_COMPILED_KERNEL)
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
        self.base.on_custom_event(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}
