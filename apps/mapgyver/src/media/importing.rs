//! Picture import creates one owned media tree or edits an existing channel, never an asset.
use super::{
    authoring::{channel, configure_channel, label_with_reserved, manager, media_tree},
    protocol::*,
};
use crate::app::AppEngine;
use golden_core::edit::Edit;
use golden_core::node::Node;

pub(super) fn picture(
    engine: &mut AppEngine,
    locator: AssetLocator,
    target: Option<ChannelReference>,
    label: Option<String>,
) -> Result<MediaEditResult, String> {
    let source = ChannelSource::Picture { file: Some(locator) };
    let reference = if let Some(reference) = target {
        if label.is_some() {
            return Err("media label is only valid when creating Picture media".into());
        }
        if channel(engine, reference)?.definition.media_kind() != MediaKind::Picture {
            return Err("Picture import requires a Picture channel".into());
        }
        configure_channel(engine, reference, source)?;
        reference
    } else {
        let parent = manager(engine)?;
        let label = label_with_reserved(engine, parent, label, MediaKind::Picture.default_label(), &[])?;
        let definition = ChannelDefinition {
            source,
            ..ChannelDefinition::for_media(MediaKind::Picture)
        };
        let (tree, reference) = media_tree(label, definition);
        engine.edits.push(Edit::AddUserItemTree {
            parent,
            prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
            tree,
        });
        reference
    };
    Ok(MediaEditResult::PictureImported { reference })
}
