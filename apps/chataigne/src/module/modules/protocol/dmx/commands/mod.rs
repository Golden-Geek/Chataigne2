use golden_core::{
    events::{CustomEvent, Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

use crate::app::module_modules_protocol_dmx::parse_slots_json;

pub(crate) const DMX_SET_CHANNEL_COMMAND_NODE_TYPE: &str = "dmx_set_channel_command";
pub(crate) const DMX_SEND_FRAME_COMMAND_NODE_TYPE: &str = "dmx_send_frame_command";
pub(crate) const DMX_BLACKOUT_COMMAND_NODE_TYPE: &str = "dmx_blackout_command";
pub(crate) const DMX_COMMAND_TYPES: &[&str] = &[
    DMX_SET_CHANNEL_COMMAND_NODE_TYPE,
    DMX_SEND_FRAME_COMMAND_NODE_TYPE,
    DMX_BLACKOUT_COMMAND_NODE_TYPE,
];

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(crate) enum DmxCommandRequest {
    SetChannel { channel: u16, value: u8 },
    SendFrame { slots: Vec<u8> },
    Blackout,
}

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

    fn request(&self, snapshot: &ProcessTreeSnapshot) -> Result<DmxCommandRequest, String> {
        let channel = command_int(snapshot, self.id(), "channel")
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (1..=512).contains(value))
            .ok_or_else(|| "DMX channel must be between 1 and 512".to_string())?;
        let value = command_int(snapshot, self.id(), "value")
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| "DMX value must be between 0 and 255".to_string())?;
        Ok(DmxCommandRequest::SetChannel { channel, value })
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
            run_command(ctx, self.id(), self.get_type(), |snapshot| self.request(snapshot));
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if crate::app::module_command::is_command_execute_request(&event, self.id()) {
            run_command_with_event(ctx, self.id(), self.get_type(), &event, |snapshot| {
                self.request(snapshot)
            });
        }
    }
}

#[node("dmx_send_frame_command", label = "Send DMX Frame")]
#[children(
    slots: String = "[]".to_string() (
        label = "Channels",
        description = "JSON array containing up to 512 channel values in the 0..255 range."
    );
)]
pub struct DmxSendFrameCommand {
    base: crate::app::ModuleCommandBase,
}

impl DmxSendFrameCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request(&self, snapshot: &ProcessTreeSnapshot) -> Result<DmxCommandRequest, String> {
        let slots = command_string(snapshot, self.id(), "slots")
            .ok_or_else(|| "missing DMX frame channels".to_string())
            .and_then(|value| parse_slots_json(value.as_str()))?;
        Ok(DmxCommandRequest::SendFrame { slots })
    }
}

#[golden_core::item(
    "module_command",
    node = "dmx_send_frame_command",
    via = base,
    from_struct
)]
impl Node for DmxSendFrameCommand {
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
            run_command(ctx, self.id(), self.get_type(), |snapshot| self.request(snapshot));
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if crate::app::module_command::is_command_execute_request(&event, self.id()) {
            run_command_with_event(ctx, self.id(), self.get_type(), &event, |snapshot| {
                self.request(snapshot)
            });
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
            run_command(ctx, self.id(), self.get_type(), |_| {
                Ok(DmxCommandRequest::Blackout)
            });
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if crate::app::module_command::is_command_execute_request(&event, self.id()) {
            run_command_with_event(ctx, self.id(), self.get_type(), &event, |_| {
                Ok(DmxCommandRequest::Blackout)
            });
        }
    }
}

fn command_triggered(ctx: &ProcessCtx, command_id: NodeId, param: NodeId) -> bool {
    ctx.tree_snapshot()
        .is_some_and(|snapshot| crate::app::module_command::module_command_triggered(snapshot, command_id, param))
}

fn run_command(
    ctx: &mut ProcessCtx,
    command_id: NodeId,
    command_type: &str,
    request: impl FnOnce(&ProcessTreeSnapshot) -> Result<DmxCommandRequest, String>,
) {
    let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
        return;
    };
    let snapshot = snapshot_arc.as_ref();
    if let Err(error) = request(snapshot).and_then(|request| {
        crate::app::module_command::emit_module_command_request(
            ctx,
            snapshot,
            command_id,
            command_type,
            &request,
        )
    }) {
        golden_core::logerror!(format!("Failed to trigger DMX command: {error}"));
    }
}

fn run_command_with_event(
    ctx: &mut ProcessCtx,
    command_id: NodeId,
    command_type: &str,
    event: &CustomEvent,
    request: impl FnOnce(&ProcessTreeSnapshot) -> Result<DmxCommandRequest, String>,
) {
    let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
        return;
    };
    let snapshot = crate::app::module_command::command_execute_snapshot(
        event,
        snapshot_arc.as_ref(),
        command_id,
    );
    if let Err(error) = request(snapshot.as_ref()).and_then(|request| {
        crate::app::module_command::emit_module_command_request(
            ctx,
            snapshot.as_ref(),
            command_id,
            command_type,
            &request,
        )
    }) {
        golden_core::logerror!(format!("Failed to trigger DMX command: {error}"));
    }
}

fn command_int(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<i32> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param| snapshot.node(param))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_int)
}

fn command_string(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param| snapshot.node(param))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_str)
}

#[cfg(test)]
mod tests;
