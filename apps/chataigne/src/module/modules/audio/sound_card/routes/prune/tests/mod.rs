use super::super::tests::{add_routes, apply, context, fixture, inject_failure_after, module_mut};
use super::*;

fn guard(source: &golden_io::RevisionSource) -> golden_io::RevisionGuard {
    source.reader().guard_at(source.reader().current())
}

#[test]
fn idle_pruner_owns_no_snapshot_or_pending_edits() {
    let work = Pruning::default();
    assert!(!work.busy());
    assert!(!work.pending());
}

#[test]
fn pruning_has_fixed_scan_and_delete_turns_and_rechecks_a_failed_prefix() {
    let (mut engine, module) = fixture();
    let parent = add_routes(&mut engine, module, "output", 40);
    let lifetime = golden_io::RevisionSource::default();
    let mut work = Pruning::default();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = context();
    assert!(work
        .service(&mut ctx, &snapshot, parent, &[], vec![], guard(&lifetime))
        .unwrap());
    assert_eq!(ctx.edits.pending.len(), DELETE_TURN + 1);
    let mut duplicate = context();
    assert!(work
        .service(&mut duplicate, &snapshot, parent, &[], vec![], guard(&lifetime))
        .unwrap());
    assert!(duplicate.edits.pending.is_empty());
    inject_failure_after(&mut engine, &mut ctx, 4);
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(parent).len(), 37);
    let mut turns = 0;
    while work.busy() {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = context();
        work.service(&mut ctx, &snapshot, parent, &[], vec![], guard(&lifetime))
            .unwrap();
        assert!(ctx.edits.pending.len() <= DELETE_TURN + 1);
        apply(&mut engine, &mut ctx);
        turns += 1;
        assert!(turns < 20);
    }
    assert!(engine.process_tree_snapshot().child_ids_slice(parent).is_empty());
}

#[test]
fn final_resweep_keeps_routes_rewired_to_a_live_channel() {
    let (mut engine, module) = fixture();
    let parent = add_routes(&mut engine, module, "output", 70);
    let snapshot = engine.process_tree_snapshot();
    let reference = super::super::default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap();
    let lifetime = golden_io::RevisionSource::default();
    let mut work = Pruning::default();
    let mut ctx = context();
    assert!(work
        .service(
            &mut ctx,
            &snapshot,
            parent,
            std::slice::from_ref(&reference),
            vec![],
            guard(&lifetime)
        )
        .unwrap());
    assert!(ctx.edits.pending.is_empty());
    assert_eq!(work.0.as_ref().unwrap().cursor, SCAN_TURN);
    let route = snapshot.child_ids_slice(parent)[0];
    let channel = find_path(&snapshot, route, "channel").unwrap();
    engine.edits.push(Edit::SetParam {
        node: channel,
        value: ParamValue::Reference(NodeReference::default()),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    for _ in 0..20 {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = context();
        let busy = work
            .service(
                &mut ctx,
                &snapshot,
                parent,
                std::slice::from_ref(&reference),
                vec![],
                guard(&lifetime),
            )
            .unwrap();
        apply(&mut engine, &mut ctx);
        if !busy {
            break;
        }
    }
    assert!(!work.busy());
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(parent).len(), 69);
}

#[test]
fn correcting_a_route_between_admission_and_application_rejects_its_deletion() {
    let (mut engine, module) = fixture();
    let parent = add_routes(&mut engine, module, "output", 1);
    let snapshot = engine.process_tree_snapshot();
    let reference = super::super::default_channel_reference(&snapshot, module, "output", 1)
        .unwrap()
        .unwrap();
    let route = snapshot.child_ids_slice(parent)[0];
    let channel = find_path(&snapshot, route, "channel").unwrap();
    engine.edits.push(Edit::SetParam {
        node: channel,
        value: ParamValue::Reference(NodeReference::default()),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let lifetime = golden_io::RevisionSource::default();
    let mut work = Pruning::default();
    let mut ctx = context();
    work.service(
        &mut ctx,
        &snapshot,
        parent,
        std::slice::from_ref(&reference),
        vec![],
        guard(&lifetime),
    )
    .unwrap();
    engine.edits.push(Edit::SetParam {
        node: channel,
        value: ParamValue::Reference(reference),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.edits.pending.extend(ctx.edits.drain());
    assert!(engine.apply_edits().is_err());
    assert!(engine.nodes.get(route).is_some());
}

#[test]
fn clearing_the_driver_finishes_resets_without_a_native_runtime() {
    use std::time::{Duration, Instant};
    let (mut engine, module) = fixture();
    let parent = add_routes(&mut engine, module, "output", 40);
    let snapshot = engine.process_tree_snapshot();
    let driver = find_path(&snapshot, module, "connection/audio_driver").unwrap();
    engine.edits.push(Edit::SetParam {
        node: driver,
        value: ParamValue::Enum("none".into()),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        let snapshot = engine.process_tree_snapshot();
        if snapshot.child_ids_slice(parent).is_empty() && !module_mut(&mut engine, module).output_route_reset_pending {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(module_mut(&mut engine, module).runtime.is_none());
}

#[test]
fn resource_retry_waits_for_its_coarse_deadline_without_requesting_snapshots() {
    let mut work = Pruning(
        None,
        Some(std::time::Instant::now() + std::time::Duration::from_secs(60)),
    );
    assert!(work.busy());
    assert!(!work.needs_snapshot());
    let (mut engine, module) = fixture();
    let node = module_mut(&mut engine, module);
    node.configuration_dirty = true;
    node.routes.prune_output = work;
    assert!(!node.worker_wake_requires_tree_snapshot());
    work = std::mem::take(&mut node.routes.prune_output);
    work.1 = Some(std::time::Instant::now());
    assert!(work.needs_snapshot());
}
