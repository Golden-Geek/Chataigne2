use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{
    edit::{Edit, NodeTree, UserItemTreeInsertion},
    events::EventKind,
    node::{Folder, Node, NodeCreationContext, NodeId, UserContainerRules},
    process_ctx::ProcessCtx,
};

static READY_WITH_NODE_WAKER_COUNT: AtomicUsize = AtomicUsize::new(0);

#[crate::node("ready_node_waker_probe")]
struct ReadyNodeWakerProbe {}

#[crate::node("ready_node_waker_probe", from_struct)]
impl Node for ReadyNodeWakerProbe {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if ctx.node_waker().is_ok() {
            READY_WITH_NODE_WAKER_COUNT.fetch_add(1, Ordering::SeqCst);
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
        ReadyNodeWakerProbe,
        ForestContainer,
        ForestCatalogItem,
        ExpansionProbe,
    }
);

#[crate::node("expansion_probe")]
struct ExpansionProbe {
    remaining: usize,
    runtime_active: bool,
    #[state(default = 0)]
    runtime_starts: usize,
}

#[crate::node("expansion_probe", from_struct)]
impl Node for ExpansionProbe {
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        if self.remaining > 0 {
            ctx.add_child(self.id(), Self::new(self.remaining - 1, false), None);
        }
    }

    fn on_node_ready(&mut self, _ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.runtime_active = true;
        self.runtime_starts += 1;
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.runtime_active = false;
    }
}

#[test]
fn atomic_expansion_failure_restores_the_original_graph_and_ready_state() {
    let mut engine = Engine::new(LifecycleTestNode::from(Folder::new("root")));
    engine.add_node(ExpansionProbe::new(0, false).into(), None);
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.clear_history();
    engine.set_lifecycle_limits(crate::engine::LifecycleLimits {
        max_passes: std::num::NonZeroUsize::new(2).unwrap(),
        ..Default::default()
    });
    let before = engine.ui_event_log().to_vec();
    let result: Result<(), EngineEditError> = engine.apply_atomic_edit_operation(|engine| {
        // Multiple payload handoffs still restore one live root exactly once.
        engine.edits.push(Edit::ReplaceNode {
            node: id,
            new_node: Box::new(ExpansionProbe::new(0, false)),
        });
        engine.edits.push(Edit::ReplaceNode {
            node: id,
            new_node: Box::new(ExpansionProbe::new(20, false)),
        });
        engine.apply_edits()
    });
    assert!(matches!(result, Err(EngineEditError::LifecycleBudgetExceeded { .. })));
    assert_eq!(engine.nodes.len(), 2);
    assert_eq!(engine.undo_len(), 0);
    assert_eq!(engine.ui_event_log(), before);
    let LifecycleTestNode::ExpansionProbe(node) = engine.nodes.get(id).unwrap() else {
        panic!("probe")
    };
    assert_eq!(node.remaining, 0);
    assert!(node.runtime_active, "rollback reactivates the original payload");
    assert_eq!(node.runtime_starts, 2, "one initial start and one rollback start");
    assert!(engine.edits.pending.is_empty());
}

#[test]
fn project_load_augmentation_uses_the_same_expansion_guard() {
    let mut engine = Engine::new(LifecycleTestNode::from(Folder::new("root")));
    engine.set_lifecycle_limits(crate::engine::LifecycleLimits {
        max_passes: std::num::NonZeroUsize::new(2).unwrap(),
        ..Default::default()
    });
    let result =
        engine.apply_project_load_node_trees(vec![NodeTree::new(ExpansionProbe::new(20, false))], engine.root, None);
    assert!(matches!(result, Err(EngineEditError::LifecycleBudgetExceeded { .. })));
    assert!(engine.nodes.len() <= 4);
    assert!(engine.edits.pending.is_empty());
    assert_eq!(engine.undo_len(), 0);
}

#[test]
fn batched_ready_callbacks_can_reserve_node_wakers() {
    READY_WITH_NODE_WAKER_COUNT.store(0, Ordering::SeqCst);
    let root: LifecycleTestNode = Folder::new("root".to_owned()).into();
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::AddNodeTree {
        parent: engine.root,
        prev_sibling: None,
        tree: NodeTree::new(ReadyNodeWakerProbe::new()),
    });
    engine.apply_edits().expect("batched node tree should be added");

    assert_eq!(READY_WITH_NODE_WAKER_COUNT.load(Ordering::SeqCst), 1);
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
