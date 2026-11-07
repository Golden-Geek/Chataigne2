//! Real Parameter pulses enter the same bounded app-operation service as script commands.
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{MediaEditResult, MediaId, MediaKind},
};
use golden_core::{
    events::CustomEvent,
    node::{Node, NodeId, NodeUuid},
    process_ctx::ProcessCtx,
    script::{APP_OPERATION_RESULT_TOPIC, ScriptAppOperationRequest},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub(crate) const NAMESPACE: &str = "mapgyver.matrix.control";

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(crate) enum Action {
    Play,
    Cut,
    StopAll,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    owner: NodeUuid,
    action: Action,
}

#[derive(Default)]
pub(crate) struct Pulses {
    ready: bool,
    sequence: u64,
    owner: Arc<()>,
}
impl Pulses {
    pub(crate) fn ready(&mut self) {
        self.ready = true;
    }
    pub(crate) fn revoke(&mut self) {
        self.owner = Arc::new(());
    }
    pub(crate) fn destroy(&mut self) {
        self.revoke();
        self.ready = false;
    }
    pub(crate) fn invoke(&mut self, ctx: &mut ProcessCtx, node: &golden_core::node::NodeData, action: Action) {
        if !self.ready || !node.effective_enabled {
            return;
        }
        let result = (|| {
            self.sequence = self
                .sequence
                .checked_add(1)
                .ok_or("Matrix control sequence exhausted")?;
            let request_id = format!("matrix-{}-{}", node.meta.uuid.0, self.sequence);
            let reservation = ScriptAppOperationRequest::reserve(&request_id, NAMESPACE, "invoke")?;
            let payload = serde_json::to_value(Invocation {
                owner: node.meta.uuid,
                action,
            })
            .map_err(|error| error.to_string())?;
            let request = reservation.finish(payload).map_err(|error| error.to_string())?;
            ctx.enqueue_app_operation(node.id, Arc::downgrade(&self.owner), request);
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            ctx.set_node_warning_with(node.id, Some("matrix-control"), error, None);
        }
    }
    pub(crate) fn result(ctx: &mut ProcessCtx, node: NodeId, event: &CustomEvent) {
        if event.topic != APP_OPERATION_RESULT_TOPIC || event.origin != Some(node) {
            return;
        }
        let reply = &event.payload["args"][0];
        if reply["status"] == "rejected" {
            ctx.set_node_warning_with(
                node,
                Some("matrix-control"),
                reply["message"].as_str().unwrap_or("Matrix control rejected"),
                None,
            );
        } else {
            ctx.clear_node_warning(node, Some("matrix-control"));
        }
    }
}

pub(crate) fn apply(
    engine: &mut AppEngine,
    operation: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    if operation != "invoke" {
        return Err("unknown Matrix control operation".into());
    }
    let invocation: Invocation = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let id = engine
        .node_id_by_uuid(invocation.owner)
        .ok_or("Matrix control owner is missing")?;
    let node = engine.nodes.get(id).ok_or("Matrix control owner is missing")?;
    if !node.node_data().effective_enabled {
        return Err("Matrix control owner is disabled".into());
    }
    let mut ancestor = Some(id);
    let mut media = None;
    while let Some(parent) = ancestor {
        let node = engine.nodes.get(parent).ok_or("Matrix control ancestry changed")?;
        if let AppNode::Media(owner) = node {
            if owner.kind() == MediaKind::Matrix {
                media = Some(MediaId(node.node_data().meta.uuid));
            }
            // A Cell's nested media is a separate playback domain.
            break;
        }
        ancestor = node.node_data().parent;
    }
    let media = media.ok_or("control is outside a Matrix")?;
    let mode = match invocation.action {
        Action::Play => super::protocol::LaunchMode::Configured,
        Action::Cut => super::protocol::LaunchMode::Cut,
        Action::StopAll => super::protocol::LaunchMode::Cut,
    };
    let result: MediaEditResult = match (node, invocation.action) {
        (AppNode::Media(owner), Action::StopAll) if owner.kind() == MediaKind::Matrix => {
            super::playback::unload(engine, media)?
        }
        (AppNode::MatrixColumn(_), Action::Play | Action::Cut) => {
            super::playback::launch(engine, media, super::protocol::ColumnId(invocation.owner), mode)?
        }
        (AppNode::MatrixCell(cell), Action::Play | Action::Cut) => {
            let layer = cell
                .node_data()
                .parent
                .and_then(|id| engine.nodes.get(id))
                .filter(|node| matches!(node, AppNode::MatrixElement(element) if element.is_layer()))
                .ok_or("Cell control has no Layer")?;
            let layer = super::protocol::LayerId(layer.node_data().meta.uuid);
            let column = cell.column();
            super::playback::launch_cell(engine, media, layer, column, mode)?
        }
        _ => return Err("control action does not belong to this node".into()),
    };
    serde_json::to_value(result).map_err(|error| error.to_string())
}
