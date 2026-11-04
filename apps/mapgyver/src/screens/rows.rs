//! Constant-size row projections, independent of Surface and channel collection size.
use super::protocol::{ScreenId, ScreenRowSnapshot, ScreenTarget};
use crate::app::{AppEngine, AppNode};
use golden_core::{model::NodeUuid, node::Node};

fn display_name(name: &str, identity: &str, index: u32) -> String {
    let name = name.trim();
    if name.is_empty() || name == identity || name.contains('\\') {
        if index > 0 {
            format!("Display {index}")
        } else {
            "Display".into()
        }
    } else {
        name.into()
    }
}

pub(super) fn inspect(engine: &AppEngine, node: NodeUuid) -> Result<Option<ScreenRowSnapshot>, String> {
    // A mounted outliner row can outlive deletion/project replacement until its next UI
    // snapshot. Missing data is a normal bounded query result, not a rejected edit.
    let Some(owner) = engine.node_id_by_uuid(node).and_then(|id| engine.nodes.get(id)) else {
        return Ok(None);
    };
    let (detail, extent, enabled) = match owner {
        AppNode::Screen(screen) => {
            let definition = screen.definition();
            let detail = match &definition.target {
                Some(ScreenTarget::Fullscreen { display }) => {
                    let hint = screen.display_hint();
                    let manager = screen.node_data().parent.and_then(|id| engine.nodes.get(id));
                    let resolved = match manager {
                        Some(AppNode::ScreenManager(manager)) => {
                            super::target::fullscreen_display(screen, manager, &manager.displays()).cloned()
                        }
                        _ => None,
                    };
                    match resolved {
                        Some(found) => format!(
                            "Fullscreen · {}{}",
                            display_name(&found.name, &found.id, found.index),
                            if found.id == *display { "" } else { " (recovered)" }
                        ),
                        None => format!(
                            "Fullscreen · {} (missing)",
                            display_name(&hint.name, display, hint.index)
                        ),
                    }
                }
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
    Ok(Some(ScreenRowSnapshot {
        node,
        detail,
        extent,
        enabled,
    }))
}
