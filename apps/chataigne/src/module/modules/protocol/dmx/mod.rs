mod frame;
mod transport;

use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    node,
    node::{Node, NodeCreationContext, NodeData, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::ReconnectBackoff;

pub(crate) use crate::app::module_modules_protocol_dmx_commands::{
    DmxCommandRequest, DMX_COMMAND_TYPES,
};
pub(crate) use frame::{parse_slots_json, slots_json, DmxFrame, DMX_SLOT_COUNT};
pub(crate) use transport::{
    DmxProtocol, DmxTransportConfig, DmxTransportHandle, DmxWorkerEvent,
};

const DMX_UPDATE_RATE_HZ: u32 = 120;
const ARTNET_COMPILED_KERNEL: &str = "chataigne.runtime.artnet";
const SACN_COMPILED_KERNEL: &str = "chataigne.runtime.sacn";
const DMX_TRANSPORT_WARNING_ID: &str = "dmx_transport";
const DMX_FRAME_RECEIVED_CALLBACK: &str = "dmxFrameReceived";
const DMX_SCRIPT_METHODS: &[&str] = &["setChannel", "sendFrame", "blackout"];

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
        priority: i32 = 100 [1..200] (
            label = "Priority",
            description = "sACN source priority. Art-Net keeps this value only as frame metadata."
        );
        output_frame: String = "[]".to_string() (
            label = "Output Channels",
            description = "Persistent JSON array containing up to 512 channel values."
        );
    }
    folder(values) {
        received_universe: i32 = 0 (
            label = "Received Universe",
            read_only = true
        );
        received_frame: String = "[]".to_string() (
            label = "Received Channels",
            description = "Latest received DMX frame as a JSON channel array.",
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
    ignored_param_changes: HashSet<NodeId>,
    reconnect: ReconnectBackoff,
    retry_at: Option<Instant>,
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
            HashSet::new(),
            ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(8)),
            None,
        )
    }

    fn compiled_kernel(&self) -> &'static str {
        match self.protocol {
            DmxProtocol::ArtNet => ARTNET_COMPILED_KERNEL,
            DmxProtocol::Sacn => SACN_COMPILED_KERNEL,
        }
    }

    fn initialize_fresh_defaults(&mut self, ctx: &mut ProcessCtx) {
        self.ignored_param_changes.insert(self.listen_port.id());
        self.listen_port
            .set(ctx, i32::from(self.protocol.default_port()));
        self.ignored_param_changes.insert(self.destination.id());
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
        let bind_host =
            crate::app::module::common::network_interfaces::bind_host_for_interface_variant(
                self.network_interface.get_ref().as_str(),
            );
        let bind_ip = bind_host
            .parse::<IpAddr>()
            .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        let listen_port = u16::try_from(self.listen_port.get())
            .map_err(|_| "DMX UDP port must be between 1 and 65535".to_string())?;
        let universe = u16::try_from(self.universe.get())
            .map_err(|_| "DMX universe must be positive".to_string())?;
        if universe == 0 || universe > self.protocol.maximum_universe() {
            return Err(format!(
                "{} universe must be between 1 and {}",
                self.protocol.label(),
                self.protocol.maximum_universe()
            ));
        }
        let destination = parse_destination(
            self.destination.get_ref().as_str(),
            listen_port,
            self.protocol,
        )?;
        Ok(DmxTransportConfig {
            protocol: self.protocol,
            bind_ip,
            listen_port,
            receive_enabled: self.receive_enabled.get(),
            universe,
            destination,
        })
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot
            .node(self.id())
            .is_some_and(|node| node.enabled)
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
        match DmxTransportHandle::spawn(config.clone()) {
            Ok(transport) => {
                self.transport = Some(transport);
                self.last_transport_config = Some(config);
                self.retry_at = None;
                self.reconnect.reset();
                self.base.set_connected(ctx, true);
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
            golden_core::node::NodeHandle::new(connection)
                .clear_warning(ctx, Some(DMX_TRANSPORT_WARNING_ID));
        }
    }

    fn drain_transport(&mut self, ctx: &mut ProcessCtx) {
        let Some(transport) = &self.transport else {
            return;
        };
        let replaced_frames = transport.take_replaced_frames();
        let Some(event) = transport.take_latest_event() else {
            return;
        };
        if replaced_frames > 0 {
            self.ignored_param_changes.insert(self.dropped_frames.id());
            self.dropped_frames.set(
                ctx,
                self.dropped_frames.get().saturating_add(
                    i32::try_from(replaced_frames).unwrap_or(i32::MAX),
                ),
            );
        }
        match event {
            DmxWorkerEvent::Frame(frame) => self.apply_received_frame(ctx, frame),
            DmxWorkerEvent::Error(error) => self.transport_failed(ctx, error),
        }
    }

    fn apply_received_frame(&mut self, ctx: &mut ProcessCtx, frame: DmxFrame) {
        self.ignored_param_changes.insert(self.received_universe.id());
        self.received_universe.set(ctx, i32::from(frame.universe));
        self.ignored_param_changes.insert(self.received_frame.id());
        self.received_frame.set(ctx, slots_json(frame.slots.as_slice()));
        self.base.emit_incoming_traffic(ctx);
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            DMX_FRAME_RECEIVED_CALLBACK,
            vec![
                serde_json::json!(frame.universe),
                serde_json::json!(frame.slots),
                serde_json::json!({
                    "protocol": self.protocol.label(),
                    "sequence": frame.sequence,
                    "priority": frame.priority,
                }),
            ],
        );
    }

    fn configured_frame(&self, slots: Vec<u8>) -> Result<DmxFrame, String> {
        let universe = u16::try_from(self.universe.get())
            .map_err(|_| "DMX universe must be positive".to_string())?;
        let priority = u8::try_from(self.priority.get())
            .map_err(|_| "sACN priority must be between 1 and 200".to_string())?;
        DmxFrame::with_metadata(universe, 0, priority, slots)
    }

    fn send_frame(&mut self, ctx: &mut ProcessCtx, frame: DmxFrame) -> Result<(), String> {
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| format!("{} transport is not connected", self.protocol.label()))?;
        transport.send(frame)?;
        self.base.emit_outgoing_traffic(ctx);
        Ok(())
    }

    fn apply_command(
        &mut self,
        ctx: &mut ProcessCtx,
        request: DmxCommandRequest,
    ) -> Result<(), String> {
        match request {
            DmxCommandRequest::SetChannel { channel, value } => {
                let mut frame = self.configured_frame(self.output_slots.clone())?;
                frame.set_channel(channel, value)?;
                self.output_slots = frame.slots.clone();
                self.persist_output_slots(ctx);
                self.send_frame(ctx, frame)
            }
            DmxCommandRequest::SendFrame { slots } => {
                let frame = self.configured_frame(slots)?;
                self.output_slots = frame.slots.clone();
                self.persist_output_slots(ctx);
                self.send_frame(ctx, frame)
            }
            DmxCommandRequest::Blackout => {
                let mut frame = DmxFrame::blackout(
                    u16::try_from(self.universe.get())
                        .map_err(|_| "DMX universe must be positive".to_string())?,
                )?;
                frame.priority = u8::try_from(self.priority.get())
                    .map_err(|_| "sACN priority must be between 1 and 200".to_string())?;
                self.output_slots = frame.slots.clone();
                self.persist_output_slots(ctx);
                self.send_frame(ctx, frame)
            }
        }
    }

    fn persist_output_slots(&mut self, ctx: &mut ProcessCtx) {
        self.ignored_param_changes.insert(self.output_frame.id());
        self.output_frame
            .set(ctx, slots_json(self.output_slots.as_slice()));
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !DMX_COMMAND_TYPES.contains(&request.command_type.as_str()) {
            return;
        }
        if let Err(error) = serde_json::from_value::<DmxCommandRequest>(request.payload)
            .map_err(|error| format!("invalid DMX command payload: {error}"))
            .and_then(|request| self.apply_command(ctx, request))
        {
            golden_core::logerror!(format!("Failed to execute DMX command: {error}"));
            self.set_transport_warning(ctx, error.as_str());
        }
    }

    fn on_param_change_inner(&mut self, ctx: &mut ProcessCtx, param: NodeId) {
        if self.ignored_param_changes.remove(&param) {
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
        }
        if param == self.receive_enabled.id() {
            self.base.set_data_capabilities(
                ctx,
                crate::app::module::ModuleDataCapabilities::new(
                    self.receive_enabled.get(),
                    true,
                ),
            );
        }
        if param == self.output_frame.id() {
            match parse_slots_json(self.output_frame.get_ref().as_str()) {
                Ok(slots) => {
                    self.output_slots = slots.clone();
                    match self.configured_frame(slots) {
                        Ok(frame) => {
                            if let Err(error) = self.send_frame(ctx, frame) {
                                self.set_transport_warning(ctx, error.as_str());
                            }
                        }
                        Err(error) => self.set_transport_warning(ctx, error.as_str()),
                    }
                }
                Err(error) => self.set_transport_warning(ctx, error.as_str()),
            }
        }
    }

    fn handle_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let request = match method {
            "setChannel" => Some(script_set_channel(args)),
            "sendFrame" => Some(script_send_frame(args)),
            "blackout" => Some(script_blackout(args)),
            _ => None,
        }?;
        Some(request.and_then(|request| self.apply_command(ctx, request)))
    }

    fn script_descriptor_for_node(
        &self,
        node_data: &NodeData,
        node_type: &str,
    ) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            node_data,
            node_type,
            DMX_SCRIPT_METHODS,
        )
    }
}

#[node("dmx_module_base", via = base, from_struct)]
impl Node for DmxModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, DMX_COMMAND_TYPES);
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.transport_dirty = true;
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(self.receive_enabled.get(), true),
        );
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        if context == NodeCreationContext::Fresh {
            self.initialize_fresh_defaults(ctx);
        }
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(self.receive_enabled.get(), true),
        );
        self.output_slots =
            parse_slots_json(self.output_frame.get_ref().as_str()).unwrap_or_default();
        self.transport_dirty = true;
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.drain_transport(ctx);
        if self.retry_due() {
            self.retry_at = None;
            self.transport_dirty = true;
        }
        if !self.transport_dirty {
            return;
        }
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        self.refresh_transport(ctx, snapshot_arc.as_ref());
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn needs_update(&self) -> bool {
        self.transport_dirty
            || self.retry_due()
            || self
                .transport
                .as_ref()
                .is_some_and(DmxTransportHandle::has_pending)
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.transport_dirty
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(DMX_UPDATE_RATE_HZ)
            .with_compiled_kernel(self.compiled_kernel())
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
        if let Some(result) = self.handle_script_method(ctx, method, args) {
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

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, _enabled: bool) {
        self.transport_dirty = true;
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }
}

macro_rules! delegate_dmx_module_node {
    () => {
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

        fn on_param_change(
            &mut self,
            ctx: &mut ProcessCtx,
            param: NodeId,
            old_value: ParamValue,
        ) {
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
        return Err("sendFrame expects one JSON channel array".to_string());
    }
    let value = args[0]
        .as_str()
        .ok_or_else(|| "sendFrame expects a JSON string".to_string())?;
    parse_slots_json(value.as_str()).map(|slots| DmxCommandRequest::SendFrame { slots })
}

pub(crate) fn script_blackout(args: &[ParamValue]) -> Result<DmxCommandRequest, String> {
    if !args.is_empty() {
        return Err("blackout does not accept arguments".to_string());
    }
    Ok(DmxCommandRequest::Blackout)
}

#[cfg(test)]
mod tests;
