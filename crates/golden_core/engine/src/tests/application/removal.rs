use super::*;

#[test]
fn remove_nodes_batch_restores_nonadjacent_siblings_in_one_ui_transaction() {
    let mut engine = Engine::new(Folder::new("Root"));
    for label in ["A", "B", "C"] {
        engine.add_node(Folder::new(label), None);
    }
    engine.apply_edits().expect("siblings should attach");
    let root = engine.root;
    let before = engine.ui_direct_children(root).expect("root children");
    engine.clear_history();
    engine.clear_ui_event_log();

    let acknowledgement = engine.apply_ui_intent(UiEditIntent::RemoveNodes {
        nodes: vec![before[0], before[2]],
    });
    assert!(acknowledgement.success, "remove should succeed: {acknowledgement:?}");
    assert_eq!(engine.undo_len(), 1);
    assert_eq!(engine.ui_direct_children(root), Some(vec![before[1]]));
    let removal_batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let removal_transactions = removal_batch
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::GraphTransaction { transaction } => Some(transaction),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(removal_transactions.len(), 1);
    assert_eq!(removal_transactions[0].ops.len(), 2);
    assert!(matches!(
        &removal_transactions[0].ops[0],
        UiGraphOp::SubtreeRemoved { parent_after: None, .. }
    ));
    assert!(matches!(
        &removal_transactions[0].ops[1],
        UiGraphOp::SubtreeRemoved { parent_after: Some(patch), .. }
            if patch.parent == root && patch.children == vec![before[1]]
    ));

    engine.clear_ui_event_log();
    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.ui_direct_children(root), Some(before.clone()));
    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let graph_transactions = batch
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::GraphTransaction { transaction } => Some(transaction),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(graph_transactions.len(), 1);
    assert_eq!(graph_transactions[0].ops.len(), 2);
    assert!(matches!(
        &graph_transactions[0].ops[0],
        UiGraphOp::SubtreeInserted {
            parent_children_after: None,
            ..
        }
    ));
    assert!(matches!(
        &graph_transactions[0].ops[1],
        UiGraphOp::SubtreeInserted { parent_children_after: Some(children), .. } if children == &before
    ));

    engine.clear_ui_event_log();
    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.ui_direct_children(root), Some(vec![before[1]]));
    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let graph_transactions = batch
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::GraphTransaction { transaction } => Some(transaction),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(graph_transactions.len(), 1);
    assert_eq!(graph_transactions[0].ops.len(), 2);
    assert!(matches!(
        &graph_transactions[0].ops[0],
        UiGraphOp::SubtreeRemoved { parent_after: None, .. }
    ));
    assert!(matches!(
        &graph_transactions[0].ops[1],
        UiGraphOp::SubtreeRemoved { parent_after: Some(patch), .. }
            if patch.parent == root && patch.children == vec![before[1]]
    ));
}

#[test]
fn history_remove_batch_reuses_ui_catalog_snapshot_for_ready_callbacks() {
    HISTORY_READY_SNAPSHOT_PROBE_COUNT.store(0, Ordering::SeqCst);
    let mut engine: Engine<FacadeTestNode> = Engine::new(Folder::new("Root").into());
    engine.add_node(HistoryReadySnapshotProbe::new().into(), None);
    engine.add_node(HistoryReadySnapshotProbe::new().into(), None);
    engine.apply_edits().expect("probes should attach");
    let roots = engine.ui_direct_children(engine.root).expect("root children");
    engine.clear_history();

    let acknowledgement = engine.apply_ui_intent(UiEditIntent::RemoveNodes { nodes: roots });
    assert!(acknowledgement.success, "remove should succeed: {acknowledgement:?}");
    let ready_before = HISTORY_READY_SNAPSHOT_PROBE_COUNT.load(Ordering::SeqCst);
    let lifecycle_builds_before = engine.tick_stats().snapshot_builds;

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        HISTORY_READY_SNAPSHOT_PROBE_COUNT.load(Ordering::SeqCst),
        ready_before + 2
    );
    assert_eq!(engine.tick_stats().snapshot_builds, lifecycle_builds_before);
}

#[test]
fn remove_nodes_collapses_selected_descendants_regardless_of_selection_order() {
    for descendants_first in [true, false] {
        let mut engine = Engine::new(Folder::new("Root"));
        engine.add_node(Folder::new("A"), None);
        engine.add_node(Folder::new("B"), None);
        engine.apply_edits().expect("parents should attach");
        let root = engine.root;
        let parents = engine.ui_direct_children(root).expect("root children");
        engine.add_node(Folder::new("A child"), Some(parents[0]));
        engine.add_node(Folder::new("B child"), Some(parents[1]));
        engine.apply_edits().expect("children should attach");
        let a_child = engine.ui_direct_children(parents[0]).expect("A child")[0];
        let b_child = engine.ui_direct_children(parents[1]).expect("B child")[0];
        engine.clear_history();

        let nodes = if descendants_first {
            vec![a_child, parents[1], parents[0], b_child]
        } else {
            vec![parents[0], a_child, b_child, parents[1]]
        };
        let acknowledgement = engine.apply_ui_intent(UiEditIntent::RemoveNodes { nodes });
        assert!(acknowledgement.success, "remove should succeed: {acknowledgement:?}");
        assert_eq!(engine.undo_len(), 1);
        assert_eq!(engine.ui_direct_children(root), Some(vec![]));

        assert!(engine.undo().expect("undo should succeed"));
        assert_eq!(engine.ui_direct_children(root), Some(parents.clone()));
        assert_eq!(engine.ui_direct_children(parents[0]), Some(vec![a_child]));
        assert_eq!(engine.ui_direct_children(parents[1]), Some(vec![b_child]));

        assert!(engine.redo().expect("redo should succeed"));
        assert_eq!(engine.ui_direct_children(root), Some(vec![]));
    }
}

#[test]
fn remove_nodes_mixed_parent_selection_replays_exact_sibling_order() {
    let mut engine = Engine::new(Folder::new("Root"));
    engine.add_node(Folder::new("A"), None);
    engine.add_node(Folder::new("B"), None);
    engine.apply_edits().expect("parents should attach");
    let parents = engine.ui_direct_children(engine.root).expect("root children");
    for parent in &parents {
        engine.add_node(Folder::new("First"), Some(*parent));
        engine.add_node(Folder::new("Second"), Some(*parent));
    }
    engine.apply_edits().expect("children should attach");
    let a_children = engine.ui_direct_children(parents[0]).expect("A children");
    let b_children = engine.ui_direct_children(parents[1]).expect("B children");
    engine.clear_history();
    engine.clear_ui_event_log();

    let acknowledgement = engine.apply_ui_intent(UiEditIntent::RemoveNodes {
        nodes: vec![a_children[0], b_children[1]],
    });
    assert!(acknowledgement.success, "remove should succeed: {acknowledgement:?}");
    assert_eq!(engine.undo_len(), 1);
    assert_eq!(engine.ui_direct_children(parents[0]), Some(vec![a_children[1]]));
    assert_eq!(engine.ui_direct_children(parents[1]), Some(vec![b_children[0]]));
    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let transactions = batch
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::GraphTransaction { transaction } => Some(transaction),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].ops.len(), 2);
    assert!(matches!(
        &transactions[0].ops[0],
        UiGraphOp::SubtreeRemoved { parent_after: Some(patch), .. }
            if patch.parent == parents[0] && patch.children == vec![a_children[1]]
    ));
    assert!(matches!(
        &transactions[0].ops[1],
        UiGraphOp::SubtreeRemoved { parent_after: Some(patch), .. }
            if patch.parent == parents[1] && patch.children == vec![b_children[0]]
    ));

    engine.clear_ui_event_log();
    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.ui_direct_children(parents[0]), Some(a_children.clone()));
    assert_eq!(engine.ui_direct_children(parents[1]), Some(b_children.clone()));
    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let transactions = batch
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::GraphTransaction { transaction } => Some(transaction),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].ops.len(), 2);
    assert!(matches!(
        &transactions[0].ops[0],
        UiGraphOp::SubtreeInserted { parent, parent_children_after, .. }
            if *parent == parents[1] && parent_children_after.as_ref() == Some(&b_children)
    ));
    assert!(matches!(
        &transactions[0].ops[1],
        UiGraphOp::SubtreeInserted { parent, parent_children_after, .. }
            if *parent == parents[0] && parent_children_after.as_ref() == Some(&a_children)
    ));

    engine.clear_ui_event_log();
    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.ui_direct_children(parents[0]), Some(vec![a_children[1]]));
    assert_eq!(engine.ui_direct_children(parents[1]), Some(vec![b_children[0]]));
    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let transactions = batch
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::GraphTransaction { transaction } => Some(transaction),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].ops.len(), 2);
    assert!(matches!(
        &transactions[0].ops[0],
        UiGraphOp::SubtreeRemoved { parent_after: Some(patch), .. }
            if patch.parent == parents[0] && patch.children == vec![a_children[1]]
    ));
    assert!(matches!(
        &transactions[0].ops[1],
        UiGraphOp::SubtreeRemoved { parent_after: Some(patch), .. }
            if patch.parent == parents[1] && patch.children == vec![b_children[0]]
    ));
}

#[test]
fn nested_removal_transaction_keeps_stepwise_replay() {
    let mut engine = Engine::new(Folder::new("Root"));
    engine.add_node(Folder::new("Parent"), None);
    engine.apply_edits().expect("parent should attach");
    let parent = engine.ui_direct_children(engine.root).expect("root children")[0];
    engine.add_node(Folder::new("Child"), Some(parent));
    engine.apply_edits().expect("child should attach");
    let child = engine.ui_direct_children(parent).expect("parent children")[0];
    engine.clear_history();

    engine.edits.push(crate::edit::Edit::RemoveNode { node: child });
    engine.edits.push(crate::edit::Edit::RemoveNode { node: parent });
    engine.apply_edits().expect("nested removals should apply in order");
    assert_eq!(engine.undo_len(), 1);
    assert_eq!(engine.ui_direct_children(engine.root), Some(vec![]));

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.ui_direct_children(engine.root), Some(vec![parent]));
    assert_eq!(engine.ui_direct_children(parent), Some(vec![child]));

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.ui_direct_children(engine.root), Some(vec![]));
}

#[test]
fn remove_nodes_rejects_any_invalid_target_before_removing_valid_nodes() {
    let mut engine: Engine<FacadeTestNode> = Engine::new(Folder::new("Root").into());
    engine.add_node(Folder::new("A").into(), None);
    engine.add_node(Folder::new("B").into(), None);
    engine.apply_edits().expect("parents should attach");
    let root = engine.root;
    let first = engine.ui_direct_children(root).expect("root children")[0];
    engine.clear_history();
    engine.clear_ui_event_log();

    assert_rejected_intent_is_atomic(
        &mut engine,
        UiEditIntent::RemoveNodes {
            nodes: vec![first, NodeId(u64::MAX)],
        },
    );
    assert_rejected_intent_is_atomic(
        &mut engine,
        UiEditIntent::RemoveNodes {
            nodes: vec![first, root],
        },
    );
}
