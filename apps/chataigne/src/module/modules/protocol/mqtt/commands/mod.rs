use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue},
    process_ctx::ProcessCtx,
};

use crate::app::{
    module::common::mqtt::{mqtt_qos_enum_options, MQTT_PUBLISH_COMMAND_NODE_TYPE},
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

    fn run(&self, ctx: &mut ProcessCtx, event: Option<golden_core::events::CustomEvent>) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let result = module_command::CapturedInvocation::capture(&snapshot, self.id(), event, &["mqtt_module"])
            .and_then(|invocation| crate::app::queue_mqtt_command(ctx, invocation));
        if let Err(error) = result {
            golden_core::logerror!(format!("MQTT command was not admitted: {error}"));
        }
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

        self.run(ctx, None);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if event.topic != module_command::MODULE_COMMAND_EXECUTE_TOPIC
            || event.payload.get("command_id").and_then(serde_json::Value::as_u64) != Some(self.id().0)
        {
            return;
        }
        self.run(ctx, Some(event));
    }
}

#[cfg(test)]
mod tests;
