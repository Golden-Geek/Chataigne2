//! Shared cached output identity/extent resolution for rendering and authored pixel intents.
use super::{ScreenManagerNode, ScreenNode, protocol::*};
use crate::app::{AppEngine, AppNode};
use golden_core::node::Node;
use golden_video::Extent2D;

pub(super) fn fullscreen_display<'a>(
    screen: &ScreenNode,
    manager: &ScreenManagerNode,
    displays: &'a [DisplayInfo],
) -> Option<&'a DisplayInfo> {
    let Some(ScreenTarget::Fullscreen { display }) = screen.definition().target else {
        return None;
    };
    let hint = screen.display_hint();
    manager
        .resolved_display(ScreenId(screen.node_data().meta.uuid), &hint)
        .and_then(|resolved| displays.iter().find(|found| *found == resolved))
        .or_else(|| {
            if manager.has_runtime_allocation() {
                return None;
            }
            displays.iter().find(|found| {
                found.id == display && (manager.displays().is_empty() || DisplayIdentityHint::from(*found) == hint)
            })
        })
}

pub(super) fn output_extent(engine: &AppEngine, screen: ScreenId) -> Result<Extent2D, String> {
    let id = super::authoring::screen_id(engine, screen)?;
    let Some(AppNode::Screen(node)) = engine.nodes.get(id) else {
        unreachable!()
    };
    match node.definition().target {
        Some(ScreenTarget::Spout { extent, .. }) => Ok(extent),
        Some(ScreenTarget::Rectangle { bounds }) => Ok(bounds.extent),
        Some(ScreenTarget::Fullscreen { .. }) => {
            let manager = super::authoring::manager(engine)?;
            let Some(AppNode::ScreenManager(manager)) = engine.nodes.get(manager) else {
                unreachable!()
            };
            fullscreen_display(node, manager, &manager.displays())
                .map(|found| found.bounds.extent)
                .ok_or_else(|| "output pixel extent is unavailable for the intended fullscreen display".into())
        }
        None => Err("output dimensions require a configured Screen target".into()),
    }
}
