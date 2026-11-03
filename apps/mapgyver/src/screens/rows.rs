//! Constant-size row projections, independent of Surface and channel collection size.
use super::protocol::{ScreenId, ScreenRowSnapshot, ScreenTarget};
use crate::app::{AppEngine, AppNode};
use golden_core::{model::NodeUuid, node::Node};

pub(super) fn inspect(engine: &AppEngine, node: NodeUuid) -> Result<ScreenRowSnapshot, String> {
    let id = engine.node_id_by_uuid(node).ok_or("row object is missing")?;
    let owner = engine.nodes.get(id).ok_or("row object is missing")?;
    let (detail, extent, enabled) = match owner {
        AppNode::Screen(screen) => {
            let definition = screen.definition();
            let detail = match &definition.target {
                Some(ScreenTarget::Fullscreen { display }) => format!("Fullscreen · {display}"),
                Some(ScreenTarget::Rectangle { bounds }) => format!("Rectangle · {}, {}", bounds.left, bounds.top),
                Some(ScreenTarget::Spout { sender, .. }) => format!("Spout · {sender}"),
                None => "Unconfigured".into(),
            };
            (
                detail,
                super::target::output_extent(engine, ScreenId(node)).ok(),
                definition.enabled,
            )
        }
        AppNode::Surface(surface) => {
            let definition = surface.definition();
            let detail = match definition.source {
                None => "No source".into(),
                Some(source) => {
                    let label = |uuid| {
                        engine
                            .node_id_by_uuid(uuid)
                            .and_then(|id| engine.nodes.get(id))
                            .map(|node| node.node_data().meta.label.as_str())
                    };
                    match (label(source.media.0), label(source.channel.0)) {
                        (Some(media), Some(channel)) => format!("{media} / {channel}"),
                        _ => "Missing source".into(),
                    }
                }
            };
            (detail, None, definition.enabled && surface.node_data().meta.enabled)
        }
        _ => return Err("row object is not a Screen or Surface".into()),
    };
    Ok(ScreenRowSnapshot {
        node,
        detail,
        extent,
        enabled,
    })
}
