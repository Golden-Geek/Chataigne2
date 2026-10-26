use super::*;
use crate::{
    application::apply_project_intent,
    edit::NodeTree,
    node::{Folder, Node, UserContainerRules, UserCreatableItem},
    ui_sync::UiEditIntent,
};

#[crate::node("capture_factory")]
struct CaptureFactory {}

#[crate::node("capture_factory", from_struct)]
impl Node for CaptureFactory {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["capture_item"]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new("capture_item", "capture_item", "Item")]
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        (node_type == "capture_item").then(|| {
            NodeTree::new(CaptureItem::new()).with_child(
                NodeTree::new(Folder::new("Prepared child"))
                    .with_child(NodeTree::new(Folder::new("Prepared grandchild"))),
            )
        })
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[crate::node("capture_item")]
struct CaptureItem {}

#[crate::node("capture_item", from_struct)]
impl Node for CaptureItem {
    fn user_item_kind(&self) -> &str {
        "capture_item"
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

crate::define_node_enum!(
    enum CaptureNode {
        CaptureFactory,
        CaptureItem,
    }
);
impl ProjectLifecycle for CaptureNode {}

#[test]
fn catalog_creation_publishes_every_prepared_descendant_to_the_project_capture() {
    let mut engine = Engine::<CaptureNode>::new(Folder::new("root").into());
    engine.add_node(CaptureFactory::new().into(), None);
    engine.apply_edits().unwrap();
    let parent = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let projection = ProjectDocumentReadModel::from_engine(&mut engine).unwrap();
    let before = projection.capture().unwrap();
    let initial = serde_json::to_value(before.materialize::<CaptureNode>(None).unwrap()).unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: "capture_item".into(),
            label: Some("Created".into()),
            initial_params: Vec::new(),
        },
    );
    assert!(ack.success, "{ack:?}");
    projection.synchronize(&mut engine).unwrap();
    let after = projection.capture().unwrap();
    assert_eq!(after.node_count, engine.nodes.len());
    let captured = after.materialize::<CaptureNode>(None).unwrap();
    let live = crate::app::capture_sparse_project_file_with_ui_state(&engine, None).unwrap();
    assert_eq!(
        serde_json::to_value(captured).unwrap(),
        serde_json::to_value(live).unwrap()
    );
    assert_eq!(before.node_count, 2, "prior captures stay immutable");
    assert_eq!(
        serde_json::to_value(before.materialize::<CaptureNode>(None).unwrap()).unwrap(),
        initial
    );
    for intent in [UiEditIntent::Undo, UiEditIntent::Redo] {
        let ack = apply_project_intent(&mut engine, intent);
        assert!(ack.success, "{ack:?}");
        projection.synchronize(&mut engine).unwrap();
        let capture = projection.capture().unwrap();
        assert_eq!(capture.node_count, engine.nodes.len());
        assert_eq!(
            serde_json::to_value(capture.materialize::<CaptureNode>(None).unwrap()).unwrap(),
            serde_json::to_value(crate::app::capture_sparse_project_file_with_ui_state(&engine, None).unwrap())
                .unwrap()
        );
    }
}
