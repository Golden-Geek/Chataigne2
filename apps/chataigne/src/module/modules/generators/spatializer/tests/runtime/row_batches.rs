use super::super::super::{source_folder_uuid, value_decl_id, value_folder};
use super::*;

fn fixture(additional: usize) -> (crate::app::AppEngine, NodeId, NodeId, NodeId) {
    let (mut engine, id, _, cell) = target_radius_fixture();
    let sources = find_path(&engine, id, "parameters/sources").unwrap();
    let values = find_path(&engine, id, "values").unwrap();
    for _ in 0..additional {
        engine.add_user_item(SpatializerSourceItem::new().into(), Some(sources));
    }
    engine.apply_edits().unwrap();
    (engine, id, values, cell)
}

fn row_count(edits: &EditQueue, values: NodeId) -> usize {
    edits
        .pending
        .iter()
        .filter(|request| matches!(request.edit, Edit::AddNodeTree { parent, .. } if parent == values))
        .count()
}

fn next_rows(engine: &mut crate::app::AppEngine, id: NodeId, values: NodeId) -> EditQueue {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "row forest did not arrive");
        let edits = direct_turn(engine, id);
        if row_count(&edits, values) != 0 {
            return edits;
        }
        apply(engine, edits);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn stage_rows(engine: &mut crate::app::AppEngine, id: NodeId, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while module(engine, id).service.staged_rows() < count {
        assert!(Instant::now() < deadline, "rows were not staged");
        let edits = direct_turn(engine, id);
        apply(engine, edits);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn first_new_source(engine: &crate::app::AppEngine, id: NodeId) -> NodeId {
    nth_child(engine, find_path(engine, id, "parameters/sources").unwrap(), 1).unwrap()
}

#[test]
fn small_rows_publish_in_bounded_forests_and_preserve_existing_cells() {
    let additional = 35;
    let (mut engine, id, values, original) = fixture(additional);
    let mut submissions = 0;
    let mut inserted = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline);
        let edits = direct_turn(&mut engine, id);
        let count = row_count(&edits, values);
        assert!(count <= publication::ROWS_PER_BATCH);
        inserted += count;
        submissions += usize::from(count != 0);
        apply(&mut engine, edits);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        if module(&mut engine, id).service.idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(inserted, additional);
    assert!(submissions < additional, "rows must share guarded application turns");
    assert_eq!(child_count(&engine, values), additional + 1);
    assert_eq!(default_cell(&engine, id), original);
    for index in 0..=additional {
        let row = nth_child(&engine, values, index).unwrap();
        assert_eq!(child_count(&engine, row), 1);
        assert_eq!(float(&engine, nth_child(&engine, row, 0).unwrap()), 1.0);
    }
    eprintln!("{additional} new small rows used {submissions} guarded application turns");
}

#[test]
fn a_queued_forest_rejects_new_source_state_before_callbacks() {
    let (mut engine, id, values, original) = fixture(20);
    let mut edits = next_rows(&mut engine, id, values);
    assert!(row_count(&edits, values) > 1);
    let receipt = edits.completion();
    let source = first_new_source(&engine, id);
    let position = find_child_by_key(&engine, source, "position_2d").unwrap();
    engine.edits.push(Edit::SetParam {
        node: position,
        value: ParamValue::Vec2(0.4, 0.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(apply_failure(&mut engine, edits));
    assert_eq!(receipt.status(), EditCompletionStatus::Failed);
    assert_eq!(child_count(&engine, values), 1);
    stabilize(&mut engine);
    let uuid = source_folder_uuid(engine.nodes.get(source).unwrap().node_data().meta.uuid);
    let row = engine.node_id_by_uuid(uuid).unwrap();
    assert!((float(&engine, nth_child(&engine, row, 0).unwrap()) - 0.6).abs() < 1e-12);
    assert_eq!(child_count(&engine, values), 21);
    assert_eq!(default_cell(&engine, id), original);
}

#[test]
fn an_externally_restored_row_supersedes_the_staged_forest() {
    let (mut engine, id, values, original) = fixture(8);
    stage_rows(&mut engine, id, 3);
    let source = first_new_source(&engine, id);
    let meta = &engine.nodes.get(source).unwrap().node_data().meta;
    let uuid = source_folder_uuid(meta.uuid);
    let decl = value_decl_id("spatializer_source", meta.uuid);
    let restored = value_folder(&meta.label, &decl, uuid);
    engine.add_node(restored.into(), Some(values));
    engine.apply_edits().unwrap();
    let restored = engine.node_id_by_uuid(uuid).unwrap();
    stabilize(&mut engine);
    assert_eq!(engine.node_id_by_uuid(uuid), Some(restored));
    assert_eq!(child_count(&engine, values), 9);
    assert_eq!(child_count(&engine, restored), 1);
    assert_eq!(float(&engine, nth_child(&engine, restored, 0).unwrap()), 1.0);
    assert_eq!(default_cell(&engine, id), original);
}

#[test]
fn an_applied_forest_prefix_survives_a_failed_suffix() {
    let (mut engine, id, values, original) = fixture(20);
    let mut edits = next_rows(&mut engine, id, values);
    let trees: Vec<_> = edits
        .pending
        .iter()
        .enumerate()
        .filter_map(|(index, request)| match &request.edit {
            Edit::AddNodeTree { parent, tree, .. } if *parent == values => {
                Some((index, tree.node.node_data().meta.uuid))
            }
            _ => None,
        })
        .collect();
    assert!(trees.len() > 1);
    let receipt = edits.completion();
    let mut invalid = EditQueue::new();
    invalid.push(Edit::RemoveNode { node: NodeId(u64::MAX) });
    edits.pending.insert(trees[0].0 + 1, invalid.pending.pop().unwrap());
    assert!(apply_failure(&mut engine, edits));
    assert_eq!(receipt.status(), EditCompletionStatus::Failed);
    let applied = engine.node_id_by_uuid(trees[0].1).unwrap();
    let cell = nth_child(&engine, applied, 0).unwrap();
    assert!(engine.node_id_by_uuid(trees[1].1).is_none());
    stabilize(&mut engine);
    assert_eq!(engine.node_id_by_uuid(trees[0].1), Some(applied));
    assert_eq!(nth_child(&engine, applied, 0), Some(cell));
    assert_eq!(child_count(&engine, values), 21);
    assert_eq!(default_cell(&engine, id), original);
}

#[test]
fn shared_row_pressure_flushes_an_admitted_prefix_without_waiting_for_more_slots() {
    let (mut engine, id, values, _) = fixture(5);
    let pool = publication::rows().unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let held: Vec<_> = (0..14).map(|_| pool.try_reserve(1).unwrap()).collect();
    let mut largest = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "staged rows cannot deadlock under shared pressure"
        );
        let edits = direct_turn(&mut engine, id);
        let count = row_count(&edits, values);
        assert!(count <= 2);
        largest = largest.max(count);
        apply(&mut engine, edits);
        if module(&mut engine, id).service.idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(largest, 2);
    assert_eq!(child_count(&engine, values), 6);
    drop(held);
}

#[test]
fn a_queued_forest_cannot_follow_a_moved_values_folder() {
    let (mut engine, id, values, _) = fixture(20);
    let mut edits = next_rows(&mut engine, id, values);
    let receipt = edits.completion();
    engine.edits.push(Edit::MoveNode {
        node: values,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    assert!(apply_failure(&mut engine, edits));
    assert_eq!(receipt.status(), EditCompletionStatus::Failed);
    assert_eq!(child_count(&engine, values), 1);
}
