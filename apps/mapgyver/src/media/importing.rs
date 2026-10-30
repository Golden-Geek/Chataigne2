//! Atomic Picture import: detached trees and source assignment share one history transaction.

use golden_core::edit::{Edit, NodeTree};
use golden_core::node::Node;

use super::authoring::{channel, configure_channel, label_with_reserved, locator_label, manager, media_tree};
use super::protocol::*;
use crate::app::AppEngine;

pub(super) fn picture(
    engine: &mut AppEngine,
    locator: AssetLocator,
    target: Option<ChannelReference>,
    label: Option<String>,
) -> Result<MediaEditResult, String> {
    let parent = manager(engine)?;
    let mut definition = if let Some(reference) = target {
        let snapshot = channel(engine, reference)?;
        if snapshot.definition.media_kind() != MediaKind::Picture {
            return Err("Picture import requires a Picture channel".into());
        }
        snapshot.definition
    } else {
        ChannelDefinition::for_media(MediaKind::Picture)
    };
    let media_label = if target.is_none() {
        Some(label_with_reserved(engine, parent, label, "Picture", &[])?)
    } else {
        if label.is_some() {
            return Err("media label is only valid when creating Picture media".into());
        }
        None
    };
    let reserved = media_label.as_deref().into_iter().collect::<Vec<_>>();
    let asset_label = label_with_reserved(engine, parent, None, locator_label(&locator), &reserved)?;
    let node = super::AssetNode::new(asset_label, Some(locator));
    let asset = node.id();
    definition.source = ChannelSource::Picture { asset: Some(asset) };
    let previous = engine.nodes.get(parent).and_then(|node| node.node_data().last_child);
    let (tree, reference) = if let Some(reference) = target {
        // Validate and queue the source change before queuing any catalog insertion.
        configure_channel(engine, reference, definition)?;
        (None, reference)
    } else {
        let (tree, reference) = media_tree(media_label.expect("new media label"), definition);
        (Some(tree), reference)
    };
    engine.edits.push(Edit::AddNodeTree {
        parent,
        prev_sibling: previous,
        tree: NodeTree::new(node),
    });
    if let Some(tree) = tree {
        engine.edits.push(Edit::AddNodeTree {
            parent,
            prev_sibling: previous,
            tree,
        });
    }
    Ok(MediaEditResult::PictureImported { reference, asset })
}
