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
        (kind == "live_creation_item").then(|| {
            NodeTree::new(LiveCreationItem::new())
                .with_child(NodeTree::new(Folder::new("Intrinsic")))
                .with_child(NodeTree::new(Folder::new("Curated")).as_user_item())
        })
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
impl Node for LiveCreationItem {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[crate::node::FOLDER_NODE_TYPE]))
    }
}
crate::define_node_enum!(
    enum LiveCreationNode {
        LiveCreationFactory,
        LiveCreationItem,
    }
);

#[test]
fn declared_parameter_folders_close_catalog_inheritance_without_closing_ordinary_folders() {
    for declaration in ["parameters", "parameters/nested"] {
        let mut engine = Engine::<LiveCreationNode>::new(LiveCreationFactory::new().into());
        let open = Folder::new("Parameters");
        let open_uuid = open.node_data().meta.uuid;
        let mut closed = Folder::new_schema_owned("Declared parameters");
        closed.node_data_mut().meta.decl_id = crate::node::DeclId(declaration.into());
        let closed_uuid = closed.node_data().meta.uuid;
        let descendant = Folder::new("Nested");
        let descendant_uuid = descendant.node_data().meta.uuid;
        engine.edits.push(crate::edit::Edit::AddNodeTree {
            parent: engine.root,
            prev_sibling: None,
            tree: NodeTree::new(open).with_child(NodeTree::new(closed).with_child(NodeTree::new(descendant))),
        });
        engine.apply_edits().unwrap();
        let open = engine.node_id_by_uuid(open_uuid).unwrap();
        let closed = engine.node_id_by_uuid(closed_uuid).unwrap();
        let descendant = engine.node_id_by_uuid(descendant_uuid).unwrap();
        assert_eq!(engine.catalog_creatable_items(open).len(), 1);
        for parent in [closed, descendant] {
            assert!(engine.catalog_creatable_items(parent).is_empty());
            let before = engine.nodes.len();
            assert!(
                engine
                    .queue_catalog_create(parent, "live_creation_item", None, None)
                    .is_err()
            );
            let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
                parent,
                node_type: "live_creation_item".into(),
                label: None,
                initial_params: vec![],
            });
            assert!(!ack.success, "{ack:?}");
            assert!(ack.created_node.is_none());
            assert_eq!(engine.nodes.len(), before);
        }
        let project = engine.to_project_file_with(Node::project_encode_data).unwrap();
        let loaded = Engine::<LiveCreationNode>::from_project_file_with(
            project,
            <LiveCreationNode as crate::app::ProjectNode>::project_decode_node,
        )
        .unwrap();
        let restored_closed = loaded.node_id_by_uuid(closed_uuid).unwrap();
        let restored_open = loaded.node_id_by_uuid(open_uuid).unwrap();
        assert!(loaded.catalog_creatable_items(restored_closed).is_empty());
        assert_eq!(loaded.catalog_creatable_items(restored_open).len(), 1);
    }
}

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
            let receipt = ack.created_node.expect("committed root identity");
            let created = engine.nodes.get(receipt.node_id).unwrap();
            assert_eq!(created.get_type(), "live_creation_item");
            assert_eq!(created.node_data().meta.uuid, receipt.uuid);
            assert_eq!(created.node_data().parent, Some(engine.root));
        } else {
            engine
                .queue_catalog_create(engine.root, "live_creation_item", None, None)
                .unwrap();
            engine.apply_edits().unwrap();
        }
        let created = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        let marker = engine.nodes.get(created).unwrap().node_data().first_child.unwrap();
        let curated = engine.nodes.get(created).unwrap().node_data().last_child.unwrap();
        assert_ne!(curated, marker);
        assert_eq!(
            engine.nodes.get(curated).unwrap().node_data().user_role,
            crate::node::UserNodeRole::ItemRoot
        );
        assert_eq!(
            engine.nodes.get(marker).unwrap().node_data().meta.label,
            "Current document"
        );
    }
}
