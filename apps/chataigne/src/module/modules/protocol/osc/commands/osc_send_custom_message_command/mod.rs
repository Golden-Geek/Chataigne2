use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

pub(crate) const OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE: &str = "osc_send_custom_message_command";
const OSC_SEND_CUSTOM_MESSAGE_DEFAULT_ADDRESS: &str = "/custom";

#[node("osc_send_custom_message_command", label = "Send Custom Message")]
#[children(
    address: String = OSC_SEND_CUSTOM_MESSAGE_DEFAULT_ADDRESS.to_string() (
        label = "Address",
        description = "OSC address pattern to send when this command runs."
    );
    node arguments: crate::app::OscCommandArguments = crate::app::OscCommandArguments::create() (
        label = "Arguments",
        description = "OSC arguments appended in child order. Vector-like values expand into multiple OSC arguments.",
        collapsed = true
    );
)]
pub struct OscSendCustomMessageCommand {
    base: crate::app::ModuleCommandBase,
}

impl OscSendCustomMessageCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "osc_send_custom_message_command", via = base, from_struct)]
impl Node for OscSendCustomMessageCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE).then(Self::create)
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
        if !crate::app::module_command::module_command_triggered(snapshot, self.id(), param) {
            return;
        }
        self.run(ctx, None);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if event.topic == crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC
            && event.payload.get("command_id").and_then(serde_json::Value::as_u64) == Some(self.id().0)
        {
            self.run(ctx, Some(event));
        }
    }
}

impl OscSendCustomMessageCommand {
    fn run(&self, ctx: &mut ProcessCtx, event: Option<golden_core::events::CustomEvent>) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let result =
            crate::app::module_command::CapturedInvocation::capture(&snapshot, self.id(), event, &["osc_module"])
                .and_then(|invocation| crate::app::queue_osc_command(ctx, invocation));
        if let Err(error) = result {
            golden_core::logerror!(format!("OSC command was not admitted: {error}"));
        }
    }
}

#[cfg(test)]
mod tests;
