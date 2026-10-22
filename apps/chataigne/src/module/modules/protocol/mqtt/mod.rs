mod configuration;
mod input;
mod output;
pub(crate) use output::dispatch as queue_mqtt_command;
mod payload;
mod service;
mod transport;
use payload::*;

use std::collections::VecDeque;

use golden_core::{
    engine::NodeExecutionRule,
    events::Event,
    logerror, node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId, NodeScriptDescriptor},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::mqtt::{mqtt_qos_enum_options, MQTT_PUBLISH_COMMAND_NODE_TYPE, MQTT_QOS_AT_MOST_ONCE};

use self::transport::{
    MqttConnectionStatus, MqttTransportConfig, MqttTransportHandle, MqttWorkerEvent, PublishReceipt, PublishStatus,
};

const MQTT_TARGET_WARNING_ID: &str = "mqtt_target_transport";
const MQTT_SUBSCRIPTION_NODE_TYPE: &str = "mqtt_subscription";
const MQTT_SUBSCRIPTIONS_DECL_ID: &str = "subscriptions";
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

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        if context == NodeCreationContext::Fresh && self.node_data().first_child.is_none() {
            ctx.add_user_item_boxed(self.id(), Box::new(MqttSubscription::new()), None);
        }
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
    last_transport_config: Option<golden_io::RetainedPayload<MqttTransportConfig>>,
    configuration: Option<configuration::Preparation>,
    configuration_key: Option<configuration::Key>,
    transport_dirty: bool,
    incoming: input::Inbox,
    pending_publishes: VecDeque<(PublishReceipt, &'static str)>,
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
            None,
            None,
            true,
            input::Inbox::default(),
            VecDeque::new(),
            None,
            None,
            golden_io::ReconnectBackoff::new(std::time::Duration::from_millis(250), std::time::Duration::from_secs(8)),
        )
    }

    #[cfg(test)]
    pub(crate) fn disable_transport_for_test(&mut self, snapshot: &ProcessTreeSnapshot) {
        self.configuration = None;
        self.configuration_key = configuration::Key::capture(snapshot, self.id());
        self.stop_transport();
        self.transport_dirty = false;
        self.retry_at = None;
    }

    #[cfg(test)]
    pub(crate) fn enqueue_incoming_message_for_test(&mut self, message: transport::MqttReceivedPublish) {
        self.incoming.activate();
        self.incoming.push(MqttTransportHandle::retain_for_test(message));
        self.request_service();
    }

    #[cfg(test)]
    pub(crate) fn has_pending_incoming_messages_for_test(&self) -> bool {
        !self.incoming.is_empty()
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn drain_transport_events(&mut self, ctx: &mut ProcessCtx) {
        for _ in 0..self.pending_publishes.len().min(transport::EVENT_TURN) {
            let (receipt, description) = self.pending_publishes.pop_front().unwrap();
            match receipt.status() {
                PublishStatus::Pending => self.pending_publishes.push_back((receipt, description)),
                PublishStatus::Acknowledged | PublishStatus::DispatchedUnconfirmed => {
                    self.base.emit_outgoing_traffic(ctx)
                }
                PublishStatus::Rejected => {
                    let message = format!(
                        "MQTT {description} was not sent: {}",
                        receipt.failure().unwrap_or("native preparation rejected the request")
                    );
                    self.set_target_warning(ctx, &message);
                    logerror!(message);
                }
                PublishStatus::FailedUnconfirmed => {
                    let message = format!(
                        "MQTT {description} failed or was cancelled; delivery is unconfirmed and is not replayed"
                    );
                    self.set_target_warning(ctx, &message);
                    logerror!(message);
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
                if self.incoming.len() + worker_events.len() >= 64 {
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
                    self.incoming.push(message);
                }
                MqttWorkerEvent::Status(status) => match status {
                    MqttConnectionStatus::Connected => {
                        if let Some(config) = &self.last_transport_config {
                            golden_core::logsuccess!(
                                origin = self.id();
                                format!("Connected MQTT broker {}:{}.", config.remote_host, config.remote_port)
                            );
                        }
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

    fn queue_output_request(&mut self, _ctx: &mut ProcessCtx, request: output::Request) -> Result<(), String> {
        if !self.node_data().effective_enabled || self.pending_publishes.len() >= 100 {
            return Err("MQTT module is disabled or its pending publication capacity is exhausted".into());
        }
        let description = request.description();
        let transport = self.transport.as_ref().ok_or("MQTT transport is not available")?;
        let receipt = transport.send_output(request, self.base.log_outgoing_enabled())?;
        self.pending_publishes.push_back((receipt, description));
        Ok(())
    }

    fn handle_script_publish_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        output::Request::script(ctx, method, args)
            .map(|request| request.and_then(|request| self.queue_output_request(ctx, request)))
    }

    fn on_effective_enabled_changed_inner(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.configuration = None;
        self.configuration_key = None;
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
            self.incoming.suspend();
        }
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
        self.incoming.suspend();
        if let Some(mut transport) = self.transport.take() {
            transport.stop();
        }
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
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if let Err(error) = self.register_service(ctx) {
            self.set_target_warning(ctx, &error);
            return;
        }
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        self.refresh_transport(ctx, &snapshot_arc);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.service_io(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.configuration = None;
        self.configuration_key = None;
        self.wake = None;
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
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.dispatch_inbox(ctx);
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.notice_configuration(&snapshot);
            let id = self.id();
            self.incoming.notice_processing(&snapshot, id);
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.on_effective_enabled_changed_inner(ctx, enabled);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
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
