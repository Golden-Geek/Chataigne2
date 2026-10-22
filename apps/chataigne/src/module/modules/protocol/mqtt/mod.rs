mod payload;
mod service;
mod transport;
use payload::*;

use std::collections::VecDeque;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::{
    mqtt::{mqtt_qos_enum_options, MqttPublishRequest, MqttQos, MQTT_PUBLISH_COMMAND_NODE_TYPE, MQTT_QOS_AT_MOST_ONCE},
    received_values::{
        apply_received_value_payload, ReceivedValueApplyOptions, ReceivedValueApplyResult, ReceivedValuePayload,
    },
    streaming::parser::parse_scalar_value,
};

use self::transport::{
    MqttConnectionStatus, MqttCredentials, MqttReceivedPublish, MqttSubscriptionConfig, MqttTransportConfig,
    MqttTransportHandle, MqttWorkerEvent, PublishReceipt, PublishStatus,
};

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
    pending_incoming_messages: Vec<golden_io::RetainedPayload<MqttReceivedPublish>>,
    pending_publishes: VecDeque<(PublishReceipt, String)>,
    wake: Option<crate::app::module::common::worker_wake::ModuleWake>,
    retry_at: Option<std::time::Instant>,
    retry_backoff: golden_io::ReconnectBackoff,
}

impl MqttModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            None,
            None,
            true,
            Vec::new(),
            VecDeque::new(),
            None,
            None,
            golden_io::ReconnectBackoff::new(std::time::Duration::from_millis(250), std::time::Duration::from_secs(8)),
        )
    }

    #[cfg(test)]
    pub(crate) fn disable_transport_for_test(&mut self) {
        self.stop_transport();
        self.transport_dirty = false;
        self.retry_at = None;
    }

    #[cfg(test)]
    pub(crate) fn enqueue_incoming_message_for_test(&mut self, message: MqttReceivedPublish) {
        self.pending_incoming_messages
            .push(MqttTransportHandle::retain_for_test(message));
        self.request_service();
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
        self.retry_at = None;

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

        let Some(wake) = &self.wake else { return };
        let notifier = wake.notifier.clone();
        match MqttTransportHandle::spawn(config.clone(), self.id(), notifier) {
            Ok(handle) => {
                self.transport = Some(handle);
                self.last_transport_config = Some(config);
                self.retry_backoff.reset();
                self.clear_target_warning(ctx);
                self.base.set_connected(ctx, false);
            }
            Err(error) => {
                logerror!("Failed to start MQTT transport: {}", error);
                self.transport = None;
                self.last_transport_config = None;
                self.set_target_warning(ctx, error.as_str());
                self.base.set_connected(ctx, false);
                self.schedule_retry();
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

        let qos_variant =
            child_enum_param(snapshot, subscription_id, "qos").unwrap_or_else(|| MQTT_QOS_AT_MOST_ONCE.to_string());
        let qos = MqttQos::from_variant(qos_variant.as_str())
            .ok_or_else(|| format!("invalid MQTT subscription QoS '{qos_variant}'"))?;

        Ok(Some(MqttSubscriptionConfig { topic_filter, qos }))
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..self.pending_publishes.len().min(transport::EVENT_TURN) {
            let (receipt, description) = self.pending_publishes.pop_front().unwrap();
            match receipt.status() {
                PublishStatus::Pending => self.pending_publishes.push_back((receipt, description)),
                PublishStatus::Acknowledged | PublishStatus::DispatchedUnconfirmed => {
                    self.base.emit_outgoing_traffic(ctx)
                }
                PublishStatus::FailedUnconfirmed => {
                    let message = format!(
                        "MQTT {description} failed or was cancelled; delivery is unconfirmed and is not replayed"
                    );
                    self.set_target_warning(ctx, &message);
                    logerror!("{}", message);
                }
            }
        }
        let (worker_events, worker_disconnected) = {
            let Some(transport) = &self.transport else {
                return;
            };

            let mut worker_events = Vec::new();
            let mut worker_disconnected = false;
            for _ in 0..transport::EVENT_TURN {
                if self.pending_incoming_messages.len() + worker_events.len() >= 64 {
                    break;
                }
                let drain = transport.drain_events(&mut worker_events);
                if drain.state == golden_io::PendingDrainState::Disconnected {
                    worker_disconnected = true;
                    break;
                }
                if drain.received == 0 {
                    break;
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
            self.transport_dirty = false;
            self.schedule_retry();
        }

        if received_message {
            self.base.emit_incoming_traffic(ctx);
        }
        if self
            .transport
            .as_ref()
            .is_some_and(|transport| !transport.is_connected())
        {
            self.base.set_connected(ctx, false);
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

        // At most one admitted message per actor turn. Native payload preparation
        // and receipt-driven row publication remain part of the C04 actor audit.
        for _ in 0..1 {
            let Some(message) = messages.next() else { break };
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
                    ReceivedValueApplyResult::Applied { needs_snapshot_refresh } => {
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

        remaining.extend(messages);
        self.pending_incoming_messages = remaining;
        false
    }

    fn queue_publish_request(&mut self, _ctx: &mut ProcessCtx, request: MqttPublishRequest) -> Result<String, String> {
        let topic = request.topic.trim();
        if !rumqttc::valid_topic(topic) || topic.is_empty() {
            return Err(format!("invalid MQTT publish topic '{}'", request.topic));
        }

        if self.pending_publishes.len() >= 100 || request.description.capacity() > 256 {
            return Err("MQTT pending publication capacity exhausted".into());
        }
        let description = request.description.clone();

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| "MQTT transport is not available".to_string())?;
        let receipt = transport.send(request, self.base.log_outgoing_enabled())?;
        self.pending_publishes.push_back((receipt, description.clone()));

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
            self.retry_at = None;
            self.retry_backoff.reset();
            self.request_service();
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
            self.request_service();
        }
    }

    fn on_effective_enabled_changed_inner(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.transport_dirty = true;
            self.request_service();
        } else {
            self.stop_transport();
            self.last_transport_config = None;
            self.clear_target_warning(ctx);
            self.base.set_connected(ctx, false);
            self.transport_dirty = false;
            self.retry_at = None;
            self.pending_incoming_messages.clear();
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
        if let Err(error) = self.register_service(ctx) {
            self.set_target_warning(ctx, &error);
        }
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
        if let Err(error) = self.register_service(ctx) {
            self.set_target_warning(ctx, &error);
            return;
        }
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.refresh_transport(ctx, snapshot);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_transport();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.service_needs_snapshot()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive()
    }

    fn needs_update(&self) -> bool {
        false
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.service_needs_snapshot()
    }
    fn worker_wake_when_disabled(&self) -> bool {
        self.transport.is_some() || !self.pending_publishes.is_empty()
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), MQTT_SCRIPT_METHODS)
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
        self.base.emit_script_param_callback(ctx, snapshot, param, &old_value);
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
