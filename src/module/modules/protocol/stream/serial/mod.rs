use std::sync::mpsc::TryRecvError;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::{
    serial::{
        serial_port_name_for_variant, serial_port_options, sync_serial_port_enum_options,
        SerialConnectionConfig, SerialConnectionEvent, SerialConnectionHandle,
        SerialConnectionStatus, SerialDiscoveryRegistration, SerialDiscoverySnapshot,
        NO_SERIAL_PORT_VARIANT,
    },
    streaming::{
        commands::StreamingSendRequest,
        module_helpers::{
            format_bytes_for_log, streaming_command_type_supported, streaming_parse_config, StreamingIncomingQueue,
        },
        parser::{StreamingParseConfig, StreamingParser},
    },
};

const SERIAL_MODULE_UPDATE_RATE_HZ: u32 = 120;
const SERIAL_PORT_OPTIONS_WARNING_ID: &str = "serial_port_options";
const SERIAL_PORT_CONNECTION_WARNING_ID: &str = "serial_port_connection";

#[node("serial_module", label = "Serial")]
#[children(
    folder(parameters, label = "Parameters", reuse = true) {
        folder(port, label = "Port") {
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
        }
        folder(receiver, label = "Receiver", can_be_disabled = true) {
            auto_add: bool = true (
                label = "Auto Add",
                description = "Automatically create missing value nodes from incoming serial data."
            );
            parse_mode: Enum = "line" (
                label = "Parse Mode",
                description = "How incoming bytes are converted into values.",
                enum_options = ["line (default)", "raw"]
            );
            line_delimiter: String = "\\n".to_string() (
                label = "Line Delimiter",
                description = "Delimiter that terminates one incoming line. Escape sequences such as \\n, \\r, \\t, and \\xNN are supported."
            );
            value_separator: String = ",".to_string() (
                label = "Value Separator",
                description = "Separator used to split one line into values. Leave empty to split on whitespace."
            );
            first_element: Enum = "name" (
                label = "First Element",
                description = "Whether the first line element is a parameter name or a value.",
                enum_options = ["name (default)", "value"]
            );
            hierarchy_from_name: bool = true (
                label = "Name Hierarchy",
                description = "Split parameter names such as my.received.value into nested value folders."
            );
            hierarchy_delimiter: String = ".".to_string() (
                label = "Hierarchy Delimiter",
                description = "Delimiter used to split incoming parameter names into nested folders."
            );
        }
        folder(sender, label = "Sender", can_be_disabled = true) {}
    }
    node command_tester: crate::app::StreamingCommandTester = crate::app::StreamingCommandTester::create() (
        label = "Command Tester",
        description = "Create and trigger ad-hoc streaming commands through this module."
    );
)]
pub struct SerialModule {
    base: crate::app::ModuleBase,
    parser: StreamingParser,
    incoming: StreamingIncomingQueue,
    port_discovery: Option<SerialDiscoveryRegistration>,
    last_port_snapshot_version: u64,
    last_port_registration_error: Option<String>,
    transport: Option<SerialConnectionHandle>,
    last_transport_config: Option<SerialConnectionConfig>,
    transport_dirty: bool,
}

impl SerialModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            StreamingParser::default(),
            StreamingIncomingQueue::new(),
            None,
            0,
            None,
            None,
            None,
            true,
        )
    }

    fn ensure_port_discovery_registration(&mut self, ctx: &mut ProcessCtx) {
        if self.port_discovery.is_some() {
            return;
        }

        match SerialDiscoveryRegistration::register() {
            Ok(registration) => {
                self.port_discovery = Some(registration);
                self.last_port_snapshot_version = 0;
                self.last_port_registration_error = None;
                self.sync_port_state_from_manager(ctx, true);
            }
            Err(error) => {
                if self.last_port_registration_error.as_deref() != Some(error.as_str()) {
                    logerror!(origin = self.id(); format!("Failed to start serial discovery: {}", error));
                }
                self.last_port_registration_error = Some(error.clone());
                self.set_port_warning(ctx, SERIAL_PORT_OPTIONS_WARNING_ID, error.as_str());
            }
        }
    }

    fn sync_port_state_from_manager(&mut self, ctx: &mut ProcessCtx, force: bool) {
        let Some(port_discovery) = &self.port_discovery else {
            return;
        };

        let snapshot_version = port_discovery.snapshot_version();
        if !force && snapshot_version == self.last_port_snapshot_version {
            return;
        }

        self.apply_port_discovery_snapshot(ctx, port_discovery.snapshot());
    }

    fn apply_port_discovery_snapshot(&mut self, ctx: &mut ProcessCtx, snapshot: SerialDiscoverySnapshot) {
        self.last_port_snapshot_version = snapshot.version;

        if !self.port_name.is_bound() {
            return;
        }

        sync_serial_port_enum_options(ctx, self.port_name.id(), serial_port_options(snapshot.ports.as_slice()));
        if let Some(error) = snapshot.error.as_deref() {
            self.set_port_warning(ctx, SERIAL_PORT_OPTIONS_WARNING_ID, error);
        } else {
            self.clear_port_warning(ctx, SERIAL_PORT_OPTIONS_WARNING_ID);
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
            self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
            self.base.set_connected(ctx, false);
            return;
        }

        let config = match self.transport_config(snapshot) {
            Ok(Some(config)) => config,
            Ok(None) => {
                self.stop_transport();
                self.last_transport_config = None;
                self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
                self.base.set_connected(ctx, false);
                return;
            }
            Err(error) => {
                logerror!("Invalid serial module configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID, error.as_str());
                self.base.set_connected(ctx, false);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            return;
        }

        self.stop_transport();

        match SerialConnectionHandle::spawn(config.clone()) {
            Ok(handle) => {
                golden_core::log!(
                    origin = self.id();
                    format!(
                        "Starting serial connection to {} @ {} baud.",
                        config.port_name,
                        config.baud_rate
                    )
                );
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
                self.base.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!("Failed to start serial transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID, error.as_str());
                self.base.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<Option<SerialConnectionConfig>, String> {
        let receive_enabled = self.receiver_enabled(snapshot).unwrap_or(false);
        let send_enabled = self.sender_enabled(snapshot).unwrap_or(false);
        if !receive_enabled && !send_enabled {
            return Ok(None);
        }

        let port_name = serial_port_name_for_variant(self.port_name.get_ref().as_str())
            .ok_or_else(|| "serial port is not selected".to_string())?;

        let baud_rate = u32::try_from(self.baud_rate.get())
            .map_err(|_| "serial baud rate 'parameters/port/baud_rate' must be positive".to_string())?;

        Ok(Some(SerialConnectionConfig {
            port_name,
            baud_rate,
            receive_enabled,
            send_enabled,
            dtr: self.dtr.get(),
            rts: self.rts.get(),
        }))
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
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

        let parse_config = self.current_parse_config();
        let mut received_bytes = false;
        for event in worker_events {
            match event {
                SerialConnectionEvent::Bytes(bytes) => match self.parser.push_bytes(bytes.as_slice(), &parse_config) {
                    Ok(messages) => {
                        received_bytes = true;
                        if self.base.log_incoming_enabled() {
                            golden_core::log!(
                                origin = self.id();
                                format!("Received serial {}", format_bytes_for_log(bytes.as_slice()))
                            );
                        }
                        self.incoming.push_messages(messages);
                    }
                    Err(error) => {
                        logerror!("Failed to parse serial input: {}", error);
                    }
                },
                SerialConnectionEvent::Warning(error) => {
                    logerror!("Serial transport warning: {}", error);
                    self.set_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID, error.as_str());
                }
                SerialConnectionEvent::Status(status) => match status {
                    SerialConnectionStatus::Connected { port_name } => {
                        if let Some(config) = self.last_transport_config.as_ref() {
                            golden_core::logsuccess!(
                                origin = self.id();
                                format!(
                                    "Connected serial port {} @ {} baud.",
                                    port_name,
                                    config.baud_rate
                                )
                            );
                        } else {
                            golden_core::logsuccess!(
                                origin = self.id();
                                format!("Connected serial port {}.", port_name)
                            );
                        }
                        self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
                        self.base.set_connected(ctx, true);
                    }
                    SerialConnectionStatus::Recovering { message, .. } => {
                        logerror!("Serial transport recovering: {}", message);
                        self.set_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID, message.as_str());
                        self.base.set_connected(ctx, false);
                    }
                }
            }
        }

        if worker_disconnected {
            logerror!("Serial transport worker stopped unexpectedly; restarting.");
            self.stop_transport();
            self.last_transport_config = None;
            self.set_port_warning(
                ctx,
                SERIAL_PORT_CONNECTION_WARNING_ID,
                "Serial transport worker stopped unexpectedly. Restarting.",
            );
            self.base.set_connected(ctx, false);
            self.transport_dirty = true;
        }

        if received_bytes {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn current_parse_config(&self) -> StreamingParseConfig {
        streaming_parse_config(
            self.parse_mode.get_ref().as_str(),
            self.line_delimiter.get_ref().as_str(),
            self.value_separator.get_ref().as_str(),
            self.first_element.get_ref().as_str(),
            self.hierarchy_from_name.get(),
            self.hierarchy_delimiter.get_ref().as_str(),
        )
    }

    fn queue_send_request(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        request: &StreamingSendRequest,
    ) -> Result<String, String> {
        if !self.sender_enabled(snapshot).unwrap_or(false) {
            return Err("serial sender is disabled".to_string());
        }

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "serial transport is not available".to_string())?;
        transport.send(request.bytes.clone())?;
        self.base.emit_outgoing_traffic(ctx);

        if self.base.log_outgoing_enabled() {
            golden_core::log!(
                origin = self.id();
                format!("Sent serial {}", format_bytes_for_log(request.bytes.as_slice()))
            );
        }

        Ok(format!("Queued serial {}", request.description))
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
            .map_err(|error| format!("invalid serial command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, snapshot, &payload))
        {
            logerror!(format!(
                "Failed to handle serial command {:?}: {error}",
                request.command_id
            ));
        }
    }

    fn on_param_change_inner(&mut self, param: NodeId) {
        if self.incoming.take_ignored_param_change(param) {
            return;
        }

        if self.param_affects_transport(param) {
            self.transport_dirty = true;
        }
    }

    fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.port_name.is_bound() && self.port_name.id() == param)
            || (self.baud_rate.is_bound() && self.baud_rate.id() == param)
            || (self.dtr.is_bound() && self.dtr.id() == param)
            || (self.rts.is_bound() && self.rts.id() == param)
    }

    fn receiver_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "receiver")
    }

    fn receiver_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let receiver_id = self.receiver_node_id(snapshot)?;
        snapshot.node(receiver_id).map(|node| node.enabled)
    }

    fn sender_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "sender")
    }

    fn sender_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let sender_id = self.sender_node_id(snapshot)?;
        snapshot.node(sender_id).map(|node| node.enabled)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                self.receiver_enabled(snapshot).unwrap_or(false),
                self.sender_enabled(snapshot).unwrap_or(false),
            ),
        );
    }

    fn set_port_warning(&self, ctx: &mut ProcessCtx, warning_id: &str, message: &str) {
        if !self.port_name.is_bound() {
            return;
        }
        self.port_name.set_warning_with(ctx, Some(warning_id), message, None);
    }

    fn clear_port_warning(&self, ctx: &mut ProcessCtx, warning_id: &str) {
        if !self.port_name.is_bound() {
            return;
        }
        self.port_name.clear_warning(ctx, Some(warning_id));
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }
}

#[golden_core::item(
    "module",
    node = "serial_module",
    via = base,
    from_struct,
    menu_path = ["Generic"]
)]
impl Node for SerialModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.transport_dirty = true;
        crate::app::module::enable_module_authoring(self.node_data_mut());

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.ensure_port_discovery_registration(ctx);

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.refresh_transport(ctx, snapshot);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.drain_transport_events(ctx);
        self.ensure_port_discovery_registration(ctx);
        self.sync_port_state_from_manager(ctx, false);

        let needs_snapshot = self.transport_dirty || self.incoming.has_pending_messages();
        if !needs_snapshot {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);

        if self.transport_dirty {
            self.refresh_transport(ctx, snapshot);
        }

        self.incoming
            .process_pending(ctx, snapshot, self.base.values_id(), self.auto_add.get());
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.port_discovery = None;
        self.last_port_snapshot_version = 0;
        self.last_port_registration_error = None;
        self.stop_transport();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(SERIAL_MODULE_UPDATE_RATE_HZ)
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
                    self.clear_port_warning(ctx, SERIAL_PORT_CONNECTION_WARNING_ID);
                    self.base.set_connected(ctx, false);
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
mod tests {
    use std::time::Duration;

    use golden_core::{
        edit::Edit,
        node::{Folder, Node, NodeId, NodeMetaPatch},
        parameter::{ParamValue, ParameterConstraints, ParameterEnumOption, ParameterEventBehaviour},
        process_ctx::ExecutionPhase,
    };

    use super::SerialModule;

    #[test]
    fn serial_module_root_enable_toggle_stops_and_restarts_transport_while_recovering() {
        let (mut engine, module_id) = create_serial_module();
        let port_name_id = serial_module(&engine, module_id)
            .port_name
            .id();

        allow_serial_port_variant(&mut engine, port_name_id, "missing-test-port");
        set_param(
            &mut engine,
            port_name_id,
            ParamValue::Enum("missing-test-port".to_string()),
        );
        settle_transport_state(&mut engine, "serial transport config should settle");

        let module = serial_module(&engine, module_id);
        assert!(
            module.transport.is_some(),
            "serial module should start a transport handle even when the selected port is recovering"
        );
        assert!(
            module.last_transport_config.is_some(),
            "serial module should retain transport config while enabled"
        );

        set_node_enabled(&mut engine, module_id, false);
        settle_transport_state(&mut engine, "serial module disable should settle");

        let module = serial_module(&engine, module_id);
        assert!(
            module.transport.is_none(),
            "serial module should stop the recovering transport as soon as the module root is disabled"
        );
        assert!(
            module.last_transport_config.is_none(),
            "serial module should clear cached transport config while disabled"
        );

        engine
            .run_tick(Duration::from_millis(20))
            .expect("disabled serial module tick should succeed");

        let module = serial_module(&engine, module_id);
        assert!(
            module.transport.is_none(),
            "serial module should stay disconnected while disabled instead of reconnecting in the background"
        );

        set_node_enabled(&mut engine, module_id, true);
        settle_transport_state(&mut engine, "serial module re-enable should restart transport");

        let module = serial_module(&engine, module_id);
        assert!(
            module.transport.is_some(),
            "serial module should recreate its transport after re-enable"
        );
        assert!(
            module.last_transport_config.is_some(),
            "serial module should restore transport config after re-enable"
        );
    }

    fn create_serial_module() -> (crate::app::AppEngine, NodeId) {
        let root: crate::app::AppNode = Folder::new("root").into();
        let mut engine = crate::app::AppEngine::new(root);
        engine.add_node(SerialModule::create().into(), None);
        engine.apply_edits().expect("serial module should attach");
        for _ in 0..4 {
            engine.apply_edits().expect("serial defaults should materialize");
        }
        engine.resolve().expect("serial runtime schedule should resolve");

        let module_id = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("serial module should be attached under root");

        (engine, module_id)
    }

    fn serial_module(engine: &crate::app::AppEngine, module_id: NodeId) -> &SerialModule {
        let crate::app::AppNode::SerialModule(module) = engine
            .nodes
            .get(module_id)
            .expect("serial module should exist")
        else {
            panic!("expected SerialModule node");
        };

        module
    }

    fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
        engine.edits.push(Edit::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }

    fn allow_serial_port_variant(engine: &mut crate::app::AppEngine, node: NodeId, variant: &str) {
        let snapshot = engine
            .nodes
            .get(node)
            .and_then(|candidate| candidate.engine_param_snapshot())
            .expect("serial port parameter should exist");
        let mut constraints = snapshot.constraints.clone();
        constraints.enum_options.push(ParameterEnumOption {
            variant_id: variant.to_string(),
            value: ParamValue::Enum(variant.to_string()),
            label: variant.to_string(),
            tags: Vec::new(),
            ordering: None,
        });
        set_param_constraints(engine, node, constraints);
        engine
            .apply_edits()
            .expect("serial port test enum option should apply");
    }

    fn set_param_constraints(engine: &mut crate::app::AppEngine, node: NodeId, constraints: ParameterConstraints) {
        engine.edits.push(Edit::SetParamConstraints { node, constraints });
    }

    fn set_node_enabled(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
        engine.edits.push(Edit::PatchMeta {
            node,
            patch: NodeMetaPatch {
                enabled: Some(enabled),
                ..Default::default()
            },
        });
    }

    fn settle_transport_state(engine: &mut crate::app::AppEngine, context: &str) {
        engine.apply_edits().expect(context);
        engine
            .dispatch_inbox(ExecutionPhase::EngineTick)
            .expect("transport edits should dispatch");
        engine
            .apply_edits()
            .expect("transport event reactions should apply");
        engine
            .run_tick(Duration::from_millis(20))
            .expect("transport tick should succeed");
        engine.apply_edits().expect("transport edits should apply");
    }
}
