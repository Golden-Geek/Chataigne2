use golden_core::{
    events::{CustomEvent, Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{NodeReference, ParamValue, ReferenceTargetKind},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

pub(crate) const NODE_SET_VALUE_COMMAND_NODE_TYPE: &str = "node_set_value_command";
pub(crate) const NODE_TRIGGER_COMMAND_NODE_TYPE: &str = "node_trigger_command";
pub(crate) const NODE_COMMAND_TYPES: &[&str] = &[
    NODE_SET_VALUE_COMMAND_NODE_TYPE,
    NODE_TRIGGER_COMMAND_NODE_TYPE,
];

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(crate) enum NodeControlRequest {
    SetValue { target: NodeId, value: ParamValue },
    Trigger { target: NodeId },
}

#[node("node_set_value_command", label = "Set Node Value")]
#[children(
    target: NodeReference = NodeReference::default() (
        label = "Target",
        description = "Parameter whose value will be updated.",
        reference_target_kind = ReferenceTargetKind::ParameterOnly
    );
    value: ParamValue = ParamValue::Bool(false) (
        label = "Value",
        description = "Value converted and written through the target parameter's constraints."
    );
)]
pub struct NodeSetValueCommand {
    base: crate::app::ModuleCommandBase,
}

impl NodeSetValueCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request(&self, snapshot: &ProcessTreeSnapshot) -> Result<NodeControlRequest, String> {
        let target = command_reference(snapshot, self.id(), "target")
            .ok_or_else(|| "Set Node Value requires a valid target parameter".to_string())?;
        let value = command_value(snapshot, self.id(), "value")
            .ok_or_else(|| "Set Node Value requires a value".to_string())?;
        Ok(NodeControlRequest::SetValue { target, value })
    }
}

#[golden_core::item(
    "module_command",
    node = "node_set_value_command",
    via = base,
    from_struct
)]
impl Node for NodeSetValueCommand {
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

#[node("node_trigger_command", label = "Trigger Node")]
#[children(
    target: NodeReference = NodeReference::default() (
        label = "Target",
        description = "Trigger parameter that will receive a new trigger edge.",
        reference_target_kind = ReferenceTargetKind::ParameterOnly
    );
)]
pub struct NodeTriggerCommand {
    base: crate::app::ModuleCommandBase,
}

impl NodeTriggerCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request(&self, snapshot: &ProcessTreeSnapshot) -> Result<NodeControlRequest, String> {
        command_reference(snapshot, self.id(), "target")
            .map(|target| NodeControlRequest::Trigger { target })
            .ok_or_else(|| "Trigger Node requires a valid target parameter".to_string())
    }
}

#[golden_core::item(
    "module_command",
    node = "node_trigger_command",
    via = base,
    from_struct
)]
impl Node for NodeTriggerCommand {
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

fn command_reference(
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    path: &str,
) -> Option<NodeId> {
    let param = crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)?;
    let reference = snapshot
        .node(param)?
        .param_value
        .as_ref()
        .and_then(|value| match value {
            ParamValue::Reference(reference) => Some(reference),
            _ => None,
        })?;
    reference
        .cached_id()
        .filter(|target| {
            snapshot
                .node(*target)
                .is_some_and(|node| node.uuid == reference.uuid())
        })
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
}

fn command_value(
    snapshot: &ProcessTreeSnapshot,
    command_id: NodeId,
    path: &str,
) -> Option<ParamValue> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param| snapshot.node(param))
        .and_then(|node| node.param_value.clone())
}

fn command_triggered(ctx: &ProcessCtx, command_id: NodeId, param: NodeId) -> bool {
    ctx.tree_snapshot()
        .is_some_and(|snapshot| crate::app::module_command::module_command_triggered(snapshot, command_id, param))
}

fn run_command(
    ctx: &mut ProcessCtx,
    command_id: NodeId,
    command_type: &str,
    request: impl FnOnce(&ProcessTreeSnapshot) -> Result<NodeControlRequest, String>,
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
        golden_core::logerror!(format!("Failed to trigger Node command: {error}"));
    }
}

fn run_command_with_event(
    ctx: &mut ProcessCtx,
    command_id: NodeId,
    command_type: &str,
    event: &CustomEvent,
    request: impl FnOnce(&ProcessTreeSnapshot) -> Result<NodeControlRequest, String>,
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
        golden_core::logerror!(format!("Failed to trigger Node command: {error}"));
    }
}

#[cfg(test)]
mod command_tests;
