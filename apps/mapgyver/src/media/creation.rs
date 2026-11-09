//! One backend catalog and complete subtree factory for every document creation surface.
use super::{
    MediaFolderNode,
    authoring::media_tree,
    protocol::{ChannelDefinition, ChannelReference, ChannelSource, MediaKind},
};
use crate::app::AppEngine;
use golden_core::{
    edit::NodeTree,
    node::{Node, UserCreatableItem, UserItemCreationContext},
    parameter::ParamValue,
};

const SOURCES: [(&str, MediaKind); 6] = [
    ("mapgyver.media.solid_color", MediaKind::SolidColor),
    ("mapgyver.media.picture", MediaKind::Picture),
    ("mapgyver.media.video", MediaKind::Video),
    ("mapgyver.media.shared_texture", MediaKind::SharedTexture),
    ("mapgyver.media.composition", MediaKind::Composition),
    ("mapgyver.media.matrix", MediaKind::Matrix),
];

pub(crate) fn media_choices() -> Vec<super::protocol::MediaCreationChoice> {
    std::iter::once(MediaKind::Reference)
        .chain(SOURCES.into_iter().map(|(_, kind)| kind))
        .map(|media_kind| super::protocol::MediaCreationChoice {
            media_kind,
            label: media_kind.default_label().into(),
        })
        .collect()
}

pub(crate) fn owned_tree(engine: &AppEngine, kind: MediaKind, label: String) -> NodeTree {
    if kind == MediaKind::Reference {
        NodeTree::new(super::ReferenceNode::from_source(label, None))
    } else {
        media(engine, kind, label).0
    }
}

pub(crate) fn media(engine: &AppEngine, kind: MediaKind, label: String) -> (NodeTree, ChannelReference) {
    let lookup = |id| engine.nodes.get(id).map(|node| node as &dyn Node);
    let context = UserItemCreationContext::new(engine.root, &lookup);
    live_media(&context, kind, label)
}

pub(super) fn tree_with_context(node_type: &str, context: &UserItemCreationContext<'_>) -> Option<NodeTree> {
    let Some((_, kind)) = SOURCES.into_iter().find(|(alias, _)| *alias == node_type) else {
        return tree(node_type);
    };
    Some(live_media(context, kind, kind.default_label().into()).0)
}

fn live_media(context: &UserItemCreationContext<'_>, kind: MediaKind, label: String) -> (NodeTree, ChannelReference) {
    if kind != MediaKind::SharedTexture {
        return media_tree(label, ChannelDefinition::for_media(kind));
    }
    let mut next = context
        .node(context.root())
        .and_then(|node| node.node_data().first_child);
    let mut options = super::sender_catalog::choices(&[]);
    while let Some(id) = next {
        let Some(node) = context.node(id) else { break };
        if let Some(manager) = node.as_any().downcast_ref::<super::MediaManagerNode>() {
            options = manager.sender_catalog.options();
            break;
        }
        next = node.node_data().next_sibling;
    }
    let sender = options.as_slice().iter().find_map(|option| match &option.value {
        ParamValue::Enum(name) if !name.is_empty() => Some(name.clone()),
        _ => None,
    });
    let definition = ChannelDefinition {
        source: ChannelSource::Spout { sender },
        ..ChannelDefinition::for_media(kind)
    };
    let (mut tree, reference) = media_tree(label, definition);
    tree.node
        .as_any_mut()
        .downcast_mut::<super::MediaNode>()
        .unwrap()
        .initialize_auto_sender_selection();
    for child in &mut tree.children {
        if let Some(channel) = child.node.as_any_mut().downcast_mut::<super::ChannelNode>() {
            channel.initialize_sender_options(options.clone());
        }
    }
    (tree, reference)
}

pub(super) fn items() -> Vec<UserCreatableItem> {
    let mut items = vec![UserCreatableItem::new(
        "mapgyver.media.folder",
        "mapgyver.media.folder",
        "Folder",
    )];
    items.push(UserCreatableItem::new(
        super::reference::NODE_TYPE,
        super::reference::NODE_TYPE,
        "Reference",
    ));
    items.extend(
        SOURCES
            .into_iter()
            .map(|(node_type, kind)| UserCreatableItem::new(node_type, "mapgyver.media.v10", kind.default_label())),
    );
    items
}

pub(super) fn tree(node_type: &str) -> Option<NodeTree> {
    if node_type == super::reference::NODE_TYPE {
        return Some(NodeTree::new(super::ReferenceNode::new()));
    }
    if node_type == "mapgyver.media.folder" {
        return Some(NodeTree::new(MediaFolderNode::new("Folder".into())));
    }
    let (_, kind) = SOURCES.into_iter().find(|(alias, _)| *alias == node_type)?;
    Some(media_tree(kind.default_label().into(), ChannelDefinition::for_media(kind)).0)
}
