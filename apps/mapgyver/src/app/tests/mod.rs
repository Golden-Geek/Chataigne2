use golden_core::app::{create_new_project_engine, from_sparse_project_json, to_sparse_project_json_pretty};
use golden_core::node::{Folder, Node};

use super::AppNode;

#[test]
fn new_project_contains_only_the_standard_dashboard_at_the_root() {
    let mut engine = create_new_project_engine::<AppNode>().expect("create MapGyver project");
    engine.apply_edits().expect("attach default project");
    let children = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.node_data().parent == Some(engine.root))
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 1);
    assert!(matches!(children[0].1, AppNode::Dashboard(_)));
}

#[test]
fn app_node_registry_round_trips_a_project_through_the_public_codec() {
    let mut engine = create_new_project_engine::<AppNode>().expect("create MapGyver project");
    engine.add_node(Folder::new("Authored Folder").into(), None);
    engine.apply_edits().expect("attach authored content");

    let json = to_sparse_project_json_pretty(&engine).expect("save MapGyver project");
    let mut restored = from_sparse_project_json::<AppNode>(&json).expect("open MapGyver project");
    restored.apply_edits().expect("attach restored content");
    let authored = restored
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Authored Folder")
        .expect("authored folder survives reopen");
    assert!(matches!(authored.1, AppNode::Folder(_)));
    assert_eq!(authored.1.node_data().parent, Some(restored.root));
    assert_eq!(
        restored
            .nodes
            .iter()
            .filter(|(_, node)| matches!(node, AppNode::Dashboard(_)))
            .count(),
        1
    );
}
