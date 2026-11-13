//! Private actor delivery requires the live Matrix's unconsumed request token.
use crate::app::{AppEngine, AppNode};
use golden_core::node::NodeUuid;

pub(crate) fn apply(engine: &mut AppEngine, payload: serde_json::Value) -> Result<serde_json::Value, String> {
    let (root, token): (NodeUuid, uuid::Uuid) = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let id = engine.node_id_by_uuid(root).ok_or("Matrix Final is missing")?;
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
        return Err("not a Matrix Final channel".into());
    };
    let control = channel.matrix_playback().ok_or("not a Matrix Final channel")?.clone();
    if !control.claim_video_refresh(token) {
        return Err("Video reconciliation requires an unconsumed live request".into());
    }
    control.refresh_videos(engine, id)?;
    Ok(serde_json::Value::Null)
}
