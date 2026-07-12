mod transport;

use std::sync::mpsc::TryRecvError;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::{
    mqtt::{
        mqtt_qos_enum_options, MqttPublishRequest, MqttQos, MQTT_PUBLISH_COMMAND_NODE_TYPE,
        MQTT_QOS_AT_MOST_ONCE,
    },
    received_values::{apply_received_value_payload, ReceivedValueApplyOptions, ReceivedValueApplyResult, ReceivedValuePayload},
    streaming::parser::parse_scalar_value,
};

use self::transport::{
    MqttConnectionStatus, MqttCredentials, MqttReceivedPublish, MqttSubscriptionConfig,
    MqttTransportConfig, MqttTransportHandle, MqttWorkerEvent,
};

const MQTT_MODULE_UPDATE_RATE_HZ: u32 = 120;
const MQTT_TARGET_WARNING_ID: &str = "mqtt_target_transport";
const MQTT_SUBSCRIPTION_NODE_TYPE: &str = "mqtt_subscription";
const MQTT_SUBSCRIPTIONS_DECL_ID: &str = "subscriptions";
const MQTT_CREDENTIALS_DECL_ID: &str = "credentials";
const MQTT_PAYLOAD_MODE_AUTO: &str = "auto";
const MQTT_PAYLOAD_MODE_TEXT: &str = "text";
const MQTT_PAYLOAD_MODE_JSON: &str = "json";
const MQTT_PAYLOAD_MODE_RAW: &str = "raw";
const MQTT_MESSAGE_RECEIVED_CALLBACK: &str = "messageReceived";
const MQTT_SCRIPT_METHODS: &[&str] = &["publish", "publishText", "publishJson"];
const MQTT_MODULE_COMMAND_TYPES: &[&str] = &[MQTT_PUBLISH_COMMAND_NODE_TYPE];

#[node("mqtt_subscription", label = "Subscription")]
#[children(
    topic_filter: String = "#".to_string() (
        label = "Topic Filter",
        description = "MQTT subscription filter. Wildcards + and # are supported using MQTT filter rules."
    );
    qos: Enum = MQTT_QOS_AT_MOST_ONCE (
        label = "QoS",
        description = "Maximum MQTT quality of service requested for this subscription.",
        enum_options = mqtt_qos_enum_options()
    );
)]
pub struct MqttSubscription {}

#[golden_core::item("mqtt_subscription", node = "mqtt_subscription", from_struct)]
impl Node for MqttSubscription {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

#[node("mqtt_subscription_manager", label = "Subscriptions")]
pub struct MqttSubscriptionManager {}

#[node("mqtt_subscription_manager", from_struct)]
impl Node for MqttSubscriptionManager {
    golden_core::define_user_item_factory_methods! {
        accepts = ["mqtt_subscription", "folder"];
        items = [
            {
                node_type: "mqtt_subscription",
                item_kind: "mqtt_subscription",
                label: "Subscription",
                select_when_created: false,
                create: |_this: &Self| MqttSubscription::new()
            }
        ];
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

#[node("mqtt_module", label = "MQTT")]
#[children(
    folder(connection) {
        remote_host: String = "127.0.0.1".to_string() (
            label = "Remote Host",
            description = "MQTT broker hostname or IP address."
        );
        remote_port: i32 = 1883 [0..65535] (
            label = "Remote Port",
            description = "MQTT broker port.",
            widget = "text"
        );
        client_id: String = String::new() (
            label = "Client ID",
            description = "MQTT client identifier. Leave empty to generate an app-local ID for this module."
        );
        clean_session: bool = true (
            label = "Clean Session",
            description = "Whether the broker should discard previous session state for this client ID."
        );
        keep_alive_secs: i32 = 30 [1..2147483647] (
            label = "Keep Alive",
            description = "MQTT keep-alive interval in seconds.",
            widget = "text"
        );
        folder(credentials, label = "Credentials", enabled = false, can_be_disabled = true) {
            username: String = String::new() (
                label = "Username",
                description = "MQTT username used when Credentials is enabled."
            );
            password: String = String::new() (
                label = "Password",
                description = "MQTT password used when Credentials is enabled."
            );
        }
        node subscriptions: MqttSubscriptionManager = MqttSubscriptionManager::new() (
            label = "Subscriptions",
            description = "MQTT topic filters received by this module.",
            can_be_disabled = true
        );
        [base_children];
    }
    folder(parameters) {
        folder(processing, label = "Processing") {
            auto_add: bool = true (
                label = "Auto Add",
                description = "Automatically create missing value nodes from incoming MQTT messages."
            );
            payload_mode: Enum = MQTT_PAYLOAD_MODE_AUTO (
                label = "Payload Mode",
                description = "How incoming MQTT payloads are converted into module values.",
                enum_options = mqtt_payload_mode_options()
            );
        }
    }
)]
pub struct MqttModule {
    base: crate::app::ModuleBase,
    transport: Option<MqttTransportHandle>,
    last_transport_config: Option<MqttTransportConfig>,
    transport_dirty: bool,
    pending_incoming_messages: Vec<MqttReceivedPublish>,
}

impl MqttModule {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::new(), None, None, true, Vec::new())
    }

    #[cfg(test)]
    pub(crate) fn disable_transport_for_test(&mut self) {
        self.stop_transport();
        self.transport_dirty = false;
    }

    #[cfg(test)]
    pub(crate) fn enqueue_incoming_message_for_test(&mut self, message: MqttReceivedPublish) {
        self.pending_incoming_messages.push(message);
    }

    #[cfg(test)]
    pub(crate) fn has_pending_incoming_messages_for_test(&self) -> bool {
        !self.pending_incoming_messages.is_empty()
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn refresh_transport(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.transport_dirty = false;

        if !self.module_enabled(snapshot) {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_target_warning(ctx);
            self.base.set_connected(ctx, false);
            return;
        }

        let config = match self.transport_config(snapshot) {
            Ok(config) => config,
            Err(error) => {
                logerror!("Invalid MQTT module configuration: {}", error);
                self.stop_transport();
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
                return;
            }
        };

        if self.transport.is_some() && self.last_transport_config.as_ref() == Some(&config) {
            return;
        }

        self.stop_transport();

        match MqttTransportHandle::spawn(config.clone()) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.clear_target_warning(ctx);
                self.base.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!("Failed to start MQTT transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
            }
        }
    }

    fn transport_config(&self, snapshot: &ProcessTreeSnapshot) -> Result<MqttTransportConfig, String> {
        let remote_host = self.remote_host.get_ref().trim().to_string();
        if remote_host.is_empty() {
            return Err("MQTT remote host cannot be empty".to_string());
        }

        let remote_port = u16::try_from(self.remote_port.get())
            .map_err(|_| "MQTT remote port 'connection/remote_port' must be between 0 and 65535".to_string())?;

        let client_id = self.effective_client_id();
        if client_id.is_empty() && !self.clean_session.get() {
            return Err("MQTT client ID cannot be empty when Clean Session is disabled".to_string());
        }

        let keep_alive_secs = u64::try_from(self.keep_alive_secs.get())
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| "MQTT keep alive 'connection/keep_alive_secs' must be positive".to_string())?;

        Ok(MqttTransportConfig {
            remote_host,
            remote_port,
            client_id,
            credentials: self.credentials_config(snapshot)?,
            clean_session: self.clean_session.get(),
            keep_alive_secs,
            subscriptions: self.collect_enabled_subscriptions(snapshot)?,
        })
    }

    fn effective_client_id(&self) -> String {
        let configured = self.client_id.get_ref().trim();
        if !configured.is_empty() {
            configured.to_string()
        } else {
            format!("chataigne2-{}", self.id().0)
        }
    }

    fn credentials_config(&self, snapshot: &ProcessTreeSnapshot) -> Result<Option<MqttCredentials>, String> {
        if !self.credentials_enabled(snapshot).unwrap_or(false) {
            return Ok(None);
        }

        let username = self.username.get_ref().trim().to_string();
        if username.is_empty() {
            return Err("MQTT username cannot be empty when Credentials is enabled".to_string());
        }

        Ok(Some(MqttCredentials {
            username,
            password: self.password.get_ref().clone(),
        }))
    }

    fn collect_enabled_subscriptions(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<Vec<MqttSubscriptionConfig>, String> {
        if !self.subscriptions_enabled(snapshot).unwrap_or(false) {
            return Ok(Vec::new());
        }

        let Some(subscriptions_id) = self.subscriptions_node_id(snapshot) else {
            return Ok(Vec::new());
        };

        let mut subscription_ids = Vec::new();
        collect_subscriptions_recursive(snapshot, subscriptions_id, &mut subscription_ids);

        subscription_ids
            .into_iter()
            .filter_map(|subscription_id| self.subscription_config(snapshot, subscription_id).transpose())
            .collect()
    }

    fn subscription_config(
        &self,
        snapshot: &ProcessTreeSnapshot,
        subscription_id: NodeId,
    ) -> Result<Option<MqttSubscriptionConfig>, String> {
        if !snapshot.node(subscription_id).is_some_and(|node| node.enabled) {
            return Ok(None);
        }

        let topic_filter = child_string_param(snapshot, subscription_id, "topic_filter").unwrap_or_default();
        let topic_filter = topic_filter.trim().to_string();
        if !rumqttc::valid_filter(topic_filter.as_str()) {
            return Err(format!("invalid MQTT subscription filter '{topic_filter}'"));
        }

        let qos_variant = child_enum_param(snapshot, subscription_id, "qos")
            .unwrap_or_else(|| MQTT_QOS_AT_MOST_ONCE.to_string());
        let qos = MqttQos::from_variant(qos_variant.as_str())
            .ok_or_else(|| format!("invalid MQTT subscription QoS '{qos_variant}'"))?;

        Ok(Some(MqttSubscriptionConfig { topic_filter, qos }))
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

        let mut received_message = false;
        for event in worker_events {
            match event {
                MqttWorkerEvent::Publish(message) => {
                    received_message = true;
                    self.log_incoming_message(&message);
                    self.pending_incoming_messages.push(message);
                }
                MqttWorkerEvent::Status(status) => match status {
                    MqttConnectionStatus::Connected => {
                        golden_core::logsuccess!(
                            origin = self.id();
                            format!("Connected MQTT broker {}:{}.", self.remote_host.get_ref(), self.remote_port.get())
                        );
                        self.clear_target_warning(ctx);
                        self.base.set_connected(ctx, true);
                    }
                    MqttConnectionStatus::Recovering { message } => {
                        logerror!("MQTT transport recovering: {}", message);
                        self.set_target_warning(ctx, message.as_str());
                        self.base.set_connected(ctx, false);
                    }
                },
                MqttWorkerEvent::Error(error) => {
                    logerror!("MQTT transport error: {}", error);
                }
            }
        }

        if worker_disconnected {
            logerror!("MQTT transport worker stopped unexpectedly; restarting.");
            self.stop_transport();
            self.last_transport_config = None;
            self.set_target_warning(ctx, "MQTT transport worker stopped unexpectedly. Restarting.");
            self.base.set_connected(ctx, false);
            self.transport_dirty = true;
        }

        if received_message {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn process_pending_incoming(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> bool {
        let Some(values_id) = self.base.values_id() else {
            self.pending_incoming_messages.clear();
            return false;
        };
        if self.pending_incoming_messages.is_empty() {
            return false;
        }

        let mut remaining = Vec::new();
        let mut messages = std::mem::take(&mut self.pending_incoming_messages).into_iter();

        while let Some(message) = messages.next() {
            let decoded = match received_values_for_publish(&message, self.payload_mode.get_ref().as_str()) {
                Ok(decoded) => decoded,
                Err(error) => {
                    logerror!("Failed to parse MQTT payload from '{}': {}", message.topic, error);
                    self.emit_message_received_callback(ctx, &message);
                    continue;
                }
            };

            let mut retry = false;
            for received in decoded {
                let result = apply_received_value_payload(
                    ctx,
                    snapshot,
                    values_id,
                    received.path_segments.as_slice(),
                    &received.payload,
                    ReceivedValueApplyOptions {
                        auto_add: self.auto_add.get(),
                        // source_description: received.source_description.as_str(),
                        event_behaviour: ParameterEventBehaviour::Append,
                    },
                );

                match result {
                    ReceivedValueApplyResult::Applied {
                        needs_snapshot_refresh,
                    } => {
                        if needs_snapshot_refresh {
                            retry = true;
                            break;
                        }
                    }
                    ReceivedValueApplyResult::Ignored => {}
                    ReceivedValueApplyResult::Retry => {
                        retry = true;
                        break;
                    }
                }
            }

            if retry {
                remaining.push(message);
                remaining.extend(messages);
                self.pending_incoming_messages = remaining;
                return true;
            }

            self.emit_message_received_callback(ctx, &message);
        }

        self.pending_incoming_messages = remaining;
        false
    }

    fn queue_publish_request(&self, ctx: &mut ProcessCtx, request: MqttPublishRequest) -> Result<String, String> {
        let topic = request.topic.trim();
        if !rumqttc::valid_topic(topic) || topic.is_empty() {
            return Err(format!("invalid MQTT publish topic '{}'", request.topic));
        }

        let description = request.description.clone();
        let log_topic = request.topic.clone();
        let log_payload = request.payload.clone();

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "MQTT transport is not available".to_string())?;
        transport.send(request)?;

        self.base.emit_outgoing_traffic(ctx);
        if self.base.log_outgoing_enabled() {
            golden_core::log!(
                origin = self.id();
                format!("Sent MQTT {} to {}", format_mqtt_payload_for_log(log_payload.as_slice()), log_topic)
            );
        }

        Ok(format!("Queued MQTT {description}"))
    }

    fn handle_script_publish_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let request = mqtt_publish_request_from_script(method, args)?;

        Some(request.and_then(|request| self.queue_publish_request(ctx, request).map(|_| ())))
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !MQTT_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str()) {
            return;
        }
        let command_id = request.command_id;

        if let Err(error) = serde_json::from_value::<MqttPublishRequest>(request.payload)
            .map_err(|error| format!("invalid MQTT publish command payload: {error}"))
            .and_then(|payload| self.queue_publish_request(ctx, payload))
        {
            logerror!(format!("Failed to handle MQTT command {:?}: {error}", command_id));
        }
    }

    fn on_param_change_inner(&mut self, snapshot: &ProcessTreeSnapshot, param: NodeId) {
        if self.param_affects_transport(snapshot, param) {
            self.transport_dirty = true;
        }
    }

    fn param_affects_transport(&self, snapshot: &ProcessTreeSnapshot, param: NodeId) -> bool {
        (self.remote_host.is_bound() && self.remote_host.id() == param)
            || (self.remote_port.is_bound() && self.remote_port.id() == param)
            || (self.client_id.is_bound() && self.client_id.id() == param)
            || (self.clean_session.is_bound() && self.clean_session.id() == param)
            || (self.keep_alive_secs.is_bound() && self.keep_alive_secs.id() == param)
            || self
                .credentials_node_id(snapshot)
                .is_some_and(|node| is_descendant_or_self(snapshot, param, node))
            || self
                .subscriptions_node_id(snapshot)
                .is_some_and(|node| is_descendant_or_self(snapshot, param, node))
    }

    fn on_meta_changed_inner(&mut self, _ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if node != self.id() && patch.enabled.is_some() {
            self.transport_dirty = true;
        }
    }

    fn on_effective_enabled_changed_inner(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.transport_dirty = true;
        } else {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_target_warning(ctx);
            self.base.set_connected(ctx, false);
            self.transport_dirty = false;
        }
    }

    fn ensure_default_subscription(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(subscriptions_id) = self.subscriptions_node_id(snapshot) else {
            return;
        };

        let mut subscription_ids = Vec::new();
        collect_subscriptions_recursive(snapshot, subscriptions_id, &mut subscription_ids);
        if !subscription_ids.is_empty() {
            return;
        }

        ctx.add_user_item_boxed(subscriptions_id, Box::new(MqttSubscription::new()), None);
    }

    fn credentials_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let connection_id = self.base.connection_id()?;
        snapshot.find_child_by_decl_id(connection_id, MQTT_CREDENTIALS_DECL_ID)
    }

    fn credentials_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let credentials_id = self.credentials_node_id(snapshot)?;
        snapshot.node(credentials_id).map(|node| node.enabled)
    }

    fn subscriptions_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        self.subscriptions.current_id().or_else(|| {
            let connection_id = self.base.connection_id()?;
            snapshot.find_child_by_decl_id(connection_id, MQTT_SUBSCRIPTIONS_DECL_ID)
        })
    }

    fn subscriptions_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        let subscriptions_id = self.subscriptions_node_id(snapshot)?;
        snapshot.node(subscriptions_id).map(|node| node.enabled)
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                self.subscriptions_enabled(snapshot).unwrap_or(false),
                true,
            ),
        );
    }

    fn set_target_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.remote_host.is_bound() {
            NodeHandle::new(self.remote_host.id()).set_warning_with(ctx, Some(MQTT_TARGET_WARNING_ID), message, None);
        }
    }

    fn clear_target_warning(&self, ctx: &mut ProcessCtx) {
        if self.remote_host.is_bound() {
            NodeHandle::new(self.remote_host.id()).clear_warning(ctx, Some(MQTT_TARGET_WARNING_ID));
        }
    }

    fn stop_transport(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
    }

    fn log_incoming_message(&self, message: &MqttReceivedPublish) {
        if !self.base.log_incoming_enabled() {
            return;
        }

        golden_core::log!(
            origin = self.id();
            format!(
                "Received MQTT {} on {}",
                format_mqtt_payload_for_log(message.payload.as_slice()),
                message.topic
            )
        );
    }

    fn emit_message_received_callback(&self, ctx: &mut ProcessCtx, message: &MqttReceivedPublish) {
        let payload = mqtt_payload_script_arg(message.payload.as_slice());
        crate::app::module::script_api::emit_script_callback(
            ctx,
            self.id(),
            MQTT_MESSAGE_RECEIVED_CALLBACK,
            vec![
                serde_json::json!(message.topic.as_str()),
                payload.clone(),
                serde_json::json!({
                    "topic": message.topic.as_str(),
                    "payload": payload,
                    "bytes": crate::app::module::script_api::bytes_arg(message.payload.as_slice()),
                    "qos": message.qos.variant(),
                    "retain": message.retain,
                }),
            ],
        );
    }
}

#[golden_core::item(
    "module",
    node = "mqtt_module",
    via = base,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for MqttModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, MQTT_MODULE_COMMAND_TYPES);
        self.transport_dirty = true;
        crate::app::module::enable_module_authoring(self.node_data_mut());

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        self.refresh_data_capabilities(ctx, snapshot);
        self.ensure_default_subscription(ctx, snapshot);
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

        let needs_snapshot = self.transport_dirty || !self.pending_incoming_messages.is_empty();
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

        self.process_pending_incoming(ctx, snapshot);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.transport_dirty || !self.pending_incoming_messages.is_empty()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(MQTT_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            MQTT_SCRIPT_METHODS,
        )
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.handle_script_publish_method(ctx, method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.base
            .emit_script_param_callback(ctx, snapshot, param, &old_value);
        self.on_param_change_inner(snapshot, param);
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

#[derive(Clone, Debug, PartialEq)]
struct MqttReceivedValue {
    path_segments: Vec<String>,
    payload: ReceivedValuePayload,
    source_description: String,
}

fn received_values_for_publish(
    message: &MqttReceivedPublish,
    payload_mode: &str,
) -> Result<Vec<MqttReceivedValue>, String> {
    let path_segments = mqtt_topic_segments(message.topic.as_str());
    let source_description = format!("MQTT topic '{}'", message.topic);

    match payload_mode {
        MQTT_PAYLOAD_MODE_JSON => json_received_values(message.payload.as_slice(), path_segments, source_description),
        MQTT_PAYLOAD_MODE_RAW => Ok(vec![MqttReceivedValue {
            path_segments,
            payload: raw_payload_value(message.payload.as_slice()),
            source_description,
        }]),
        MQTT_PAYLOAD_MODE_TEXT => Ok(vec![MqttReceivedValue {
            path_segments,
            payload: text_payload_value(message.payload.as_slice()),
            source_description,
        }]),
        _ => auto_received_values(message.payload.as_slice(), path_segments, source_description),
    }
}

fn auto_received_values(
    payload: &[u8],
    path_segments: Vec<String>,
    source_description: String,
) -> Result<Vec<MqttReceivedValue>, String> {
    if payload_looks_like_json(payload) {
        if let Ok(values) = json_received_values(payload, path_segments.clone(), source_description.clone()) {
            return Ok(values);
        }
    }

    Ok(vec![MqttReceivedValue {
        path_segments,
        payload: text_payload_value(payload),
        source_description,
    }])
}

fn json_received_values(
    payload: &[u8],
    path_segments: Vec<String>,
    source_description: String,
) -> Result<Vec<MqttReceivedValue>, String> {
    let value: serde_json::Value =
        serde_json::from_slice(payload).map_err(|error| format!("invalid JSON payload: {error}"))?;
    let mut messages = Vec::new();
    let mut current_path = path_segments;
    collect_json_received_values(&value, &mut current_path, source_description.as_str(), &mut messages)?;
    Ok(messages)
}

fn collect_json_received_values(
    value: &serde_json::Value,
    path_segments: &mut Vec<String>,
    source_description: &str,
    messages: &mut Vec<MqttReceivedValue>,
) -> Result<(), String> {
    if let Some(payload) = decode_json_payload(value)? {
        messages.push(MqttReceivedValue {
            path_segments: path_segments.clone(),
            payload,
            source_description: source_description.to_string(),
        });
        return Ok(());
    }

    match value {
        serde_json::Value::Object(entries) => {
            for (key, child) in entries {
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                path_segments.push(key.to_string());
                collect_json_received_values(child, path_segments, source_description, messages)?;
                path_segments.pop();
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path_segments.push((index + 1).to_string());
                collect_json_received_values(child, path_segments, source_description, messages)?;
                path_segments.pop();
            }
        }
        _ => {}
    }

    Ok(())
}

fn decode_json_payload(value: &serde_json::Value) -> Result<Option<ReceivedValuePayload>, String> {
    match value {
        serde_json::Value::Object(_) => Ok(None),
        serde_json::Value::Array(items) => {
            if let Ok(value) = ParamValue::from_script_json(value) {
                return Ok(Some(ReceivedValuePayload::Single(value)));
            }

            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let Ok(value) = ParamValue::from_script_json(item) else {
                    return Ok(None);
                };
                values.push(value);
            }

            Ok(Some(match values.len() {
                0 => ReceivedValuePayload::Single(ParamValue::Trigger()),
                1 => ReceivedValuePayload::Single(values.remove(0)),
                _ => ReceivedValuePayload::Multi(values),
            }))
        }
        _ => ParamValue::from_script_json(value)
            .map(ReceivedValuePayload::Single)
            .map(Some)
            .map_err(|error| format!("invalid JSON value: {error}")),
    }
}

fn text_payload_value(payload: &[u8]) -> ReceivedValuePayload {
    if payload.is_empty() {
        return ReceivedValuePayload::Single(ParamValue::Trigger());
    }

    ReceivedValuePayload::Single(parse_scalar_value(String::from_utf8_lossy(payload).as_ref()))
}

fn raw_payload_value(payload: &[u8]) -> ReceivedValuePayload {
    if payload.is_empty() {
        return ReceivedValuePayload::Single(ParamValue::Trigger());
    }

    ReceivedValuePayload::Multi(
        payload
            .iter()
            .map(|byte| ParamValue::Int(i32::from(*byte)))
            .collect(),
    )
}

fn payload_looks_like_json(payload: &[u8]) -> bool {
    let trimmed = payload
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .collect::<Vec<_>>();
    matches!(trimmed.first(), Some(b'{') | Some(b'['))
}

fn mqtt_topic_segments(topic: &str) -> Vec<String> {
    let segments = topic
        .split('/')
        .enumerate()
        .map(|(index, segment)| {
            let segment = segment.trim();
            if segment.is_empty() {
                format!("level_{}", index + 1)
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>();

    if segments.is_empty() {
        vec!["message".to_string()]
    } else {
        segments
    }
}

fn mqtt_publish_request_from_script(method: &str, args: &[ParamValue]) -> Option<Result<MqttPublishRequest, String>> {
    match method {
        "publish" | "publishText" | "publishJson" => {}
        _ => return None,
    }

    let Some(topic) = args.first().and_then(ParamValue::as_str) else {
        return Some(Err(format!("method '{method}' expects an MQTT topic string")));
    };

    let payload = match method {
        "publishJson" => args
            .get(1)
            .map(ParamValue::to_script_json)
            .map(|value| serde_json::to_vec(&value).map_err(|error| format!("failed to encode JSON payload: {error}")))
            .transpose()
            .map(|payload| payload.unwrap_or_default()),
        _ => Ok(args
            .get(1)
            .and_then(ParamValue::as_str)
            .unwrap_or_default()
            .into_bytes()),
    };

    Some(payload.and_then(|payload| {
        Ok(MqttPublishRequest {
            topic,
            payload,
            qos: script_qos_arg(args.get(2))?,
            retain: args.get(3).and_then(ParamValue::as_bool).unwrap_or(false),
            description: method.to_string(),
        })
    }))
}

fn script_qos_arg(value: Option<&ParamValue>) -> Result<MqttQos, String> {
    let Some(value) = value else {
        return Ok(MqttQos::AtMost);
    };

    if let Some(value) = value.as_int() {
        return match value {
            0 => Ok(MqttQos::AtMost),
            1 => Ok(MqttQos::AtLeast),
            2 => Ok(MqttQos::Exactly),
            _ => Err(format!("MQTT QoS integer must be 0, 1, or 2, got {value}")),
        };
    }

    let variant = value
        .as_str()
        .ok_or_else(|| "MQTT QoS must be an integer or string".to_string())?;
    MqttQos::from_variant(variant.as_str()).ok_or_else(|| format!("invalid MQTT QoS '{variant}'"))
}

fn mqtt_payload_script_arg(payload: &[u8]) -> serde_json::Value {
    std::str::from_utf8(payload)
        .map(|text| serde_json::json!(text))
        .unwrap_or_else(|_| crate::app::module::script_api::bytes_arg(payload))
}

fn format_mqtt_payload_for_log(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "<empty>".to_string();
    }

    match std::str::from_utf8(payload) {
        Ok(text) if !text.chars().any(char::is_control) => format!("\"{text}\""),
        _ => payload
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn collect_subscriptions_recursive(snapshot: &ProcessTreeSnapshot, parent: NodeId, output: &mut Vec<NodeId>) {
    for child_id in snapshot.child_ids(parent) {
        let Some(child_snapshot) = snapshot.node(child_id) else {
            continue;
        };

        if child_snapshot.node_type == MQTT_SUBSCRIPTION_NODE_TYPE {
            output.push(child_id);
        } else if child_snapshot.node_type == "folder" {
            collect_subscriptions_recursive(snapshot, child_id, output);
        }
    }
}

fn child_string_param(snapshot: &ProcessTreeSnapshot, parent: NodeId, child_name: &str) -> Option<String> {
    snapshot.find_child_by_decl_id(parent, child_name).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

fn child_enum_param(snapshot: &ProcessTreeSnapshot, parent: NodeId, child_name: &str) -> Option<String> {
    snapshot.find_child_by_decl_id(parent, child_name).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
    })
}

fn is_descendant_or_self(snapshot: &ProcessTreeSnapshot, start: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(start);
    while let Some(node_id) = current {
        if node_id == ancestor {
            return true;
        }
        current = snapshot.node(node_id).and_then(|node| node.parent);
    }
    false
}

fn mqtt_payload_mode_options() -> Vec<golden_core::parameter::ParameterEnumOption> {
    [
        (MQTT_PAYLOAD_MODE_AUTO, "Auto (JSON or Text)"),
        (MQTT_PAYLOAD_MODE_TEXT, "Text"),
        (MQTT_PAYLOAD_MODE_JSON, "JSON"),
        (MQTT_PAYLOAD_MODE_RAW, "Raw Bytes"),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(ordering, (variant_id, label))| golden_core::parameter::ParameterEnumOption {
            variant_id: variant_id.to_string(),
            value: ParamValue::Enum(variant_id.to_string()),
            label: label.to_string(),
            tags: Vec::new(),
            ordering: Some(ordering as i32),
        },
    )
    .collect()
}

#[cfg(test)]
mod tests;
