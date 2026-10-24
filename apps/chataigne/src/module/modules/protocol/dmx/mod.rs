mod frame;
mod outgoing;
mod persistence;
mod service;
mod transport;

use std::{
    collections::VecDeque,
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant},
};

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    node,
    node::{Node, NodeCreationContext, NodeData, NodeId, NodeScriptDescriptor},
    parameter::{ByteArray, Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::ReconnectBackoff;

pub(crate) use crate::app::module_modules_protocol_dmx_commands::DMX_COMMAND_TYPES;
pub(crate) use frame::{DMX_SLOT_COUNT, DmxFrame, parse_slots_json};
pub(crate) use outgoing::{DmxCommandRequest, dispatch as queue_dmx_command};
pub(crate) use persistence::migrate_command_children;
pub(crate) use transport::{DmxProtocol, DmxTransportConfig, DmxTransportHandle, DmxWorkerEvent};
use transport::{ReceivedFrame, SendReceipt, SendStatus};

const ARTNET_COMPILED_KERNEL: &str = "chataigne.runtime.artnet";
const SACN_COMPILED_KERNEL: &str = "chataigne.runtime.sacn";
const DMX_TRANSPORT_WARNING_ID: &str = "dmx_transport";
const DMX_SCRIPT_METHODS: &[&str] = &["setChannel", "setChannels", "sendFrame", "blackout"];

#[node("dmx_module_base", label = "DMX Module")]
#[children(
    folder(connection) {
        network_interface: Enum = "any" (
            label = "Network Interface",
            description = "Network interface used for DMX input and output.",
            enum_options = ["any (default)"]
        );
        receive_enabled: bool = false (
            label = "Receive",
            description = "Receive the configured DMX universe on the protocol's UDP port."
        );
        listen_port: i32 = 6454 [1..65535] (
            label = "Port",
            description = "UDP port used for protocol input and unicast or broadcast output.",
            widget = "text"
        );
        destination: String = "255.255.255.255:6454".to_string() (
            label = "Destination",
            description = "Destination IP and port. Leave empty on sACN to use universe multicast."
        );
    }
    folder(parameters) {
        universe: i32 = 1 [1..63999] (
            label = "Universe",
            description = "One-based DMX universe transported by this module."
        );
        priority: i32 = 100 [0..200] (
            label = "Priority",
            description = "sACN source priority. Art-Net keeps this value only as frame metadata."
        );
        output_frame: ByteArray = ByteArray::default() (
            label = "Output Channels",
            description = "Persistent byte array containing up to 512 channel values."
        );
    }
    folder(values) {
        received_universe: i32 = 0 (
            label = "Received Universe",
            read_only = true
        );
        received_frame: ByteArray = ByteArray::default() (
            label = "Received Channels",
            description = "Latest received DMX frame as a byte array.",
            read_only = true
        );
        dropped_frames: i32 = 0 (
            label = "Dropped Frames",
            description = "Frames replaced by newer input before the engine consumed them.",
            read_only = true
        );
    }
)]
pub struct DmxModuleBase {
    base: crate::app::ModuleBase,
    protocol: DmxProtocol,
    transport: Option<DmxTransportHandle>,
    last_transport_config: Option<DmxTransportConfig>,
    transport_dirty: bool,
    output_slots: Vec<u8>,
    reconnect: ReconnectBackoff,
    retry_at: Option<Instant>,
    pending_sends: VecDeque<SendReceipt>,
    wake: Option<crate::app::module::common::worker_wake::ModuleWake>,
    outgoing: outgoing::Outgoing,
}

impl DmxModuleBase {
    pub fn create(protocol: DmxProtocol) -> Self {
        Self::new(
            crate::app::ModuleBase::create_with_command_types(DMX_COMMAND_TYPES),
            protocol,
            None,
            None,
            true,
            Vec::new(),
            ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(8)),
            None,
            VecDeque::new(),
            None,
            outgoing::Outgoing::default(),
        )
    }

    fn compiled_kernel(&self) -> &'static str {
        match self.protocol {
            DmxProtocol::ArtNet => ARTNET_COMPILED_KERNEL,
            DmxProtocol::Sacn => SACN_COMPILED_KERNEL,
        }
    }

    fn initialize_fresh_defaults(&mut self, ctx: &mut ProcessCtx) {
        self.listen_port.set(ctx, i32::from(self.protocol.default_port()));
        self.destination.set(
            ctx,
            match self.protocol {
                DmxProtocol::ArtNet => {
                    format!("255.255.255.255:{}", self.protocol.default_port())
                }
                DmxProtocol::Sacn => String::new(),
            },
        );
    }

    fn transport_config(&self) -> Result<DmxTransportConfig, String> {
        if self.network_interface.get_ref().as_str().len() > 128 || self.destination.get_ref().len() > 128 {
            return Err("DMX interface and destination text are limited to 128 bytes".into());
        }
        let bind_host = crate::app::module::common::network_interfaces::bind_host_for_interface_variant(
            self.network_interface.get_ref().as_str(),
        );
        let bind_ip = bind_host
            .parse::<IpAddr>()
            .map_err(|_| "DMX network interface must be an IP address or any".to_owned())?;
        let listen_port = u16::try_from(self.listen_port.get())
            .map_err(|_| "DMX UDP port must be between 1 and 65535".to_string())?;
        let universe = u16::try_from(self.universe.get()).map_err(|_| "DMX universe must be positive".to_string())?;
        if universe == 0 || universe > self.protocol.maximum_universe() {
            return Err(format!(
                "{} universe must be between 1 and {}",
                self.protocol.label(),
                self.protocol.maximum_universe()
            ));
        }
        let destination = parse_destination(self.destination.get_ref().as_str(), listen_port, self.protocol)?;
        Ok(DmxTransportConfig {
            protocol: self.protocol,
            source_id: *self.node_data().meta.uuid.0.as_bytes(),
            bind_ip,
            listen_port,
            receive_enabled: self.receive_enabled.get(),
            universe,
            destination,
        })
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).is_some_and(|node| node.enabled)
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;
        if !self.module_enabled(snapshot) {
            self.stop_transport();
            self.last_transport_config = None;
            self.retry_at = None;
            self.base.set_connected(ctx, false);
            self.clear_transport_warning(ctx);
            return;
        }

        let config = match self.transport_config() {
            Ok(config) => config,
            Err(error) => {
                self.transport_failed(ctx, error);
                return;
            }
        };
        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            return;
        }

        self.stop_transport();
        let Some(wake) = &self.wake else { return };
        match DmxTransportHandle::spawn(config.clone(), wake.notifier.clone()) {
            Ok(transport) => {
                self.transport = Some(transport);
                self.last_transport_config = Some(config);
                self.retry_at = None;
                self.reconnect.reset();
                self.base.set_connected(ctx, false);
                self.clear_transport_warning(ctx);
            }
            Err(error) => self.transport_failed(ctx, error),
        }
    }

    fn transport_failed(&mut self, ctx: &mut ProcessCtx, error: String) {
        self.stop_transport();
        self.last_transport_config = None;
        self.base.set_connected(ctx, false);
        self.set_transport_warning(ctx, error.as_str());
        self.retry_at = Some(self.reconnect.schedule(Instant::now()));
        if let (Some(wake), Some(at)) = (&self.wake, self.retry_at) {
            wake.later(at.saturating_duration_since(Instant::now()));
        }
    }

    fn retry_due(&self) -> bool {
        self.retry_at.is_some_and(|retry_at| Instant::now() >= retry_at)
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }

    fn set_transport_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if let Some(connection) = self.base.connection_id() {
            golden_core::node::NodeHandle::new(connection).set_warning_with(
                ctx,
                Some(DMX_TRANSPORT_WARNING_ID),
                message,
                None,
            );
        }
    }

    fn clear_transport_warning(&self, ctx: &mut ProcessCtx) {
        if let Some(connection) = self.base.connection_id() {
            golden_core::node::NodeHandle::new(connection).clear_warning(ctx, Some(DMX_TRANSPORT_WARNING_ID));
        }
    }

    fn drain_transport(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..self.pending_sends.len().min(16) {
            let receipt = self.pending_sends.pop_front().unwrap();
            match receipt.status() {
                SendStatus::Pending => self.pending_sends.push_back(receipt),
                SendStatus::Sent => self.base.emit_outgoing_traffic(ctx),
                SendStatus::FailedUnconfirmed | SendStatus::NotSent => {
                    let message = receipt
                        .failure()
                        .unwrap_or("DMX send cancelled or worker stopped; delivery is unconfirmed");
                    self.set_transport_warning(ctx, message);
                    golden_core::logerror!(message);
                }
            }
        }
        let Some(transport) = &self.transport else { return };
        let mut events = Vec::with_capacity(4);
        for _ in 0..4 {
            if let Some(event) = transport.take_status() {
                events.push(event);
            } else {
                break;
            }
        }
        let stopped = !transport.is_running();
        let frame = if !self.transport_dirty && ctx.tree_snapshot().is_some() {
            transport.take_frame()
        } else {
            None
        };
        let replaced_frames = if frame.is_some() {
            transport.take_replaced_frames()
        } else {
            0
        };
        for event in events {
            match event {
                DmxWorkerEvent::Ready => {
                    self.base.set_connected(ctx, true);
                    self.clear_transport_warning(ctx);
                    golden_core::node::NodeHandle::new(self.id()).clear_warning(ctx, Some("dmx_input"));
                    self.reconnect.reset();
                }
                DmxWorkerEvent::Warning(message) => self.set_transport_warning(ctx, &message),
                DmxWorkerEvent::InputLost => {
                    golden_core::node::NodeHandle::new(self.id()).set_warning_with(
                        ctx,
                        Some("dmx_input"),
                        "DMX input sources lost; holding last look",
                        None,
                    );
                }
                DmxWorkerEvent::InputRecovered => {
                    golden_core::node::NodeHandle::new(self.id()).clear_warning(ctx, Some("dmx_input"));
                }
                DmxWorkerEvent::Error(error) => {
                    self.transport_failed(ctx, error);
                    return;
                }
                #[cfg(test)]
                DmxWorkerEvent::Frame(_) => unreachable!(),
            }
        }
        if stopped {
            self.transport_failed(ctx, "DMX native worker stopped unexpectedly".into());
            return;
        }
        let Some(frame) = frame else { return };
        self.apply_received_frame(ctx, frame, replaced_frames);
    }

    fn apply_received_frame(
        &mut self,
        ctx: &mut ProcessCtx,
        frame: golden_io::RetainedPayload<ReceivedFrame>,
        replaced_frames: u64,
    ) {
        use golden_core::edit::{Edit, EditPreconditions, NodeReadGuard};
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let mut guards = Vec::with_capacity(9);
        for id in [
            self.id(),
            self.received_universe.id(),
            self.received_frame.id(),
            self.dropped_frames.id(),
        ] {
            let Some(guard) = NodeReadGuard::capture_structure(&snapshot, id) else {
                return;
            };
            guards.push(guard);
        }
        for id in [
            self.network_interface.id(),
            self.receive_enabled.id(),
            self.listen_port.id(),
            self.destination.id(),
            self.universe.id(),
        ] {
            let Some(guard) = NodeReadGuard::capture(&snapshot, id) else {
                return;
            };
            guards.push(guard);
        }
        ctx.edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(guards, vec![frame.source.clone()])
                .expect("nine fixed DMX source reads"),
        });
        if replaced_frames > 0 {
            self.dropped_frames.set(
                ctx,
                self.dropped_frames
                    .get()
                    .saturating_add(i32::try_from(replaced_frames).unwrap_or(i32::MAX)),
            );
        }
        self.received_universe.set(ctx, i32::from(frame.universe));
        self.received_frame.set(ctx, ByteArray(frame.slots.clone()));
        self.base.emit_incoming_traffic(ctx);
        ctx.emit_custom_event(frame.callback.event(self.id()));
        ctx.edits.retain_payload(frame);
    }

    fn persist_output_slots(&mut self, ctx: &mut ProcessCtx) {
        if self.output_frame.get_ref().as_slice() == self.output_slots.as_slice() {
            return;
        }
        let value = ParamValue::Bytes(self.output_slots.clone());
        self.output_frame.apply_runtime_value(&value);
        ctx.set_param_from(
            self.id(),
            self.output_frame.id(),
            value,
            golden_core::parameter::ParameterEventBehaviour::Coalesce,
        );
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if matches!(ctx.current_event().map(|event| &event.kind), Some(golden_core::events::EventKind::ParamChanged { source: Some(source), .. }) if *source == self.id())
        {
            return;
        }
        if [
            self.network_interface.id(),
            self.receive_enabled.id(),
            self.listen_port.id(),
            self.destination.id(),
            self.universe.id(),
        ]
        .contains(&param)
        {
            self.transport_dirty = true;
            self.retry_at = None;
            self.request_service();
        }
        if param == self.receive_enabled.id() {
            self.base.set_data_capabilities(
                ctx,
                crate::app::module::ModuleDataCapabilities::new(self.receive_enabled.get(), true),
            );
        }
        if param == self.output_frame.id() {
            if let Err(error) = self.capture_property_output(ctx) {
                self.set_transport_warning(ctx, &error);
            }
        }
    }

    fn script_descriptor_for_node(&self, node_data: &NodeData, node_type: &str) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(node_data, node_type, DMX_SCRIPT_METHODS)
    }
}

#[node("dmx_module_base", via = base, from_struct)]
impl Node for DmxModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        if let Err(error) = self.register_service(ctx) {
            self.set_transport_warning(ctx, &error);
        }
        self.base.configure_command_tester(ctx, DMX_COMMAND_TYPES);
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.transport_dirty = true;
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(self.receive_enabled.get(), true),
        );
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        if let Err(error) = self.register_service(ctx) {
            self.set_transport_warning(ctx, &error);
            return;
        }
        if context == NodeCreationContext::Fresh {
            self.initialize_fresh_defaults(ctx);
        }
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(self.receive_enabled.get(), true),
        );
        self.transport_dirty = true;
        self.request_service();
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.clear_output();
        self.wake = None;
        self.stop_transport();
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.service_needs_snapshot()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel(self.compiled_kernel())
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.service_needs_snapshot()
    }
    fn worker_wake_when_disabled(&self) -> bool {
        self.transport.is_some() || !self.pending_sends.is_empty()
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
        if let Some(result) = self.capture_script_output(ctx, method, args) {
            result?;
            return Ok(true);
        }
        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot.as_ref(), param, &old_value);
        }
        self.on_param_change_inner(ctx, param);
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.transport_dirty = true;
            self.request_service();
        } else {
            self.clear_output();
            self.stop_transport();
            self.last_transport_config = None;
            self.transport_dirty = false;
            self.retry_at = None;
            if let Some(wake) = &self.wake {
                wake.cancel_deadline();
            }
            self.base.set_connected(ctx, false);
        }
    }
}

macro_rules! delegate_dmx_module_node {
    () => {
        fn project_migrate_children<'a>(
            &self,
            children: &'a [golden_core::engine::ProjectNodeRecord],
        ) -> Result<std::borrow::Cow<'a, [golden_core::engine::ProjectNodeRecord]>, String> {
            persistence::migrate_module_children(children)
        }

        fn init(&mut self, ctx: &mut ProcessCtx) {
            self.base.init(ctx);
        }

        fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
            self.base.on_node_ready(ctx, context);
        }

        fn update(&mut self, ctx: &mut ProcessCtx) {
            self.base.update(ctx);
        }

        fn destroy(&mut self, ctx: &mut ProcessCtx) {
            self.base.destroy(ctx);
        }

        fn needs_update(&self) -> bool {
            self.base.needs_update()
        }

        fn update_requires_tree_snapshot(&self) -> bool {
            self.base.update_requires_tree_snapshot()
        }
        fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
            self.base.on_worker_wake(ctx);
        }
        fn worker_wake_requires_tree_snapshot(&self) -> bool {
            self.base.worker_wake_requires_tree_snapshot()
        }
        fn worker_wake_when_disabled(&self) -> bool {
            self.base.worker_wake_when_disabled()
        }

        fn execution_rule(&self) -> NodeExecutionRule {
            self.base.execution_rule()
        }

        fn child_event_interest_depth(&self, event: &Event) -> u32 {
            self.base.child_event_interest_depth(event)
        }

        fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
            self.base
                .script_descriptor_for_node(self.node_data(), self.get_type())
        }

        fn engine_call_script_method(
            &mut self,
            ctx: &mut ProcessCtx,
            method: &str,
            args: &[ParamValue],
        ) -> Result<bool, String> {
            self.base.engine_call_script_method(ctx, method, args)
        }

        fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
            self.base.on_param_change(ctx, param, old_value);
        }

        fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
            self.base.on_effective_enabled_changed(ctx, enabled);
        }

        fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
            self.base.on_custom_event(ctx, event);
        }
    };
}

#[node("artnet_module", label = "Art-Net")]
pub struct ArtNetModule {
    base: DmxModuleBase,
}

impl ArtNetModule {
    pub fn create() -> Self {
        Self::new(DmxModuleBase::create(DmxProtocol::ArtNet))
    }
}

#[golden_core::item(
    "module",
    node = "artnet_module",
    via = base,
    from_struct,
    menu_path = ["Network", "Lighting"]
)]
impl Node for ArtNetModule {
    delegate_dmx_module_node!();

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("sacn_module", label = "sACN")]
pub struct SacnModule {
    base: DmxModuleBase,
}

impl SacnModule {
    pub fn create() -> Self {
        Self::new(DmxModuleBase::create(DmxProtocol::Sacn))
    }
}

#[golden_core::item(
    "module",
    node = "sacn_module",
    via = base,
    from_struct,
    menu_path = ["Network", "Lighting"]
)]
impl Node for SacnModule {
    delegate_dmx_module_node!();

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

pub(crate) fn parse_destination(
    value: &str,
    default_port: u16,
    protocol: DmxProtocol,
) -> Result<Option<SocketAddr>, String> {
    let value = value.trim();
    if value.is_empty() {
        return match protocol {
            DmxProtocol::ArtNet => Err("Art-Net destination cannot be empty".to_string()),
            DmxProtocol::Sacn => Ok(None),
        };
    }
    value
        .parse::<SocketAddr>()
        .or_else(|_| {
            value
                .parse::<IpAddr>()
                .map(|address| SocketAddr::new(address, default_port))
        })
        .map(Some)
        .map_err(|_| "DMX destination must be an IP address with an optional port".to_string())
}

pub(crate) fn script_set_channel(args: &[ParamValue]) -> Result<DmxCommandRequest, String> {
    if args.len() != 2 {
        return Err("setChannel expects channel and value".to_string());
    }
    if args.iter().any(|value| matches!(value, ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) if text.len() > 32)) {
        return Err("setChannel numeric text exceeds 32 bytes".into());
    }
    let channel = args[0]
        .as_int()
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| (1..=DMX_SLOT_COUNT as u16).contains(value))
        .ok_or_else(|| "setChannel channel must be between 1 and 512".to_string())?;
    let value = args[1]
        .as_int()
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| "setChannel value must be between 0 and 255".to_string())?;
    Ok(DmxCommandRequest::SetChannel { channel, value })
}

pub(crate) fn script_send_frame(args: &[ParamValue]) -> Result<DmxCommandRequest, String> {
    if args.len() != 1 {
        return Err("sendFrame expects one byte array or JSON channel string".to_string());
    }
    let value = match &args[0] {
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => value.as_str(),
        ParamValue::Bytes(slots) if slots.len() <= DMX_SLOT_COUNT => {
            return Ok(DmxCommandRequest::SendFrame { slots: slots.clone() });
        }
        _ => return Err("sendFrame expects up to 512 bytes or a JSON channel string".into()),
    };
    parse_slots_json(value).map(|slots| DmxCommandRequest::SendFrame { slots })
}

pub(crate) fn script_set_channels(args: &[ParamValue]) -> Result<DmxCommandRequest, String> {
    if args.len() != 2 {
        return Err("setChannels expects a starting channel and a byte array".into());
    }
    let channel = args[0]
        .as_int()
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| (1..=512).contains(value))
        .ok_or("setChannels start must be between 1 and 512")?;
    let slots = args[1].as_bytes().ok_or("setChannels expects a byte array")?;
    if slots.is_empty() || usize::from(channel - 1) + slots.len() > DMX_SLOT_COUNT {
        return Err("setChannels requires nonempty data contained within channels 1..512".into());
    }
    Ok(DmxCommandRequest::SetChannels {
        channel,
        slots: slots.to_vec(),
    })
}

pub(crate) fn script_blackout(args: &[ParamValue]) -> Result<DmxCommandRequest, String> {
    if !args.is_empty() {
        return Err("blackout does not accept arguments".to_string());
    }
    Ok(DmxCommandRequest::Blackout)
}

#[cfg(test)]
mod tests;
