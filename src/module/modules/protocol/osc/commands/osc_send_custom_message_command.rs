use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
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

    fn request_payload(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Result<crate::app::OscSendCustomMessageRequest, String> {
        let address = command_string_param(snapshot, self.id(), "address")
            .ok_or_else(|| "missing OSC command address 'address'".to_string())?;
        if address.trim().is_empty() {
            return Err("OSC address cannot be empty".to_string());
        }

        let arguments_id = crate::app::module_command::resolve_module_command_child(
            snapshot,
            self.id(),
            "arguments",
        )
        .ok_or_else(|| "missing OSC command arguments folder 'arguments'".to_string())?;

        let mut arguments = Vec::new();
        for child_id in snapshot.child_ids(arguments_id) {
            let child = snapshot
                .node(child_id)
                .ok_or_else(|| "OSC command argument disappeared while collecting values".to_string())?;
            let value = child
                .param_value
                .clone()
                .ok_or_else(|| format!("OSC command argument '{}' is not a parameter", child.label))?;
            arguments.push(value);
        }

        Ok(crate::app::OscSendCustomMessageRequest { address, arguments })
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

        if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
            crate::app::module_command::emit_module_command_request(ctx, snapshot, self.id(), self.get_type(), &payload)
        }) {
            golden_core::logerror!(format!("Failed to trigger OSC command: {error}"));
        }
    }
}

fn command_string_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

#[cfg(test)]
mod osc_send_custom_message_command_tests;
