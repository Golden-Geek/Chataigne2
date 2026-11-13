//! Real Parameter pulses enter the same bounded app-operation service as script commands.
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::{MediaEditResult, MediaId, MediaKind},
};
use golden_core::{
    edit::Edit,
    events::CustomEvent,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessCtx,
    script::{APP_OPERATION_RESULT_TOPIC, ScriptAppOperationRequest},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) const NAMESPACE: &str = "mapgyver.matrix.control";

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) enum Action {
    Play,
    Cut,
    StopAll,
    Stop,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    owner: NodeUuid,
    action: Action,
    token: uuid::Uuid,
}

#[derive(Default)]
pub(crate) struct Pulses {
    ready: bool,
    pending: HashMap<uuid::Uuid, Action>,
    owner: Arc<()>,
}
impl Pulses {
    pub(crate) fn ready(&mut self) {
        self.ready = true;
    }
    pub(crate) fn revoke(&mut self) {
        self.owner = Arc::new(());
        self.pending.clear();
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
            let token = uuid::Uuid::new_v4();
            let request_id = format!("matrix:{token}");
            let reservation = ScriptAppOperationRequest::reserve(&request_id, NAMESPACE, "invoke")?;
            let payload = serde_json::to_value(Invocation {
                owner: node.meta.uuid,
                action,
                token,
            })
            .map_err(|error| error.to_string())?;
            let request = reservation.finish(payload).map_err(|error| error.to_string())?;
            self.pending.insert(token, action);
            ctx.enqueue_app_operation(node.id, Arc::downgrade(&self.owner), request);
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            ctx.set_node_warning_with(node.id, Some("matrix-control"), error, None);
        }
    }
    pub(crate) fn claim(&mut self, action: Action, token: uuid::Uuid) -> bool {
        if self.pending.get(&token) != Some(&action) {
            return false;
        }
        self.pending.remove(&token);
        true
    }
    pub(crate) fn result(&mut self, ctx: &mut ProcessCtx, node: NodeId, event: &CustomEvent) {
        if event.topic != APP_OPERATION_RESULT_TOPIC || event.origin != Some(node) {
            return;
        }
        let reply = &event.payload["args"][0];
        if let Some(token) = reply["request_id"]
            .as_str()
            .and_then(|id| id.strip_prefix("matrix:"))
            .and_then(|token| token.parse::<uuid::Uuid>().ok())
        {
            self.pending.remove(&token);
        }
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

fn pulse(engine: &mut AppEngine, owner: NodeId, action: Action) -> Result<MediaEditResult, String> {
    let node = engine.nodes.get(owner).ok_or("Matrix control owner is missing")?;
    let parameter = match (node, action) {
        (AppNode::Media(node), Action::StopAll) => node.stop_all_parameter(),
        (AppNode::MatrixColumn(node), action) => node.control_parameter(action),
        (AppNode::MatrixCell(node), action) => node.control_parameter(action),
        _ => None,
    }
    .ok_or("Matrix control does not belong to this owner")?;
    let parameter_uuid = engine
        .nodes
        .get(parameter)
        .ok_or("Matrix control Parameter is missing")?
        .node_data()
        .meta
        .uuid;
    let owner = node.node_data().meta.uuid;
    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Append,
    });
    Ok(MediaEditResult::MatrixControlInvoked {
        owner,
        parameter: parameter_uuid,
    })
}

pub(super) fn launch_column(
    engine: &mut AppEngine,
    media: MediaId,
    column: super::protocol::ColumnId,
    mode: super::protocol::LaunchMode,
) -> Result<MediaEditResult, String> {
    let root = super::playback::root(engine, media)?;
    let id = engine.node_id_by_uuid(column.0).ok_or("Matrix Column is missing")?;
    if !matches!(engine.nodes.get(id), Some(AppNode::MatrixColumn(_)))
        || super::column_axis::owner(id, |id| engine.nodes.get(id).map(|node| node as &dyn Node)) != Some(root)
    {
        return Err("Column belongs to another Matrix or is missing".into());
    }
    pulse(engine, id, launch_action(mode))
}

pub(super) fn launch_cell(
    engine: &mut AppEngine,
    media: MediaId,
    layer: super::protocol::LayerId,
    column: super::protocol::ColumnId,
    mode: super::protocol::LaunchMode,
) -> Result<MediaEditResult, String> {
    let root = super::playback::root(engine, media)?;
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get(root) else {
        unreachable!()
    };
    let control = node.matrix_playback().ok_or("Matrix playback is missing")?.clone();
    let index = super::configuration::index(engine, media, &control)?;
    if !index.layers.contains(&layer) || !index.transitions.contains_key(&column) {
        return Err("Cell address belongs to another Matrix or is missing".into());
    }
    if let Some(cell) = index.cell_nodes.get(&(layer, column)) {
        return pulse(engine, *cell, launch_action(mode));
    }
    let document = super::document::Document::load(engine, media)?;
    let cell = super::cells::ensure(engine, &document, layer, column)?;
    pulse(engine, cell, launch_action(mode))
}

pub(super) fn clear(engine: &mut AppEngine, media: MediaId) -> Result<MediaEditResult, String> {
    super::playback::root(engine, media)?;
    let id = engine.node_id_by_uuid(media.0).ok_or("Matrix is missing")?;
    pulse(engine, id, Action::StopAll)
}

fn launch_action(mode: super::protocol::LaunchMode) -> Action {
    match mode {
        super::protocol::LaunchMode::Configured => Action::Play,
        super::protocol::LaunchMode::Cut => Action::Cut,
    }
}

pub(crate) fn apply(
    engine: &mut AppEngine,
    operation: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    if operation == "refresh-video" {
        return super::video_refresh::apply(engine, payload);
    }
    if operation != "invoke" {
        return Err("unknown Matrix control operation".into());
    }
    let invocation: Invocation = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let id = engine
        .node_id_by_uuid(invocation.owner)
        .ok_or("Matrix control owner is missing")?;
    let claimed = match engine.nodes.get_mut(id) {
        Some(AppNode::Media(node)) => node.claim_control(invocation.action, invocation.token),
        Some(AppNode::MatrixColumn(node)) => node.claim_control(invocation.action, invocation.token),
        Some(AppNode::MatrixCell(node)) => node.claim_control(invocation.action, invocation.token),
        Some(AppNode::MatrixColumnGroup(node)) => node.claim_control(invocation.action, invocation.token),
        Some(AppNode::MatrixElement(node)) => node.claim_control(invocation.action, invocation.token),
        _ => false,
    };
    if !claimed {
        return Err("Matrix control delivery requires an unconsumed live Parameter invocation".into());
    }
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
        Action::StopAll | Action::Stop => super::protocol::LaunchMode::Cut,
    };
    let result: MediaEditResult = match (node, invocation.action) {
        (_, Action::Stop) => super::stops::stop(engine, media, invocation.owner)?,
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
