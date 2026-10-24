use golden_core::{
    events::{CustomEvent, Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{ByteArray, ParamValue},
    process_ctx::ProcessCtx,
};

pub(crate) const DMX_SET_CHANNEL_COMMAND_NODE_TYPE: &str = "dmx_set_channel_command";
pub(crate) const DMX_SET_CHANNELS_COMMAND_NODE_TYPE: &str = "dmx_set_channels_command";
pub(crate) const DMX_SEND_FRAME_COMMAND_NODE_TYPE: &str = "dmx_send_frame_command";
pub(crate) const DMX_BLACKOUT_COMMAND_NODE_TYPE: &str = "dmx_blackout_command";
pub(crate) const DMX_COMMAND_TYPES: &[&str] = &[
    DMX_SET_CHANNEL_COMMAND_NODE_TYPE,
    DMX_SET_CHANNELS_COMMAND_NODE_TYPE,
    DMX_SEND_FRAME_COMMAND_NODE_TYPE,
    DMX_BLACKOUT_COMMAND_NODE_TYPE,
];

#[node("dmx_set_channel_command", label = "Set DMX Channel")]
#[children(
    channel: i32 = 1 [1..512] (
        label = "Channel",
        description = "One-based DMX channel to update."
    );
    value: i32 = 0 [0..255] (
        label = "Value",
        description = "Eight-bit DMX level written to the selected channel."
    );
)]
pub struct DmxSetChannelCommand {
    base: crate::app::ModuleCommandBase,
}

impl DmxSetChannelCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "dmx_set_channel_command",
    via = base,
    from_struct
)]
impl Node for DmxSetChannelCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if command_triggered(ctx, self.id(), param) {
            run_command(ctx, self.id(), None);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic == crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC
            && event.payload.get("command_id").and_then(serde_json::Value::as_u64) == Some(self.id().0)
        {
            run_command(ctx, self.id(), Some(event));
        }
    }
}

#[node("dmx_send_frame_command", label = "Send DMX Frame")]
#[children(
    slots: ByteArray = ByteArray::default() (
        label = "Channels",
        description = "Byte array containing up to 512 channel values in the 0..255 range."
    );
)]
pub struct DmxSendFrameCommand {
    base: crate::app::ModuleCommandBase,
}

impl DmxSendFrameCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "dmx_send_frame_command",
    via = base,
    from_struct
)]
impl Node for DmxSendFrameCommand {
    fn project_migrate_children<'a>(
        &self,
        children: &'a [golden_core::engine::ProjectNodeRecord],
    ) -> Result<std::borrow::Cow<'a, [golden_core::engine::ProjectNodeRecord]>, String> {
        crate::app::module_modules_protocol_dmx::migrate_command_children(children)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if command_triggered(ctx, self.id(), param) {
            run_command(ctx, self.id(), None);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic == crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC
            && event.payload.get("command_id").and_then(serde_json::Value::as_u64) == Some(self.id().0)
        {
            run_command(ctx, self.id(), Some(event));
        }
    }
}

#[node("dmx_set_channels_command", label = "Set DMX Channels")]
#[children(
    channel: i32 = 1 [1..512] (
        label = "Starting Channel",
        description = "One-based first channel of the contiguous update."
    );
    slots: ByteArray = ByteArray::default() (
        label = "Channels",
        description = "Byte levels written together in one universe frame, starting at the selected channel."
    );
)]
pub struct DmxSetChannelsCommand {
    base: crate::app::ModuleCommandBase,
}

impl DmxSetChannelsCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "dmx_set_channels_command", via = base, from_struct)]
impl Node for DmxSetChannelsCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if command_triggered(ctx, self.id(), param) {
            run_command(ctx, self.id(), None);
        }
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic == crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC
            && event.payload.get("command_id").and_then(serde_json::Value::as_u64) == Some(self.id().0)
        {
            run_command(ctx, self.id(), Some(event));
        }
    }
}

#[node("dmx_blackout_command", label = "Blackout DMX")]
pub struct DmxBlackoutCommand {
    base: crate::app::ModuleCommandBase,
}

impl DmxBlackoutCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item(
    "module_command",
    node = "dmx_blackout_command",
    via = base,
    from_struct
)]
impl Node for DmxBlackoutCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if command_triggered(ctx, self.id(), param) {
            run_command(ctx, self.id(), None);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic == crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC
            && event.payload.get("command_id").and_then(serde_json::Value::as_u64) == Some(self.id().0)
        {
            run_command(ctx, self.id(), Some(event));
        }
    }
}

fn command_triggered(ctx: &ProcessCtx, command_id: NodeId, param: NodeId) -> bool {
    ctx.tree_snapshot()
        .is_some_and(|snapshot| crate::app::module_command::module_command_triggered(snapshot, command_id, param))
}

fn run_command(ctx: &mut ProcessCtx, command_id: NodeId, event: Option<CustomEvent>) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    let result = crate::app::module_command::CapturedInvocation::capture(
        &snapshot,
        command_id,
        event,
        &["artnet_module", "sacn_module"],
    )
    .and_then(|invocation| crate::app::queue_dmx_command(ctx, invocation));
    if let Err(error) = result {
        golden_core::logerror!(format!("DMX command was not admitted: {error}"));
    }
}

#[cfg(test)]
mod tests;
