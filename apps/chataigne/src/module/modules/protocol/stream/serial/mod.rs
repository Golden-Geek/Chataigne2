mod projection;

use crate::app::module::common::{
    streaming::{delivery::InputDelivery, input::StreamInputConfig},
    worker_wake::ModuleWake,
};
use projection::SerialProjection;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::PendingDrainState;

use crate::app::{
    module::common::{
        serial::{
            serial_port_name_for_variant, SerialConnectionConfig, SerialConnectionEvent, SerialConnectionHandle,
            SerialConnectionStatus, SerialDiscoveryRegistration, NO_SERIAL_PORT_VARIANT,
        },
        streaming::{
            commands::{streaming_script_send_request, StreamingSendRequest},
            script as streaming_script,
        },
    },
    StreamingModuleBase,
};

const SERIAL_PORT_OPTIONS_WARNING_ID: &str = "serial_port_options";
const SERIAL_PORT_CONNECTION_WARNING_ID: &str = "serial_port_connection";

#[node("serial_module", label = "Serial")]
#[children(
    folder(connection) {
        port_name: Enum = NO_SERIAL_PORT_VARIANT (
            label = "Port",
            description = "Serial port to connect to. Ports are detected automatically and labeled for this OS.",
            enum_options = ["none (No Port)"]
        );
        baud_rate: i32 = 115200 [1..2147483647] (
            label = "Baud Rate",
            description = "Serial baud rate.",
            widget = "text"
        );
        dtr: bool = false (
            label = "DTR",
            description = "Data Terminal Ready."
        );
        rts: bool = false (
            label = "RTS",
            description = "Request To Send."
        );
        [base_children];
    }
)]
pub struct SerialModule {
    stream: StreamingModuleBase,
    port_discovery: Option<SerialDiscoveryRegistration>,
    projection: SerialProjection,
    wake: Option<ModuleWake>,
    pending_inputs: VecDeque<InputDelivery>,
    retiring_transport: Option<SerialConnectionHandle>,
    transport: Option<SerialConnectionHandle>,
    last_transport_config: Option<SerialConnectionConfig>,
    transport_dirty: bool,
    retry_at: Option<Instant>,
    discovery_retry_at: Option<Instant>,
}

impl SerialModule {
    pub fn create() -> Self {
        Self::new(
            StreamingModuleBase::create(),
            None,
            SerialProjection::default(),
            None,
            VecDeque::new(),
            None,
            None,
            None,
            true,
            None,
            None,
        )
    }
    fn request_service(&self) {
        if let Some(wake) = &self.wake {
            wake.notifier.notify();
        }
    }
    fn ensure_port_discovery_registration(&mut self) {
        if self.port_discovery.is_none() && self.discovery_retry_at.is_none_or(|at| Instant::now() >= at) {
            let Some(wake) = &self.wake else { return };
            match SerialDiscoveryRegistration::register(wake.notifier.clone()) {
                Ok(registration) => {
                    self.port_discovery = Some(registration);
                    self.projection.registration_error(None);
                    self.discovery_retry_at = None;
                }
                Err(error) => {
                    self.projection.registration_error(Some(error));
                    self.discovery_retry_at = Some(Instant::now() + Duration::from_secs(1));
                }
            }
        }
        if let Some(registration) = &self.port_discovery {
            self.projection.catalog(registration.snapshot());
        }
    }
    fn retire_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            debug_assert!(self.retiring_transport.is_none());
            transport.finish_admission();
            self.retiring_transport = Some(transport);
        }
        self.last_transport_config = None;
        self.projection.connected(false);
    }
    fn arm_service(&self) {
        let Some(wake) = &self.wake else { return };
        if self.projection.pending()
            || self.projection.needs_snapshot()
            || self.stream.has_pending_messages()
            || !self.pending_inputs.is_empty()
            || self.transport.as_ref().is_some_and(|t| t.has_pending())
            || self.retiring_transport.as_ref().is_some_and(|t| t.has_pending())
        {
            self.request_service();
            return;
        }
        let connection = (self.transport_dirty && self.retiring_transport.is_none()).then(|| {
            self.retry_at.map_or(Duration::from_millis(1), |at| {
                at.saturating_duration_since(Instant::now())
            })
        });
        let registration = self
            .discovery_retry_at
            .map(|at| at.saturating_duration_since(Instant::now()));
        let producer = self
            .port_discovery
            .as_ref()
            .and_then(|registration| registration.retry_after());
        if let Some(delay) = [connection, registration, producer].into_iter().flatten().min() {
            wake.later(delay);
        }
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_transport(&mut self, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;
        self.retry_at = None;
        let config = if self.module_enabled(snapshot) {
            match self.transport_config(snapshot) {
                Ok(config) => config,
                Err(error) => {
                    self.retire_transport();
                    self.projection.warning(Some(&error));
                    return;
                }
            }
        } else {
            None
        };
        let Some(config) = config else {
            self.retire_transport();
            self.projection.warning(None);
            return;
        };
        if let Some(transport) = self.transport.as_ref().filter(|_| {
            self.last_transport_config
                .as_ref()
                .is_some_and(|last| last.same_endpoint(&config))
        }) {
            if let Err(error) = transport.update_input(config.input.clone()) {
                self.retire_transport();
                self.projection.warning(Some(&error));
            } else {
                self.last_transport_config = Some(config);
            }
            return;
        }
        self.retire_transport();
        if self.retiring_transport.is_some() {
            self.transport_dirty = true;
            return;
        }
        let Some(wake) = &self.wake else { return };
        match SerialConnectionHandle::spawn(config.clone(), wake.notifier.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.projection.warning(None);
            }
            Err(error) => {
                self.projection.warning(Some(&error));
                self.transport_dirty = true;
                self.retry_at = Some(Instant::now() + Duration::from_millis(250));
            }
        }
    }

    fn transport_refresh_due(&self) -> bool {
        self.transport_dirty && self.retiring_transport.is_none() && self.retry_at.is_none_or(|at| Instant::now() >= at)
    }

    fn transport_config(&self, snapshot: &ProcessTreeSnapshot) -> Result<Option<SerialConnectionConfig>, String> {
        let port = snapshot
            .node(self.port_name.id())
            .and_then(|node| node.param_value.as_ref())
            .and_then(|value| match value {
                ParamValue::Enum(port) => Some(port.as_str()),
                _ => None,
            })
            .ok_or("Serial port selection is unavailable")?;
        if port.len() > 1024 {
            return Err("Serial port name exceeds 1024 bytes".into());
        }
        let Some(port_name) = serial_port_name_for_variant(port) else {
            return Ok(None);
        };
        let baud = snapshot
            .node(self.baud_rate.id())
            .and_then(|node| node.param_value.as_ref())
            .and_then(|value| match value {
                ParamValue::Int(baud) => Some(*baud),
                _ => None,
            })
            .ok_or("Serial baud rate is unavailable")?;
        let baud_rate = u32::try_from(baud).map_err(|_| "Serial baud rate must be positive")?;
        let boolean = |id| {
            snapshot
                .node(id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(|value| match value {
                    ParamValue::Bool(value) => Some(*value),
                    _ => None,
                })
                .ok_or("Serial control line configuration is unavailable")
        };
        let config = SerialConnectionConfig {
            port_name,
            baud_rate,
            dtr: boolean(self.dtr.id())?,
            rts: boolean(self.rts.id())?,
            input: StreamInputConfig {
                module: self.id(),
                parse: self.stream.parse_config_cached(),
                processing_enabled: self.stream.processing_enabled_cached(),
                log_incoming: self.stream.log_incoming_enabled(),
            },
        };
        config.validate()?;
        Ok(Some(config))
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        if !self.pending_inputs.is_empty() {
            return;
        }
        let retiring = self.retiring_transport.is_some();
        let Some(transport) = self.retiring_transport.as_ref().or(self.transport.as_ref()) else {
            return;
        };
        let mut events = Vec::new();
        let disconnected = transport.drain_events(&mut events).state == PendingDrainState::Disconnected;
        let mut received = false;
        for event in events {
            match event {
                SerialConnectionEvent::Input(input) => {
                    received = true;
                    if let Some(message) = &input.log_message {
                        golden_core::log!(origin = self.id(); message.as_str());
                    }
                    if let Err(error) = &input.values {
                        logerror!("Failed to parse serial input: {}", error);
                    }
                    Self::report_rejected_input(input.rejected_lines);
                    self.pending_inputs.push_back(InputDelivery::new(input));
                }
                SerialConnectionEvent::RejectedInput(count) => Self::report_rejected_input(count),
                SerialConnectionEvent::OutgoingLog(message) => {
                    golden_core::log!(origin = self.id(); message.as_str());
                }
                SerialConnectionEvent::Warning(error) => {
                    logerror!("Serial transport warning: {}", error);
                    if !retiring {
                        self.projection.warning(Some(&error));
                    }
                }
                SerialConnectionEvent::Status(_) if retiring => {}
                SerialConnectionEvent::Status(status) => match status {
                    SerialConnectionStatus::Connected { .. } => {
                        self.projection.connected(true);
                        self.projection.warning(None);
                    }
                    SerialConnectionStatus::Recovering { message, .. } => {
                        self.projection.connected(false);
                        self.projection.warning(Some(&message));
                    }
                },
            }
        }
        if disconnected {
            if retiring {
                self.retiring_transport = None;
            } else {
                self.transport = None;
                self.projection.warning(Some(
                    "Serial worker stopped; outstanding commands have an unknown outcome and are not replayed",
                ));
                self.retry_at = Some(Instant::now() + Duration::from_millis(250));
            }
            self.last_transport_config = None;
            self.projection.connected(false);
            self.transport_dirty = true;
        }
        if received {
            self.stream.emit_incoming_traffic(ctx);
        }
    }
    fn report_rejected_input(count: u64) {
        if count > 0 {
            logerror!(
                "Rejected {} serial lines at an input/configuration/connection boundary",
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
                    .is_ok_and(|applied| applied.as_ref().is_some_and(|config| last.same_endpoint(config)))
            })
        {
            return Err("Serial configuration is pending or disabled; command was not admitted".into());
        }
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "serial transport is not available".to_string())?;
        transport.send(&request.bytes, self.stream.log_outgoing_enabled())?;
        self.stream.emit_outgoing_traffic(ctx);

        Ok(format!("Queued serial {}", request.description))
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
        if self.param_affects_transport(param) || self.stream.affects_input_configuration(param) {
            self.transport_dirty = true;
            self.retry_at = None;
        }
        if self.port_name.is_bound() && self.port_name.id() == param {
            self.projection.selection_changed();
        }
        if self.transport_dirty || self.projection.needs_snapshot() {
            self.request_service();
        }
    }

    fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.port_name.is_bound() && self.port_name.id() == param)
            || (self.baud_rate.is_bound() && self.baud_rate.id() == param)
            || (self.dtr.is_bound() && self.dtr.id() == param)
            || (self.rts.is_bound() && self.rts.id() == param)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
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
    }
    fn reset_runtime(&mut self) {
        self.stop_transport();
        self.port_discovery = None;
        self.last_transport_config = None;
        self.wake = None;
        self.retry_at = None;
        self.discovery_retry_at = None;
        self.projection = SerialProjection::default();
        self.pending_inputs.clear();
        self.stream.discard_pending_input();
        self.transport_dirty = true;
    }
}

#[golden_core::item(
    "module",
    node = "serial_module",
    via = stream,
    from_struct,
    menu_path = ["Hardware"]
)]
impl Node for SerialModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.stream.init(ctx);
        self.transport_dirty = true;
        self.refresh_data_capabilities(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let source = match ctx.node_waker() {
            Ok(source) => source,
            Err(error) => {
                logerror!("Serial wake registration failed: {}", error);
                return;
            }
        };
        if !self.wake.as_ref().is_some_and(|wake| wake.matches(&source)) {
            self.reset_runtime();
            self.wake = match ModuleWake::new(source) {
                Ok(wake) => Some(wake),
                Err(error) => {
                    logerror!("Serial deadline registration failed: {}", error);
                    return;
                }
            };
        }
        self.ensure_port_discovery_registration();
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.stream.refresh_config_cache(&snapshot);
            self.refresh_transport(&snapshot);
            self.project_feedback(ctx, &snapshot);
        }
        self.arm_service();
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        if let Some(wake) = &self.wake {
            wake.cancel_deadline();
        }
        self.projection.settle();
        self.ensure_port_discovery_registration();
        self.drain_transport_events(ctx);
        if let Some(input) = self.pending_inputs.front_mut() {
            if input.service(ctx, &mut self.stream) {
                self.pending_inputs.pop_front();
            }
        }
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.stream.refresh_config_cache(&snapshot);
            self.refresh_data_capabilities(ctx);
            if self.transport_refresh_due() {
                self.refresh_transport(&snapshot);
            }
            self.project_feedback(ctx, &snapshot);
            self.stream.process_pending(ctx, &snapshot);
        }
        self.arm_service();
    }
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.reset_runtime();
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.transport_refresh_due()
            || self.stream.has_pending_messages()
            || self.projection.needs_snapshot()
            || self
                .port_discovery
                .as_ref()
                .is_some_and(|registration| registration.version() != self.projection.catalog.version)
    }
    fn worker_wake_when_disabled(&self) -> bool {
        true
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
    }

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if node != self.id() && patch.enabled.is_some() {
            self.transport_dirty = true;
            self.retry_at = None;
            self.request_service();
        }
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, enabled: bool) {
        self.retry_at = None;
        self.transport_dirty = enabled;
        if !enabled {
            self.retire_transport();
            self.projection.warning(None);
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
