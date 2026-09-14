use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{
    edit::{Edit, NodeTree, UserItemTreeInsertion},
    events::EventKind,
    node::{Folder, Node, NodeCreationContext, NodeId, UserContainerRules},
    process_ctx::ProcessCtx,
};

static READY_WITH_EXTERNAL_SENDER_COUNT: AtomicUsize = AtomicUsize::new(0);

#[crate::node("ready_external_sender_probe")]
struct ReadyExternalSenderProbe {}

#[crate::node("ready_external_sender_probe", from_struct)]
impl Node for ReadyExternalSenderProbe {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if ctx.external_edit_sender().is_some() {
            READY_WITH_EXTERNAL_SENDER_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

#[crate::node("forest_container")]
struct ForestContainer {}

#[crate::node("forest_container", from_struct)]
impl Node for ForestContainer {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[]))
    }

    fn user_container_accepts_item(&self, _item_type: &str, _item_kind: &str) -> bool {
        true
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        (node_type == "forest_catalog_item")
            .then(|| NodeTree::new(ForestCatalogItem::new()).with_child(NodeTree::new(Folder::new("Declared"))))
    }
}

#[crate::node("forest_catalog_item")]
struct ForestCatalogItem {}

#[crate::node("forest_catalog_item", from_struct)]
impl Node for ForestCatalogItem {
    fn user_creatable_items_require_tree_snapshot(&self) -> bool {
        true
    }
}

crate::define_node_enum!(
    enum LifecycleTestNode {
        ReadyExternalSenderProbe,
        ForestContainer,
        ForestCatalogItem,
    }
);

#[test]
fn batched_ready_callbacks_receive_external_edit_sender() {
    READY_WITH_EXTERNAL_SENDER_COUNT.store(0, Ordering::SeqCst);
    let root: LifecycleTestNode = Folder::new("root".to_owned()).into();
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::AddNodeTree {
        parent: engine.root,
        prev_sibling: None,
        tree: NodeTree::new(ReadyExternalSenderProbe::new()),
    });
    engine.apply_edits().expect("batched node tree should be added");

    assert_eq!(READY_WITH_EXTERNAL_SENDER_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn node_enum_forwards_detached_user_item_tree_creation() {
    let container: LifecycleTestNode = ForestContainer::new().into();
    let tree = container.create_user_item_tree("forest_catalog_item").unwrap();
    assert_eq!(tree.node_type(), "forest_catalog_item");
    assert_eq!(tree.children.len(), 1);
}

#[test]
fn user_item_forest_inserts_in_order_and_replays_one_history_transaction() {
    let root: LifecycleTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    let first = ForestContainer::new();
    let first_uuid = first.node_data().meta.uuid;
    let second = ForestContainer::new();
    let second_uuid = second.node_data().meta.uuid;
    engine.add_node(first.into(), None);
    engine.add_node(second.into(), None);
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let first_parent = snapshot.node_id_by_uuid(first_uuid).unwrap();
    let second_parent = snapshot.node_id_by_uuid(second_uuid).unwrap();
    let before = engine.tick_stats().snapshot_builds;

    engine.add_user_item_trees(vec![
        UserItemTreeInsertion::new(first_parent, NodeTree::new(Folder::new("Repeated"))),
        UserItemTreeInsertion::new(second_parent, NodeTree::new(Folder::new("Other"))),
        UserItemTreeInsertion::new(first_parent, NodeTree::new(Folder::new("Repeated"))),
    ]);
    engine.apply_edits().unwrap();
    let added = engine.tick_stats().snapshot_builds - before;
    assert!(added <= 3, "forest lifecycle should share its snapshots; built {added}");

    let snapshot = engine.process_tree_snapshot();
    let first_children = snapshot.child_ids(first_parent);
    let second_children = snapshot.child_ids(second_parent);
    assert_eq!(first_children.len(), 2);
    assert_eq!(second_children.len(), 1);
    assert_eq!(snapshot.node(first_children[0]).unwrap().label, "Repeated");
    assert_eq!(snapshot.node(first_children[1]).unwrap().label, "Repeated 2");
    assert_eq!(snapshot.node(second_children[0]).unwrap().label, "Other");
    let item_ids = [first_children[0], second_children[0], first_children[1]];
    assert_eq!(
        engine.undo_len(),
        2,
        "the forest should add one transaction after the parent transaction"
    );

    assert!(engine.undo().unwrap());
    assert!(item_ids.iter().all(|id| engine.nodes.get(*id).is_none()));
    assert!(engine.redo().unwrap());
    assert!(item_ids.iter().all(|id| engine.nodes.get(*id).is_some()));
    let restored = engine.process_tree_snapshot();
    assert_eq!(restored.child_ids(first_parent), first_children);
    assert_eq!(restored.child_ids(second_parent), second_children);
}

#[test]
fn user_item_forest_rejects_a_missing_later_parent_before_insertion() {
    let root: LifecycleTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    let parent = ForestContainer::new();
    let parent_uuid = parent.node_data().meta.uuid;
    engine.add_node(parent.into(), None);
    engine.apply_edits().unwrap();
    let parent = engine.process_tree_snapshot().node_id_by_uuid(parent_uuid).unwrap();
    let before = engine.nodes.len();
    engine.add_user_item_trees(vec![
        UserItemTreeInsertion::new(parent, NodeTree::new(Folder::new("Valid"))),
        UserItemTreeInsertion::new(NodeId(u64::MAX), NodeTree::new(Folder::new("Invalid"))),
    ]);
    assert!(engine.apply_edits().is_err());
    assert_eq!(engine.nodes.len(), before);
    assert!(engine.process_tree_snapshot().child_ids(parent).is_empty());
}

#[test]
fn user_item_forest_emits_one_atomic_ui_transaction() {
    let root: LifecycleTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    let parent = ForestContainer::new();
    let parent_uuid = parent.node_data().meta.uuid;
    engine.add_node(parent.into(), None);
    engine.apply_edits().unwrap();
    let parent = engine.process_tree_snapshot().node_id_by_uuid(parent_uuid).unwrap();
    engine.clear_ui_event_log();
    let before = engine.tick_stats().snapshot_builds;

    engine.add_user_item_trees(
        (0..3)
            .map(|_| {
                let mut tree = NodeTree::new(ForestCatalogItem::new());
                for _ in 0..9 {
                    tree.push_child(NodeTree::new(Folder::new("Declared")));
                }
                UserItemTreeInsertion::new(parent, tree)
            })
            .collect(),
    );
    engine.apply_edits().unwrap();

    let added = engine.tick_stats().snapshot_builds - before;
    assert!(added <= 3, "forest should share lifecycle snapshots; built {added}");
    assert_eq!(engine.process_tree_snapshot().child_ids(parent).len(), 3);
    let transactions = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::GraphTransaction { transaction } => Some(transaction),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        transactions.len(),
        1,
        "the forest should publish one atomic UI transaction"
    );
    assert_eq!(
        transactions[0]
            .ops
            .iter()
            .filter(|op| matches!(op, crate::ui_sync::UiGraphOp::SubtreeInserted { .. }))
            .count(),
        3
    );
}
