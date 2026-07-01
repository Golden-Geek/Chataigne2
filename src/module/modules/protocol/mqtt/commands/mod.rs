use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::{
        mqtt::{mqtt_qos_enum_options, MqttPublishRequest, MqttQos, MQTT_PUBLISH_COMMAND_NODE_TYPE},
        streaming::parser::decode_text_escape_sequences,
    },
    module_command,
};

#[node("mqtt_publish_command", label = "Publish")]
#[children(
    topic: String = "chataigne/topic".to_string() (
        label = "Topic",
        description = "MQTT topic to publish to. Publish topics cannot contain MQTT wildcards."
    );
    payload: String = String::new() (
        label = "Payload",
        description = "UTF-8 payload to publish. Escape sequences such as \\n, \\r, \\t, and \\xNN are supported.",
        widget = "textarea"
    );
    qos: Enum = crate::app::module::common::mqtt::MQTT_QOS_AT_MOST_ONCE (
        label = "QoS",
        description = "MQTT quality of service used for this publish.",
        enum_options = mqtt_qos_enum_options()
    );
    retain: bool = false (
        label = "Retain",
        description = "Whether the broker should retain this message as the latest value for the topic."
    );
)]
pub struct MqttPublishCommand {
    base: crate::app::ModuleCommandBase,
}

impl MqttPublishCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MqttPublishRequest, String> {
        let topic = command_string_param(snapshot, self.id(), "topic").unwrap_or_default();
        let payload_text = command_string_param(snapshot, self.id(), "payload").unwrap_or_default();
        let qos_variant = command_enum_param(snapshot, self.id(), "qos")
            .unwrap_or_else(|| crate::app::module::common::mqtt::MQTT_QOS_AT_MOST_ONCE.to_string());
        let qos = MqttQos::from_variant(qos_variant.as_str())
            .ok_or_else(|| format!("invalid MQTT QoS variant '{qos_variant}'"))?;

        Ok(MqttPublishRequest {
            topic,
            payload: decode_text_escape_sequences(payload_text.as_str()).into_bytes(),
            qos,
            retain: command_bool_param(snapshot, self.id(), "retain").unwrap_or(false),
            description: "publish".to_string(),
        })
    }
}

#[golden_core::item(
    "module_command",
    node = "mqtt_publish_command",
    via = base,
    from_struct
)]
impl Node for MqttPublishCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == MQTT_PUBLISH_COMMAND_NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ParamChanged { .. } => u32::MAX,
            _ => 0,
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        if !module_command::module_command_triggered(snapshot, self.id(), param) {
            return;
        }

        if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
            module_command::emit_module_command_request(
                ctx,
                snapshot,
                self.id(),
                self.get_type(),
                &payload,
            )
        }) {
            golden_core::logerror!(format!("Failed to trigger MQTT publish command: {error}"));
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if !module_command::is_command_execute_request(&event, self.id()) {
            return;
        }
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
            module_command::emit_module_command_request(
                ctx,
                snapshot,
                self.id(),
                self.get_type(),
                &payload,
            )
        }) {
            golden_core::logerror!(format!("Failed to execute MQTT publish command: {error}"));
        }
    }
}

fn command_string_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

fn command_enum_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
    })
}

fn command_bool_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<bool> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_bool)
    })
}

#[cfg(test)]
mod tests;
