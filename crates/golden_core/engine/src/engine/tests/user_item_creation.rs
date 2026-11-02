use super::*;
use crate::{
    edit::NodeTree,
    node::{Folder, Node, UserContainerRules, UserCreatableItem, UserItemCreationContext},
    ui_sync::UiEditIntent,
};

#[crate::node("live_creation_factory")]
struct LiveCreationFactory {}
#[crate::node("live_creation_factory", from_struct)]
impl Node for LiveCreationFactory {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["live_creation_item"]))
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new(
            "live_creation_item",
            "live_creation_item",
            "Item",
        )]
    }
    fn create_user_item_tree(&self, kind: &str) -> Option<NodeTree> {
        (kind == "live_creation_item")
            .then(|| NodeTree::new(LiveCreationItem::new()).with_child(NodeTree::new(Folder::new("Intrinsic"))))
    }
    fn create_user_item_tree_with_context(
        &self,
        kind: &str,
        context: &UserItemCreationContext<'_>,
    ) -> Option<NodeTree> {
        let mut tree = self.create_user_item_tree(kind)?;
        let root = context.node(context.root())?;
        tree.children[0].node.node_data_mut().meta.label = root.node_data().meta.label.clone();
        Some(tree)
    }
}
#[crate::node("live_creation_item")]
struct LiveCreationItem {}
#[crate::node("live_creation_item", from_struct)]
impl Node for LiveCreationItem {}
crate::define_node_enum!(
    enum LiveCreationNode {
        LiveCreationFactory,
        LiveCreationItem,
    }
);

#[test]
fn live_user_item_creation_reads_bounded_current_nodes_and_decode_factory_stays_intrinsic() {
    for prepared_ui in [false, true] {
        let root: LiveCreationNode = LiveCreationFactory::new().into();
        let mut engine = Engine::new(root);
        engine.nodes.get_mut(engine.root).unwrap().node_data_mut().meta.label = "Current document".into();
        let lookup = |id| engine.nodes.get(id).map(|node| node as &dyn Node);
        let context = UserItemCreationContext::new(engine.root, &lookup);
        let before = engine.tick_stats().snapshot_builds;
        let factory = engine.nodes.get(engine.root).unwrap();
        let contextual = factory
            .create_user_item_tree_with_context("live_creation_item", &context)
            .unwrap();
        assert_eq!(contextual.children[0].node.node_data().meta.label, "Current document");
        assert_eq!(engine.tick_stats().snapshot_builds, before);
        let intrinsic = factory.create_user_item_tree("live_creation_item").unwrap();
        assert_eq!(intrinsic.children[0].node.node_data().meta.label, "Intrinsic");
        if prepared_ui {
            let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
                parent: engine.root,
                node_type: "live_creation_item".into(),
                label: None,
                initial_params: vec![],
            });
            assert!(ack.success, "{ack:?}");
        } else {
            engine
                .queue_catalog_create(engine.root, "live_creation_item", None, None)
                .unwrap();
            engine.apply_edits().unwrap();
        }
        let created = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        let marker = engine.nodes.get(created).unwrap().node_data().first_child.unwrap();
        assert_eq!(
            engine.nodes.get(marker).unwrap().node_data().meta.label,
            "Current document"
        );
    }
}
