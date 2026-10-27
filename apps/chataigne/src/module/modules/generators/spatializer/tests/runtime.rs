use super::super::{publication, worker};
use super::*;

mod row_batches;
mod sparse;
use golden_core::{
    edit::{EditCompletionStatus, EditQueue},
    engine::EngineEditError,
    parameter::{Parameter, ParameterChangeCheck},
    process_ctx::ProcessCtx,
};
use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

fn module(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut SpatializerModule {
    match engine.nodes.get_mut(id).unwrap() {
        crate::app::AppNode::SpatializerModule(module) => module,
        _ => panic!("Spatializer expected"),
    }
}
fn context(engine: &crate::app::AppEngine) -> ProcessCtx {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    ctx
}
fn direct_turn(engine: &mut crate::app::AppEngine, id: NodeId) -> EditQueue {
    let mut ctx = context(engine);
    module(engine, id).on_worker_wake(&mut ctx);
    ctx.edits
}
fn apply(engine: &mut crate::app::AppEngine, mut edits: EditQueue) {
    engine.edits.pending.extend(edits.drain());
    engine.apply_edits().unwrap();
}
fn pump(engine: &mut crate::app::AppEngine) {
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.run_tick(Duration::from_millis(2)).unwrap();
    engine.apply_edits().unwrap();
}
fn until(engine: &mut crate::app::AppEngine, mut predicate: impl FnMut(&mut crate::app::AppEngine) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate(engine) {
        assert!(Instant::now() < deadline, "Spatializer condition timed out");
        pump(engine);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn default_cell(engine: &crate::app::AppEngine, id: NodeId) -> NodeId {
    let values = find_path(engine, id, "values").unwrap();
    let row = nth_child(engine, values, 0).unwrap();
    nth_child(engine, row, 0).unwrap()
}
fn float(engine: &crate::app::AppEngine, id: NodeId) -> f64 {
    engine
        .nodes
        .get(id)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
        .as_float()
        .unwrap()
}
fn source(engine: &crate::app::AppEngine, id: NodeId) -> NodeId {
    nth_child(engine, find_path(engine, id, "parameters/sources").unwrap(), 0).unwrap()
}
fn target_radius_fixture() -> (crate::app::AppEngine, NodeId, NodeId, NodeId) {
    let (mut engine, id) = create_spatializer_module();
    let mode = find_path(&engine, id, "parameters/mode").unwrap();
    set_param(&mut engine, mode, ParamValue::Enum("targetRadius".into()));
    stabilize(&mut engine);
    let source = source(&engine, id);
    let cell = default_cell(&engine, id);
    assert_eq!(float(&engine, cell), 1.0);
    (engine, id, source, cell)
}
fn pending_value(engine: &mut crate::app::AppEngine, id: NodeId, cell: NodeId) -> EditQueue {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "prepared value did not arrive");
        let edits = direct_turn(engine, id);
        if edits
            .pending
            .iter()
            .any(|request| matches!(request.edit, Edit::SetParam { node, .. } if node == cell))
        {
            return edits;
        }
        apply(engine, edits);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn count_and_byte_admission_preserves_supported_math_range() {
    assert!(worker::admit(512, 1024).is_ok());
    assert!(worker::admit(4096, 256).is_ok());
    assert!(worker::admit(256, 4096).is_ok());
    assert!(worker::admit(4097, 1).is_err());
    assert!(worker::admit(1, 4097).is_err());
    assert!(worker::admit(1025, 1024).is_err());
    assert!(worker::admit(usize::MAX, 2).is_err());
    assert!(worker::admit(0, 0).unwrap() > 0);
    assert_eq!(
        worker::admit(512, 1024).unwrap(),
        64 * 1024 + 1536 * 4096 + 512 * 1024 * 24
    );
}

#[test]
fn canonical_inputs_start_the_worker_without_endpoint_scan_wakes() {
    let (mut engine, id) = create_spatializer_module();
    let position = find_path(&engine, source(&engine, id), "position_2d").unwrap();
    let control = Arc::new(worker::TestControl::default());
    module(&mut engine, id).service.control = Some(control.clone());
    module(&mut engine, id).service.cancel();
    module(&mut engine, id).service.request();
    engine.run_tick(Duration::from_millis(2)).unwrap();
    assert!(engine.tick_stats().snapshot_builds > 0);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !control.entered.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }

    engine.run_tick(Duration::from_millis(2)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    assert_eq!(engine.tick_stats().snapshot_builds, 0);

    set_param(&mut engine, position, ParamValue::Vec2(0.5, 0.0));
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    assert!(module(&mut engine, id).worker_wake_requires_tree_snapshot());
    module(&mut engine, id).service.control = None;
    control.released.store(true, Ordering::Release);
    stabilize(&mut engine);
    assert!(module(&mut engine, id).service.idle());
    assert!(module(&mut engine, id).worker_wake_requires_tree_snapshot());
}

#[test]
fn output_delivery_and_queued_publication_cannot_keep_a_stale_read_view() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    module(&mut engine, id).service.cancel();
    module(&mut engine, id).service.request();
    engine.run_tick(Duration::from_millis(2)).unwrap();
    assert!(module(&mut engine, id).worker_wake_requires_tree_snapshot());

    set_param(&mut engine, cell, ParamValue::Float(0.25));
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    assert!(module(&mut engine, id).worker_wake_requires_tree_snapshot());

    set_position_2d(&mut engine, source, 0.5, 0.0);
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    let edits = pending_value(&mut engine, id, cell);
    assert!(module(&mut engine, id).worker_wake_requires_tree_snapshot());
    apply(&mut engine, edits);
    stabilize(&mut engine);
    assert!((float(&engine, cell) - 0.5).abs() < 1e-12);
}

#[test]
fn connection_and_input_callbacks_remain_available_without_echoing_generated_values() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    let connection = find_path(&engine, id, "connection/log_incoming").unwrap();
    let position = find_child_by_key(&engine, source, "position_2d").unwrap();
    for (param, expected) in [
        (connection, Some("moduleConnectionChanged")),
        (position, Some("moduleModuleParameterChanged")),
        (cell, None),
    ] {
        let mut ctx = context(&engine);
        module(&mut engine, id).on_param_change(&mut ctx, param, ParamValue::Bool(false));
        let callbacks: Vec<_> = ctx
            .edits
            .pending
            .iter()
            .filter_map(|request| match &request.edit {
                Edit::EmitCustomEvent { event } => event.payload["callback"].as_str(),
                _ => None,
            })
            .collect();
        assert_eq!(callbacks, expected.into_iter().collect::<Vec<_>>());
    }
}

#[test]
fn queued_destination_moves_are_fenced_and_generated_identity_is_restored() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    let parent = engine.nodes.get(cell).unwrap().node_data().parent.unwrap();
    let uuid = engine.nodes.get(cell).unwrap().node_data().meta.uuid;
    set_position_2d(&mut engine, source, 0.5, 0.0);
    let mut prepared = pending_value(&mut engine, id, cell);
    engine.edits.push(Edit::MoveNode {
        node: cell,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    engine.edits.pending.extend(prepared.drain());
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(float(&engine, cell), 1.0);
    stabilize(&mut engine);
    assert_eq!(engine.nodes.get(cell).unwrap().node_data().parent, Some(parent));
    assert_eq!(engine.nodes.get(cell).unwrap().node_data().meta.uuid, uuid);
    assert_eq!(float(&engine, cell), 0.5);
    engine.edits.push(Edit::MoveNode {
        node: parent,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    assert_eq!(default_cell(&engine, id), cell);
}

#[test]
fn endpoint_removal_keeps_survivors_and_unrelated_values_without_readding_defaults() {
    let (mut engine, id) = create_spatializer_module();
    let targets = find_path(&engine, id, "parameters/targets").unwrap();
    let sources = find_path(&engine, id, "parameters/sources").unwrap();
    let values = find_path(&engine, id, "values").unwrap();
    let cell = default_cell(&engine, id);
    let uuid = engine.nodes.get(cell).unwrap().node_data().meta.uuid;
    engine.add_user_item(SpatializerTargetItem::new().into(), Some(targets));
    engine.add_node(Folder::new("Unrelated").into(), Some(values));
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    let second = nth_child(&engine, targets, 1).unwrap();
    engine.edits.push(Edit::RemoveNode { node: second });
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    assert_eq!(default_cell(&engine, id), cell);
    assert_eq!(engine.nodes.get(cell).unwrap().node_data().meta.uuid, uuid);
    let source = nth_child(&engine, sources, 0).unwrap();
    engine.edits.push(Edit::RemoveNode { node: source });
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    assert_eq!(child_count(&engine, sources), 0);
    assert_eq!(child_count(&engine, values), 1);
    assert!(find_child_by_key(&engine, values, "Unrelated").is_some());
}

#[test]
fn process_capacity_includes_blocked_and_cancelled_preparations() {
    let (engine, id) = create_spatializer_module();
    let snapshot = engine.process_tree_snapshot();
    let parameters = find_path(&engine, id, "parameters").unwrap();
    let sources = find_path(&engine, id, "parameters/sources").unwrap();
    let targets = find_path(&engine, id, "parameters/targets").unwrap();
    assert!(worker::workers().wait_for_idle(Duration::from_secs(2)));
    let life = golden_io::RevisionSource::default();
    let control = Arc::new(worker::TestControl::default());
    let mut jobs = Vec::new();
    for _ in 0..4 {
        let owner = golden_io::RetirementPool::new(std::num::NonZeroUsize::MIN);
        let job = worker::start(
            &snapshot,
            worker::Roots {
                parameters,
                sources,
                targets,
                values: find_path(&engine, id, "values").unwrap(),
            },
            &owner,
            life.reader().guard_at(0),
            golden_io::WorkerNotifier::new(|| {}),
            Some(control.clone()),
        )
        .unwrap();
        jobs.push((owner, job));
    }
    let extra = golden_io::RetirementPool::new(std::num::NonZeroUsize::MIN);
    assert!(worker::start(
        &snapshot,
        worker::Roots {
            parameters,
            sources,
            targets,
            values: find_path(&engine, id, "values").unwrap(),
        },
        &extra,
        life.reader().guard_at(0),
        golden_io::WorkerNotifier::new(|| {}),
        None
    )
    .is_err());
    assert_eq!(worker::workers().metrics().active, 4);
    assert_eq!(extra.metrics().active, 0);
    life.advance();
    assert_eq!(
        worker::workers().metrics().active,
        4,
        "cancellation cannot release blocked slots"
    );
    control.released.store(true, Ordering::Release);
    assert!(worker::workers().wait_for_idle(Duration::from_secs(2)));
    for (owner, job) in jobs {
        assert_eq!(owner.metrics().peak, 1);
        assert!(!job.running());
        assert!(job.take().unwrap().is_err());
    }
}

#[test]
fn changing_inputs_during_detached_row_preparation_discards_the_stale_tree() {
    let (mut engine, id, _, old_cell) = target_radius_fixture();
    let sources = find_path(&engine, id, "parameters/sources").unwrap();
    let targets = find_path(&engine, id, "parameters/targets").unwrap();
    engine.add_user_item(SpatializerSourceItem::new().into(), Some(sources));
    for _ in 1..97 {
        engine.add_user_item(SpatializerTargetItem::new().into(), Some(targets));
    }
    engine.apply_edits().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while module(&mut engine, id).service.detached_nodes() == 0 {
        assert!(Instant::now() < deadline);
        let edits = direct_turn(&mut engine, id);
        apply(&mut engine, edits);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        module(&mut engine, id).service.detached_nodes(),
        publication::TREE_NODES_PER_TURN
    );
    let second = nth_child(&engine, sources, 1).unwrap();
    set_position_2d(&mut engine, second, 0.4, 0.0);
    stabilize(&mut engine);
    let values = find_path(&engine, id, "values").unwrap();
    assert_eq!(child_count(&engine, values), 2);
    let row = nth_child(&engine, values, 1).unwrap();
    assert_eq!(child_count(&engine, row), 97);
    for index in 0..97 {
        assert!((float(&engine, nth_child(&engine, row, index).unwrap()) - 0.6).abs() < 1e-12);
    }
    assert_eq!(default_cell(&engine, id), old_cell);
}

#[test]
fn settled_spatializer_has_no_scheduled_work_wakes_or_snapshots() {
    let (mut engine, _) = create_spatializer_module();
    for _ in 0..100 {
        pump(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.nodes_due, 0);
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_builds, 0);
    }
}

#[test]
fn queued_matrix_rejects_newer_inputs_before_their_callbacks_run() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    let uuid = engine.nodes.get(cell).unwrap().node_data().meta.uuid;
    set_position_2d(&mut engine, source, 0.5, 0.0);
    let mut prepared = pending_value(&mut engine, id, cell);
    let receipt = prepared.completion();
    let position = find_child_by_key(&engine, source, "position_2d").unwrap();
    engine.edits.push(Edit::SetParam {
        node: position,
        value: ParamValue::Vec2(0.25, 0.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.pending.extend(prepared.drain());
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(receipt.status(), EditCompletionStatus::Failed);
    assert_eq!(float(&engine, cell), 1.0);
    assert_eq!(engine.nodes.get(cell).unwrap().node_data().meta.uuid, uuid);
    stabilize(&mut engine);
    assert_eq!(float(&engine, cell), 0.75);
    assert_eq!(default_cell(&engine, id), cell);
}

#[test]
fn a_failed_publication_suffix_preserves_and_reconciles_its_applied_prefix() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    let targets = find_path(&engine, id, "parameters/targets").unwrap();
    engine.add_user_item(SpatializerTargetItem::new().into(), Some(targets));
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    let values = find_path(&engine, id, "values").unwrap();
    let row = nth_child(&engine, values, 0).unwrap();
    let second = nth_child(&engine, row, 1).unwrap();
    set_position_2d(&mut engine, source, 0.5, 0.0);
    let mut prepared = pending_value(&mut engine, id, cell);
    let mut bad = EditQueue::new();
    bad.push(Edit::RemoveNode { node: NodeId(u64::MAX) });
    let invalid = bad.pending.pop().unwrap();
    let first = prepared
        .pending
        .iter()
        .position(|request| matches!(request.edit, Edit::SetParam { node, .. } if node == cell))
        .unwrap();
    prepared.pending.insert(first + 1, invalid);
    assert!(apply_failure(&mut engine, prepared));
    assert_eq!(float(&engine, cell), 0.5);
    assert_eq!(float(&engine, second), 1.0);
    stabilize(&mut engine);
    assert_eq!(float(&engine, second), 0.5);
    assert_eq!(default_cell(&engine, id), cell);
}
fn apply_failure(engine: &mut crate::app::AppEngine, mut edits: EditQueue) -> bool {
    engine.edits.pending.extend(edits.drain());
    engine.apply_edits().is_err()
}

#[test]
fn blocked_preparation_cannot_multiply_workers_and_latest_configuration_recovers() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    let control = Arc::new(worker::TestControl::default());
    module(&mut engine, id).service.control = Some(control.clone());
    set_position_2d(&mut engine, source, 0.5, 0.0);
    until(&mut engine, |_| control.entered.load(Ordering::Acquire));
    let owner = module(&mut engine, id).service.worker_owner();
    for index in 0..100 {
        set_position_2d(&mut engine, source, index as f64 / 500.0, 0.0);
        pump(&mut engine);
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert_eq!(owner.metrics().active, 1);
        assert_eq!(owner.metrics().peak, 1);
    }
    assert_eq!(
        float(&engine, cell),
        1.0,
        "stale configuration never reaches the matrix"
    );
    module(&mut engine, id).service.control = None;
    control.released.store(true, Ordering::Release);
    stabilize(&mut engine);
    assert!((float(&engine, cell) - (1.0 - 99.0 / 500.0)).abs() < 1e-12);
    assert_eq!(default_cell(&engine, id), cell);
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn worker_failure_keeps_existing_values_and_recovers_without_recreating_cells() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    let control = Arc::new(worker::TestControl::default());
    control.released.store(true, Ordering::Release);
    control.panic.store(true, Ordering::Release);
    module(&mut engine, id).service.control = Some(control);
    set_position_2d(&mut engine, source, 0.5, 0.0);
    until(&mut engine, |engine| {
        module(engine, id)
            .service
            .warning_text()
            .is_some_and(|text| text.contains("panicked"))
    });
    assert_eq!(float(&engine, cell), 1.0);
    module(&mut engine, id).service.control = None;
    stabilize(&mut engine);
    assert_eq!(float(&engine, cell), 0.5);
    assert_eq!(default_cell(&engine, id), cell);
}

#[test]
fn removal_during_blocked_preparation_retires_without_joining_or_publishing() {
    let (mut engine, id, source, _) = target_radius_fixture();
    let control = Arc::new(worker::TestControl::default());
    module(&mut engine, id).service.control = Some(control.clone());
    set_position_2d(&mut engine, source, 0.5, 0.0);
    until(&mut engine, |_| control.entered.load(Ordering::Acquire));
    let owner = module(&mut engine, id).service.worker_owner();
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    assert!(engine.nodes.get(id).is_none());
    assert_eq!(owner.metrics().active, 1, "teardown keeps the blocked lifetime slot");
    control.released.store(true, Ordering::Release);
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    for _ in 0..10 {
        pump(&mut engine);
    }
    assert_eq!(engine.nodes.len(), 1);
}

#[test]
fn oversized_labels_reject_without_changing_values_then_recover_after_authored_repair() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    rename_node(&mut engine, source, &"x".repeat(513));
    stabilize(&mut engine);
    assert!(module(&mut engine, id)
        .service
        .warning_text()
        .unwrap()
        .contains("512 bytes"));
    assert_eq!(float(&engine, cell), 1.0);
    assert_eq!(default_cell(&engine, id), cell);
    rename_node(&mut engine, source, "Recovered");
    stabilize(&mut engine);
    assert!(module(&mut engine, id).service.warning_text().is_none());
    assert_eq!(default_cell(&engine, id), cell);
}

#[test]
fn preparation_capacity_pressure_retries_after_the_held_reservation_is_released() {
    let (mut engine, id, source, cell) = target_radius_fixture();
    let pool = worker::pool().unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let held = pool.try_reserve(128 * 1024 * 1024).unwrap();
    set_position_2d(&mut engine, source, 0.5, 0.0);
    until(&mut engine, |engine| {
        module(engine, id)
            .service
            .warning_text()
            .is_some_and(|text| text.contains("lifetime capacity"))
    });
    assert_eq!(float(&engine, cell), 1.0);
    assert_eq!(module(&mut engine, id).service.worker_owner().metrics().active, 0);
    drop(held);
    stabilize(&mut engine);
    assert_eq!(float(&engine, cell), 0.5);
}

#[test]
fn mode_dimensions_enablement_and_labels_keep_cell_identity() {
    let (mut engine, id, _, cell) = target_radius_fixture();
    let uuid = engine.nodes.get(cell).unwrap().node_data().meta.uuid;
    let mode = find_path(&engine, id, "parameters/mode").unwrap();
    for token in ["sourceRadius", "overlap", "voronoi", "targetRadius"] {
        set_param(&mut engine, mode, ParamValue::Enum(token.into()));
        stabilize(&mut engine);
        assert_eq!(default_cell(&engine, id), cell);
        assert_eq!(engine.nodes.get(cell).unwrap().node_data().meta.uuid, uuid);
    }
    let dimensions = find_path(&engine, id, "parameters/dimensions").unwrap();
    set_param(&mut engine, dimensions, ParamValue::Enum("3d".into()));
    stabilize(&mut engine);
    assert_eq!(default_cell(&engine, id), cell);
    let target = nth_child(&engine, find_path(&engine, id, "parameters/targets").unwrap(), 0).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: target,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    assert_eq!(float(&engine, cell), 0.0);
    engine.edits.push(Edit::PatchMeta {
        node: target,
        patch: NodeMetaPatch {
            enabled: Some(true),
            label: Some("Renamed".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    assert_eq!(float(&engine, cell), 1.0);
    assert_eq!(engine.nodes.get(cell).unwrap().node_data().meta.uuid, uuid);
    assert_eq!(engine.nodes.get(cell).unwrap().node_data().meta.label, "Renamed");
}

#[test]
fn wide_rows_prepare_incrementally_insert_as_subtrees_and_keep_the_engine_serviceable() {
    let (mut engine, id) = create_spatializer_module();
    let sources = find_path(&engine, id, "parameters/sources").unwrap();
    let targets = find_path(&engine, id, "parameters/targets").unwrap();
    for _ in 0..2 {
        engine.add_user_item(SpatializerSourceItem::new().into(), Some(sources));
    }
    for _ in 1..97 {
        engine.add_user_item(SpatializerTargetItem::new().into(), Some(targets));
    }
    engine.add_node(
        Parameter::new("Witness", ParamValue::Int(0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let witness = find_child_by_key(&engine, engine.root, "Witness").unwrap();
    let mut inserted_wide_rows = 0;
    let mut turns = 0;
    let mut samples = Vec::new();
    loop {
        let mut ctx = context(&engine);
        let started = Instant::now();
        module(&mut engine, id).on_worker_wake(&mut ctx);
        samples.push(started.elapsed().as_micros());
        let edits = ctx.edits;
        let value_writes = edits
            .pending
            .iter()
            .filter(|request| {
                matches!(
                    request.edit,
                    Edit::SetParam {
                        value: ParamValue::Float(_),
                        ..
                    }
                )
            })
            .count();
        assert!(value_writes <= publication::PAIRS_PER_TURN);
        for request in &edits.pending {
            if let Edit::AddNodeTree { tree, .. } = &request.edit {
                if tree.children.len() == 97 {
                    inserted_wide_rows += 1;
                }
                assert!(tree.children.len() <= 97);
            }
        }
        set_param(&mut engine, witness, ParamValue::Int(turns));
        apply(&mut engine, edits);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        turns += 1;
        if module(&mut engine, id).service.idle() {
            break;
        }
        assert!(turns < 2000);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(inserted_wide_rows, 2);
    samples.sort_unstable();
    eprintln!(
        "spatializer callback: turns={turns}, pairs_per_turn={}, tree_nodes_per_turn={}, p95_us={}, max_us={} (snapshot construction and edit application excluded)",
        publication::PAIRS_PER_TURN,
        publication::TREE_NODES_PER_TURN,
        samples[(samples.len() - 1) * 95 / 100],
        samples.last().unwrap()
    );
    assert!(turns > 100, "endpoint and tree preparation must span service turns");
    let values = find_path(&engine, id, "values").unwrap();
    assert_eq!(child_count(&engine, values), 3);
    for index in 0..3 {
        assert_eq!(child_count(&engine, nth_child(&engine, values, index).unwrap()), 97);
    }
    assert_eq!(
        engine
            .nodes
            .get(witness)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Int(turns - 1)
    );
}
