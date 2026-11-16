//! Live Screen defaults read document order and the last published native display catalog.
//! Intrinsic factories never call this module, preserving stable persistence baselines.
use super::{ScreenManagerNode, ScreenNode, SurfaceNode, protocol::*};
use crate::media::protocol::{ChannelId, ChannelReference, MediaId};
use golden_core::{
    edit::NodeTree,
    node::{Node, NodeId, UserItemCreationContext},
};
use std::collections::HashSet;

/// Fresh candidates receive host metadata before seeding, never after loading authored data.
pub(crate) fn initial_tree(displays: &[DisplayInfo]) -> NodeTree {
    let display = available_display(displays, &HashSet::new());
    with_surface("Screen".into(), fullscreen_target(display), display, None)
}

fn fullscreen_target(display: Option<&DisplayInfo>) -> ScreenTarget {
    ScreenTarget::Fullscreen {
        display: display.map(|display| display.id.clone()).unwrap_or_default(),
    }
}

pub(crate) fn tree(context: &UserItemCreationContext<'_>, label: String) -> Result<NodeTree, String> {
    let manager = manager(context, "mapgyver.screen.manager").ok_or("Screens is missing")?;
    let owner = context
        .node(manager)
        .and_then(|node| node.as_any().downcast_ref::<ScreenManagerNode>())
        .ok_or("Screens is missing")?;
    let displays = owner.displays();
    let mut requested = Vec::new();
    let mut next = context.node(manager).and_then(|node| node.node_data().first_child);
    while let Some(id) = next {
        let node = context.node(id).ok_or("Screens child is missing")?;
        next = node.node_data().next_sibling;
        let Some(screen) = node.as_any().downcast_ref::<ScreenNode>() else {
            continue;
        };
        if screen.is_fullscreen() && !screen.authored_display_identity().is_empty() {
            requested.push((ScreenId(screen.node_data().meta.uuid), screen.display_hint()));
        }
    }
    context.visit_prepared_children(manager, |node| {
        if let Some(screen) = node.as_any().downcast_ref::<ScreenNode>()
            && screen.is_fullscreen()
            && !screen.authored_display_identity().is_empty()
        {
            requested.push((ScreenId(screen.node_data().meta.uuid), screen.display_hint()));
        }
    });
    // Disabled authored assignments still reserve their physical display for a later enable.
    let reserved = if requested.len() > super::rendering::MAX_SCREEN_CATALOG {
        (0..displays.len()).collect::<HashSet<_>>()
    } else {
        super::display_matching::match_displays(&requested, &displays)
            .into_values()
            .map(|matched| matched.display_index)
            .collect::<HashSet<_>>()
    };
    let display = available_display(&displays, &reserved);
    Ok(with_surface(
        label,
        fullscreen_target(display),
        display,
        first_source(context)?,
    ))
}

fn available_display<'a>(displays: &'a [DisplayInfo], reserved: &HashSet<usize>) -> Option<&'a DisplayInfo> {
    displays
        .iter()
        .enumerate()
        .filter(|(index, display)| {
            display.internal == Some(false)
                && !reserved.contains(index)
                && display.bounds.extent.width() <= 8192
                && display.bounds.extent.height() <= 8192
        })
        .min_by(|(_, left), (_, right)| left.id.cmp(&right.id))
        .map(|(_, display)| display)
}

fn with_surface(
    label: String,
    target: ScreenTarget,
    display: Option<&DisplayInfo>,
    source: Option<ChannelReference>,
) -> NodeTree {
    let mut screen = ScreenNode::from_definition(
        label,
        ScreenDefinition {
            frame_rate: Default::default(),
            target: Some(target),
            enabled: true,
            anti_aliasing: Default::default(),
        },
    );
    screen.use_automatic_sender();
    if let Some(display) = display {
        screen.remember_display(display);
    }
    let definition = SurfaceDefinition {
        source,
        ..Default::default()
    };
    NodeTree::new(screen)
        .with_child(NodeTree::new(SurfaceNode::from_definition("Surface 1".into(), definition)).as_user_item())
}

pub(crate) fn unique_sender(used: &HashSet<String>) -> String {
    (1u64..)
        .map(|index| {
            if index == 1 {
                "MapGyver Screen".to_owned()
            } else {
                format!("MapGyver Screen {index}")
            }
        })
        .find(|sender| !used.contains(sender))
        .expect("finite authored sender set")
}

fn manager(context: &UserItemCreationContext<'_>, kind: &str) -> Option<NodeId> {
    let mut next = context.node(context.root())?.node_data().first_child;
    while let Some(id) = next {
        let node = context.node(id)?;
        if node.get_type() == kind {
            return Some(id);
        }
        next = node.node_data().next_sibling;
    }
    None
}

fn first_source(context: &UserItemCreationContext<'_>) -> Result<Option<ChannelReference>, String> {
    let manager = manager(context, "mapgyver.media.manager").ok_or("Medias is missing")?;
    let mut pending = context
        .node(manager)
        .and_then(|node| node.node_data().first_child)
        .into_iter()
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            return Err("media hierarchy contains a cycle".into());
        }
        let node = context.node(id).ok_or("media hierarchy child is missing")?;
        if let Some(next) = node.node_data().next_sibling {
            pending.push(next);
        }
        if !node.node_data().effective_enabled {
            continue;
        }
        match node.get_type() {
            "mapgyver.media.folder" => {
                if let Some(child) = node.node_data().first_child {
                    pending.push(child);
                }
            }
            "mapgyver.media.v11" => {
                let mut next = node.node_data().first_child;
                while let Some(id) = next {
                    let channel = context.node(id).ok_or("media child is missing")?;
                    if channel.get_type() == "mapgyver.media.channel.v2" {
                        return Ok(Some(ChannelReference {
                            media: MediaId(node.node_data().meta.uuid),
                            channel: ChannelId(channel.node_data().meta.uuid),
                        }));
                    }
                    next = channel.node_data().next_sibling;
                }
                return Err("enabled media has no channel".into());
            }
            _ => {}
        }
    }
    Ok(None)
}
