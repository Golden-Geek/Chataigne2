//! Live Screen defaults read document order and the last published native display catalog.
//! Intrinsic factories never call this module, preserving stable persistence baselines.
use super::{ScreenManagerNode, ScreenNode, SurfaceNode, protocol::*};
use crate::media::protocol::{ChannelId, ChannelReference, MediaId};
use golden_core::{
    edit::NodeTree,
    node::{NodeId, UserItemCreationContext},
};
use std::collections::HashSet;

/// Fresh documents precede the native catalog. Use the same no-display fallback
/// as live creation, without later rewriting the authored destination.
pub(crate) fn initial_tree() -> NodeTree {
    with_surface("Screen".into(), shared_target(&HashSet::new()), None, None)
}

fn shared_target(senders: &HashSet<String>) -> ScreenTarget {
    ScreenTarget::Spout {
        sender: unique_sender(senders),
        extent: crate::media::resolution::default_canvas(),
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
    let mut senders = HashSet::new();
    let mut next = context.node(manager).and_then(|node| node.node_data().first_child);
    while let Some(id) = next {
        let node = context.node(id).ok_or("Screens child is missing")?;
        next = node.node_data().next_sibling;
        let Some(screen) = node.as_any().downcast_ref::<ScreenNode>() else {
            continue;
        };
        match screen.definition().target {
            Some(ScreenTarget::Fullscreen { .. }) => {
                requested.push((ScreenId(node.node_data().meta.uuid), screen.display_hint()))
            }
            Some(ScreenTarget::Spout { sender, .. }) => {
                senders.insert(sender);
            }
            _ => {}
        }
    }
    // Disabled authored assignments still reserve their physical display for a later enable.
    let reserved = if requested.len() > super::rendering::MAX_SCREEN_CATALOG {
        (0..displays.len()).collect::<HashSet<_>>()
    } else {
        super::display_matching::match_displays(&requested, &displays)
            .into_values()
            .map(|matched| matched.display_index)
            .collect::<HashSet<_>>()
    };
    let secondary = displays
        .iter()
        .enumerate()
        .filter(|(index, display)| {
            !display.primary
                && !reserved.contains(index)
                && display.bounds.extent.width() <= 8192
                && display.bounds.extent.height() <= 8192
        })
        .min_by_key(|(order, display)| (if display.index == 0 { u32::MAX } else { display.index }, *order))
        .map(|(_, display)| display);
    let target = if let Some(display) = secondary {
        ScreenTarget::Fullscreen {
            display: display.id.clone(),
        }
    } else {
        shared_target(&senders)
    };
    Ok(with_surface(label, target, secondary, first_source(context)?))
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
            "mapgyver.media.v8" => {
                let mut next = node.node_data().first_child;
                while let Some(id) = next {
                    let channel = context.node(id).ok_or("media child is missing")?;
                    if channel.get_type() == "mapgyver.media.channel" {
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
