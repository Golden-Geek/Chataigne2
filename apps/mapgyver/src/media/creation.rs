//! One backend catalog and complete subtree factory for every document creation surface.
use super::{
    MediaFolderNode,
    authoring::media_tree,
    protocol::{ChannelDefinition, MediaKind},
};
use golden_core::{edit::NodeTree, node::UserCreatableItem};

const SOURCES: [(&str, MediaKind); 5] = [
    ("mapgyver.media.solid_color", MediaKind::SolidColor),
    ("mapgyver.media.picture", MediaKind::Picture),
    ("mapgyver.media.shared_texture", MediaKind::SharedTexture),
    ("mapgyver.media.composition", MediaKind::Composition),
    ("mapgyver.media.matrix", MediaKind::Matrix),
];

pub(super) fn items() -> Vec<UserCreatableItem> {
    let mut items = vec![UserCreatableItem::new(
        "mapgyver.media.folder",
        "mapgyver.media.folder",
        "Folder",
    )];
    items.extend(
        SOURCES
            .into_iter()
            .map(|(node_type, kind)| UserCreatableItem::new(node_type, "mapgyver.media.v5", kind.default_label())),
    );
    items
}

pub(super) fn tree(node_type: &str) -> Option<NodeTree> {
    if node_type == "mapgyver.media.folder" {
        return Some(NodeTree::new(MediaFolderNode::new("Folder".into())));
    }
    let (_, kind) = SOURCES.into_iter().find(|(alias, _)| *alias == node_type)?;
    Some(media_tree(kind.default_label().into(), ChannelDefinition::for_media(kind)).0)
}
