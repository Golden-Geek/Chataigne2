//! New-document defaults only; decode, copying and runtime never regenerate authored structure.
use super::{
    ColumnNode,
    authoring::{export, item_id},
    node::ElementNode,
    protocol::*,
};
use crate::media::protocol::MediaId;
use golden_core::edit::NodeTree;
use golden_video::Extent2D;

pub(crate) fn initial_tree(media: MediaId, mut root: NodeTree, canvas: Extent2D) -> (NodeTree, Vec<NodeTree>) {
    let mut exports = Vec::with_capacity(3);
    for index in 1..=3 {
        let label = format!("Layer {index}");
        let node = ElementNode::from_definition(
            label.clone(),
            ElementDefinition::Layer {
                settings: CompositeSettings::default(),
            },
        )
        .with_layer_settings(LayerSettings {
            size: canvas,
            ..Default::default()
        });
        let (channel, _) = export(media, item_id(&node), format!("{label} output"));
        exports.push(channel.as_user_item());
        root.push_child(NodeTree::new(node).as_user_item());
    }
    for index in 1..=3 {
        root.push_child(
            NodeTree::new(ColumnNode::from_definition(
                format!("Column {index}"),
                ColumnDefinition::default(),
            ))
            .as_user_item(),
        );
    }
    (root, exports)
}
