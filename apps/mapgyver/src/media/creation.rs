//! One backend catalog and complete subtree factory for every document creation surface.
use super::{
    MediaFolderNode,
    authoring::media_tree,
    protocol::{ChannelDefinition, MediaKind},
};
use golden_core::{edit::NodeTree, node::UserCreatableItem};

const SOURCES: [(&str, &str, MediaKind); 5] = [
    ("mapgyver.media.solid_color", "Solid Color", MediaKind::SolidColor),
    ("mapgyver.media.picture", "Picture", MediaKind::Picture),
    (
        "mapgyver.media.shared_texture",
        "Shared Texture",
        MediaKind::SharedTexture,
    ),
    ("mapgyver.media.composition", "Composition", MediaKind::Composition),
    ("mapgyver.media.matrix", "Matrix", MediaKind::Matrix),
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
            .map(|(node_type, label, _)| UserCreatableItem::new(node_type, "mapgyver.media.v3", label)),
    );
    items
}

pub(super) fn tree(node_type: &str) -> Option<NodeTree> {
    if node_type == "mapgyver.media.folder" {
        return Some(NodeTree::new(MediaFolderNode::new("Folder".into())));
    }
    let (_, label, kind) = SOURCES.into_iter().find(|(alias, _, _)| *alias == node_type)?;
    Some(media_tree(label.into(), ChannelDefinition::for_media(kind)).0)
}
