use super::*;
use crate::edit::{NodeTree, ReadRevision};

#[test]
fn a_scoped_fence_does_not_clone_ten_thousand_unrelated_records() {
    let (mut engine, source, _) = fixture();
    let mut tree = NodeTree::new(GuardNode::from(Folder::new("unrelated")));
    for index in 0..10_000 {
        tree = tree.with_child(NodeTree::new(GuardNode::from(Folder::new(format!("item {index}")))));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.build_process_tree_snapshot();
    let guard = NodeReadGuard::capture_subtree(&snapshot, source).unwrap();
    let before = engine.tick_stats().snapshot_builds;
    check(&mut engine, vec![guard; 32], vec![]);
    engine.apply_edits().unwrap();
    assert_eq!(engine.tick_stats().snapshot_builds, before);
    let mut folded = std::collections::HashMap::new();
    assert!(engine.tree_revisions.subtree_matches(
        &engine.nodes,
        source,
        snapshot.subtree_revision(source).unwrap(),
        &mut folded,
    ));
    assert_eq!(folded.len(), 1, "disjoint payloads are outside the live fold");
}

#[test]
fn each_queued_fence_rechecks_the_applied_prefix() {
    let (mut engine, source, target) = fixture();
    let guard = NodeReadGuard::capture_subtree(&engine.build_process_tree_snapshot(), engine.root).unwrap();
    check(&mut engine, vec![guard], vec![]);
    write(&mut engine, source, 3.0);
    check(&mut engine, vec![guard], vec![]);
    write(&mut engine, target, 5.0);
    let receipt = engine.edits.completion();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { edit_index: 2 })
    ));
    assert_eq!(value(&engine, source), ParamValue::Float(3.0));
    assert_eq!(value(&engine, target), ParamValue::Float(2.0));
    assert_eq!(receipt.status(), EditCompletionStatus::Failed);
}

#[test]
fn a_newer_disjoint_stamp_is_not_an_upper_bound_subtree_guard() {
    let (mut engine, source, target) = fixture();
    write(&mut engine, target, 3.0);
    engine.apply_edits().unwrap();
    let snapshot = engine.build_process_tree_snapshot();
    let mut guard = NodeReadGuard::capture_subtree(&snapshot, source).unwrap();
    assert_ne!(snapshot.subtree_revision(source), snapshot.subtree_revision(target));
    guard.revision = ReadRevision::Subtree(snapshot.subtree_revision(target).unwrap());
    check(&mut engine, vec![guard], vec![]);
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
}

#[test]
fn deep_overlapping_subtrees_fold_iteratively_and_keep_exact_snapshot_stamps() {
    let mut engine = Engine::new(GuardNode::from(Folder::new("root")));
    let mut parent = engine.root;
    // Construct a valid deep topology directly to isolate revision traversal from
    // insertion/lifecycle costs; the separate wide fixture uses production insertion.
    for _ in 0..20_000 {
        let child = engine.nodes.insert(GuardNode::from(Folder::new("child")));
        engine.nodes.get_mut(child).unwrap().node_data_mut().parent = Some(parent);
        let data = engine.nodes.get_mut(parent).unwrap().node_data_mut();
        data.first_child = Some(child);
        data.last_child = Some(child);
        parent = child;
    }
    engine.mark_tree_changed(parent);
    let snapshot = engine.build_process_tree_snapshot();
    let mut folded = std::collections::HashMap::new();
    for root in [engine.root, parent] {
        assert!(engine.tree_revisions.subtree_matches(
            &engine.nodes,
            root,
            snapshot.subtree_revision(root).unwrap(),
            &mut folded,
        ));
    }
    assert_eq!(folded.len(), 20_001);
    let guard = NodeReadGuard::capture_subtree(&snapshot, engine.root).unwrap();
    let before = engine.tick_stats().snapshot_builds;
    check(&mut engine, vec![guard], vec![]);
    engine.apply_edits().unwrap();
    assert_eq!(engine.tick_stats().snapshot_builds, before);
    engine.mark_tree_changed(parent);
    check(&mut engine, vec![guard], vec![]);
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
}
