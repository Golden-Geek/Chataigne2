//! One backend catalog and complete subtree factory for every document creation surface.
use super::{
    ChannelNode, MediaFolderNode,
    authoring::media_tree,
    protocol::{ChannelDefinition, ChannelReference, ChannelSource, MediaKind},
};
use crate::app::AppEngine;
use golden_core::{
    edit::NodeTree,
    node::{Node, UserCreatableItem, UserItemCreationContext},
};

const SOURCES: [(&str, MediaKind); 8] = [
    ("mapgyver.media.solid_color", MediaKind::SolidColor),
    ("mapgyver.media.picture", MediaKind::Picture),
    ("mapgyver.media.video", MediaKind::Video),
    ("mapgyver.media.shader", MediaKind::Shader),
    ("mapgyver.media.shared_texture", MediaKind::SharedTexture),
    ("mapgyver.media.ndi", MediaKind::Ndi),
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

pub(super) fn channel(engine: &AppEngine, kind: MediaKind, label: String) -> ChannelNode {
    let lookup = |id| engine.nodes.get(id).map(|node| node as &dyn Node);
    live_channel(&UserItemCreationContext::new(engine.root, &lookup), kind, label)
}

pub(super) fn live_channel(context: &UserItemCreationContext<'_>, kind: MediaKind, label: String) -> ChannelNode {
    if kind != MediaKind::Ndi {
        return ChannelNode::from_definition(label, ChannelDefinition::for_media(kind));
    }
    let catalog = sender_catalog(context, kind);
    let external_only = catalog.default_sender.is_some();
    let mut channel = ChannelNode::from_definition(
        label,
        ChannelDefinition {
            source: ChannelSource::Ndi {
                sender: catalog.default_sender,
            },
            ..ChannelDefinition::for_media(kind)
        },
    );
    channel.initialize_sender_defaults(catalog.options, external_only);
    channel
}

pub(super) fn tree_with_context(node_type: &str, context: &UserItemCreationContext<'_>) -> Option<NodeTree> {
    let Some((_, kind)) = SOURCES.into_iter().find(|(alias, _)| *alias == node_type) else {
        return tree(node_type);
    };
    Some(live_media(context, kind, kind.default_label().into()).0)
}

fn sender_catalog(context: &UserItemCreationContext<'_>, kind: MediaKind) -> super::sender_catalog::Snapshot {
    let mut next = context
        .node(context.root())
        .and_then(|node| node.node_data().first_child);
    while let Some(id) = next {
        let Some(node) = context.node(id) else { break };
        if let Some(manager) = node.as_any().downcast_ref::<super::MediaManagerNode>() {
            return manager.sender_catalog(kind).snapshot();
        }
        next = node.node_data().next_sibling;
    }
    super::sender_catalog::Snapshot::default()
}

fn live_media(context: &UserItemCreationContext<'_>, kind: MediaKind, label: String) -> (NodeTree, ChannelReference) {
    if !kind.has_sender_selection() {
        return media_tree(label, ChannelDefinition::for_media(kind));
    }
    let catalog = sender_catalog(context, kind);
    let external_only = catalog.default_sender.is_some();
    let definition = ChannelDefinition {
        source: if kind == MediaKind::Ndi {
            ChannelSource::Ndi {
                sender: catalog.default_sender,
            }
        } else {
            ChannelSource::Spout {
                sender: catalog.default_sender,
            }
        },
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
            channel.initialize_sender_defaults(catalog.options.clone(), external_only);
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
            .map(|(node_type, kind)| UserCreatableItem::new(node_type, "mapgyver.media.v11", kind.default_label())),
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
