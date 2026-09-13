use super::*;
use super::super::{AlchemistInputSocket, AlchemistOutputSocket};

#[test]
fn anode_lifecycle_snapshots_follow_the_reconciliation_stage() {
    let anode = AlchemistANode::new();
    assert!(anode.attached_requires_tree_snapshot());
    assert!(!anode.init_requires_tree_snapshot());
    assert!(anode.ready_requires_tree_snapshot());

    let input = AlchemistInputSocket::new();
    let output = AlchemistOutputSocket::new();
    assert!(!input.lifecycle_requires_tree_snapshot());
    assert!(!output.lifecycle_requires_tree_snapshot());
}

#[test]
fn duplicating_constant_reuses_one_lifecycle_snapshot() {
    let (mut engine, formula) = engine_with_formula();
    let source = create_anode(&mut engine, formula, "constant", 2.0, 3.0);
    assert!(
        engine
            .nodes
            .get(source)
            .is_some_and(Node::attached_snapshot_reusable_for_ready),
        "the app node registry must forward the snapshot reuse contract",
    );
    let before = engine.tick_stats().lifecycle_snapshot_builds;

    let duplicated = duplicate_anode(&mut engine, formula, source, Some(source), "Constant Copy");

    assert_eq!(engine.tick_stats().lifecycle_snapshot_builds - before, 1);
    assert_eq!(anode_position(&engine, duplicated), Some((3.5, 4.5)));
}
