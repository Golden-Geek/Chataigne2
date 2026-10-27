use super::*;
use golden_io::{RetainedPayload, RetirementPool, RevisionSource, WorkerNotifier};

fn fixture(sources: usize, targets: usize) -> (crate::app::AppEngine, NodeId, NodeId) {
    let (mut engine, id, _, cell) = target_radius_fixture();
    let source_list = find_path(&engine, id, "parameters/sources").unwrap();
    let target_list = find_path(&engine, id, "parameters/targets").unwrap();
    for _ in 1..sources {
        engine.add_user_item(SpatializerSourceItem::new().into(), Some(source_list));
    }
    for _ in 1..targets {
        engine.add_user_item(SpatializerTargetItem::new().into(), Some(target_list));
    }
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    (engine, id, cell)
}

fn prepare(engine: &crate::app::AppEngine, id: NodeId) -> RetainedPayload<worker::Prepared> {
    let owner = RetirementPool::new(std::num::NonZeroUsize::MIN);
    let life = RevisionSource::default();
    let job = worker::start(
        &engine.process_tree_snapshot(),
        worker::Roots {
            parameters: find_path(engine, id, "parameters").unwrap(),
            sources: find_path(engine, id, "parameters/sources").unwrap(),
            targets: find_path(engine, id, "parameters/targets").unwrap(),
            values: find_path(engine, id, "values").unwrap(),
        },
        &owner,
        life.reader().guard_at(0),
        WorkerNotifier::new(|| {}),
        None,
    )
    .unwrap();
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    let deadline = Instant::now() + Duration::from_secs(2);
    while job.running() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    job.take().unwrap().unwrap()
}

#[test]
fn canonical_matrix_has_no_endpoint_repairs_or_publication_work() {
    let (engine, id, _) = fixture(32, 129);
    let prepared = prepare(&engine, id);
    assert!(prepared.repairs.is_empty());
    let plan = prepared.output.as_ref().unwrap();
    assert!(plan.rows.is_empty());
    assert!(plan.removals.is_empty());
    assert_eq!(plan.known_outputs.len(), 32 * 129);
    assert_eq!(prepared.values.len(), 32 * 129);
}

#[test]
fn one_sparse_endpoint_is_the_only_normalization_work() {
    let (mut engine, id, _) = fixture(16, 40);
    let target = nth_child(&engine, find_path(&engine, id, "parameters/targets").unwrap(), 39).unwrap();
    let radius = find_child_by_key(&engine, target, "radius").unwrap();
    engine.edits.push(Edit::RemoveNode { node: radius });
    engine.apply_edits().unwrap();
    let prepared = prepare(&engine, id);
    assert_eq!(prepared.repairs, vec![(target, false)]);
    assert!(prepared.values.is_empty());
    assert!(prepared.output.is_none());
    drop(prepared);
    stabilize(&mut engine);
    assert!(find_child_by_key(&engine, target, "radius").is_some());
}

#[test]
fn changed_column_visits_only_changed_cells_and_batches_across_rows() {
    let (mut engine, id, original) = fixture(32, 129);
    let target = nth_child(&engine, find_path(&engine, id, "parameters/targets").unwrap(), 128).unwrap();
    set_position_2d(&mut engine, target, 0.5, 0.0);
    let prepared = prepare(&engine, id);
    let plan = prepared.output.as_ref().unwrap();
    assert_eq!(plan.rows.len(), 32);
    for row in &plan.rows {
        assert_eq!(row.pairs, vec![128]);
        assert!(row.removals.is_empty());
    }
    drop(prepared);
    let mut submissions = 0;
    let mut changed = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline);
        let edits = direct_turn(&mut engine, id);
        let count = edits
            .pending
            .iter()
            .filter(|r| matches!(r.edit, Edit::SetParam { .. }))
            .count();
        assert!(count <= publication::PAIRS_PER_TURN);
        submissions += usize::from(count > 0);
        changed += count;
        apply(&mut engine, edits);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        if module(&mut engine, id).service.idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(changed, 32);
    assert!(
        submissions <= 5,
        "changed rows should share guarded batches: {submissions}"
    );
    assert_eq!(default_cell(&engine, id), original);
    assert_eq!(float(&engine, original), 1.0);
}

#[test]
fn output_changed_during_worker_inspection_is_recaptured() {
    let (mut engine, id, original) = fixture(3, 20);
    let control = Arc::new(worker::TestControl::default());
    module(&mut engine, id).service.control = Some(control.clone());
    let source = source(&engine, id);
    set_position_2d(&mut engine, source, 0.5, 0.0);
    until(&mut engine, |_| control.entered.load(Ordering::Acquire));
    set_param(&mut engine, original, ParamValue::Float(0.25));
    module(&mut engine, id).service.control = None;
    control.released.store(true, Ordering::Release);
    stabilize(&mut engine);
    assert_eq!(default_cell(&engine, id), original);
    assert_eq!(float(&engine, original), 0.5);
    assert!(module(&mut engine, id).service.warning_text().is_none());
}

#[test]
fn canonical_target_creation_plans_only_the_new_column() {
    use chataigne_spatializer_protocol::*;
    let (mut engine, id, original) = fixture(32, 129);
    let identity = SpatializerIdentity {
        node: id,
        uuid: engine.nodes.get(id).unwrap().node_data().meta.uuid,
    };
    super::super::super::authoring::apply(
        &mut engine,
        SPATIALIZER_EDIT_OPERATION,
        serde_json::to_value(SpatializerEditRequest {
            module: identity,
            space: SpatializerSpace::Xy,
            edit: SpatializerEdit::CreateTarget { position: [2.0, 3.0] },
        })
        .unwrap(),
    )
    .unwrap();
    let prepared = prepare(&engine, id);
    assert!(prepared.repairs.is_empty());
    let plan = prepared.output.as_ref().unwrap();
    assert_eq!(plan.rows.len(), 32);
    assert!(plan
        .rows
        .iter()
        .all(|row| row.pairs == [129] && row.removals.is_empty()));
    drop(prepared);
    let values = find_path(&engine, id, "values").unwrap();
    let mut batches = 0;
    let mut inserted = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline);
        let edits = direct_turn(&mut engine, id);
        let count = edits
            .pending
            .iter()
            .filter(|r| matches!(r.edit, Edit::AddNodeTree { parent, .. } if parent != values))
            .count();
        assert!(count <= publication::PAIRS_PER_TURN);
        batches += usize::from(count > 0);
        inserted += count;
        apply(&mut engine, edits);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        if module(&mut engine, id).service.idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(inserted, 32);
    assert_eq!(batches, 2, "duplicate row fences must not fragment the 32-cell column");
    eprintln!("32 new column cells use {batches} guarded application batches");
    assert_eq!(default_cell(&engine, id), original);
    for index in 0..32 {
        assert_eq!(child_count(&engine, nth_child(&engine, values, index).unwrap()), 130);
    }
}

#[test]
fn normalization_rechecks_live_endpoint_bounds_after_earlier_repairs() {
    let (mut engine, id, original) = fixture(1, 2);
    let targets = find_path(&engine, id, "parameters/targets").unwrap();
    let first = nth_child(&engine, targets, 0).unwrap();
    let second = nth_child(&engine, targets, 1).unwrap();
    for target in [first, second] {
        let radius = find_child_by_key(&engine, target, "radius").unwrap();
        engine.edits.push(Edit::RemoveNode { node: radius });
    }
    engine.apply_edits().unwrap();
    // Capture both repairs, then let the first application advance the input
    // revision before changing the second endpoint's admitted shape.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline);
        let edits = direct_turn(&mut engine, id);
        let repaired_first = edits
            .pending
            .iter()
            .any(|r| matches!(r.edit, Edit::AddNodeTree { parent, .. } if parent == first));
        apply(&mut engine, edits);
        if repaired_first {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    for index in 0..17 {
        engine.add_node(Folder::new(format!("Extra {index}")).into(), Some(second));
    }
    engine.apply_edits().unwrap();
    let edits = direct_turn(&mut engine, id);
    assert!(!edits
        .pending
        .iter()
        .any(|r| matches!(r.edit, Edit::AddNodeTree { parent, .. } if parent == second)));
    apply(&mut engine, edits);
    stabilize(&mut engine);
    assert!(module(&mut engine, id)
        .service
        .warning_text()
        .unwrap()
        .contains("16 direct children"));
    assert_eq!(default_cell(&engine, id), original);
    assert!(find_child_by_key(&engine, second, "radius").is_none());
}

fn output_change(engine: &crate::app::AppEngine, cell: NodeId) -> golden_core::events::EventFrame {
    golden_core::events::EventFrame::from_shared(vec![Arc::new(golden_core::events::Event {
        time: engine.time,
        kind: golden_core::events::EventKind::ParamChanged {
            param: cell,
            old_value: ParamValue::Float(1.0),
            new_value: ParamValue::Float(0.5),
            source: None,
        },
    })])
}

#[test]
fn prepared_generated_echoes_do_not_request_or_build_an_inbox_snapshot() {
    let (mut engine, id, cell) = fixture(1, 40);
    let source = source(&engine, id);
    set_position_2d(&mut engine, source, 0.5, 0.0);
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    let edits = pending_value(&mut engine, id, cell);
    assert!(module(&mut engine, id).service.generated_output(cell));
    let frame = output_change(&engine, cell);
    assert!(!module(&mut engine, id).inbox_requires_tree_snapshot(&frame));
    apply(&mut engine, edits);
    let before = engine.tick_stats().snapshot_builds;
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    assert_eq!(engine.tick_stats().snapshot_builds, before);
    stabilize(&mut engine);
    assert_eq!(float(&engine, cell), 0.5);
}

#[test]
fn unknown_fields_and_changed_ownership_cannot_use_the_output_echo_catalog() {
    use golden_core::events::{Event, EventKind};
    let (mut engine, id, cell) = fixture(1, 40);
    let source = source(&engine, id);
    set_position_2d(&mut engine, source, 0.5, 0.0);
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    let edits = pending_value(&mut engine, id, cell);
    let connection = find_path(&engine, id, "connection/log_incoming").unwrap();
    let input = output_change(&engine, connection);
    assert!(module(&mut engine, id).inbox_requires_tree_snapshot(&input));
    let mut moved = output_change(&engine, cell);
    moved.push_shared(Arc::new(Event {
        time: engine.time,
        kind: EventKind::ChildMoved {
            child: cell,
            old_parent: engine.nodes.get(cell).unwrap().node_data().parent.unwrap(),
            new_parent: engine.root,
        },
    }));
    assert!(module(&mut engine, id).inbox_requires_tree_snapshot(&moved));
    apply(&mut engine, edits);
    engine.edits.push(Edit::MoveNode {
        node: cell,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    assert!(!module(&mut engine, id).service.generated_output(cell));
    stabilize(&mut engine);
    assert_eq!(default_cell(&engine, id), cell);
}

#[test]
fn an_adopted_inbox_view_keeps_apply_time_input_fences() {
    let (mut engine, id, cell) = fixture(1, 40);
    let source = source(&engine, id);
    set_position_2d(&mut engine, source, 0.5, 0.0);
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    let edits = pending_value(&mut engine, id, cell);
    apply(&mut engine, edits);
    let mut inbox = context(&engine);
    inbox.events = output_change(&engine, cell);
    module(&mut engine, id).on_inbox(&mut inbox);
    assert!(!module(&mut engine, id).worker_wake_requires_tree_snapshot());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).on_worker_wake(&mut ctx);
    assert!(!ctx.edits.pending.is_empty());
    set_position_2d(&mut engine, source, 0.25, 0.0);
    assert!(apply_failure(&mut engine, ctx.edits));
    stabilize(&mut engine);
    assert_eq!(default_cell(&engine, id), cell);
    assert_eq!(float(&engine, cell), 0.75);
}
