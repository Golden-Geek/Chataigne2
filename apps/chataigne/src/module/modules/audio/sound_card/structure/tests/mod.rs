use super::super::routes::tests::{apply, context, fixture, inject_failure_after, module_mut};
use super::*;
use golden_core::parameter::ParameterEventBehaviour;

fn set(engine: &mut crate::app::AppEngine, module: NodeId, path: &str, value: ParamValue) {
    let node = find_path(&engine.process_tree_snapshot(), module, path).unwrap();
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}

fn settle(work: &mut DerivedWork, engine: &mut crate::app::AppEngine, module: NodeId) -> usize {
    for turn in 1..2048 {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = context();
        let busy = work.service(&mut ctx, &snapshot, module).unwrap();
        assert!(
            ctx.edits
                .pending
                .iter()
                .filter(|e| !matches!(e.edit, Edit::CheckPreconditions { .. }))
                .count()
                <= service::WRITE_TURN
        );
        apply(engine, &mut ctx);
        if !busy {
            assert!(!work.busy());
            return turn;
        }
    }
    panic!("bounded structure must converge");
}

#[test]
fn idle_structure_owns_no_snapshot_or_receipt() {
    let work = DerivedWork::default();
    assert!(!work.busy());
    assert!(!work.needs_snapshot());
    assert!(!work.waiting());
}

#[test]
fn full_channel_growth_is_backpressured_and_recovers_a_committed_prefix() {
    let (mut engine, module) = fixture();
    set(
        &mut engine,
        module,
        "connection/output_routing/channel_count",
        ParamValue::Int(64),
    );
    let mut work = DerivedWork::default();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = context();
    assert!(work.service(&mut ctx, &snapshot, module).unwrap());
    assert_eq!(
        ctx.edits
            .pending
            .iter()
            .filter(|e| matches!(e.edit, Edit::AddNodeTree { .. }))
            .count(),
        service::WRITE_TURN
    );
    assert!(work.waiting());
    assert!(!work.needs_snapshot());
    let mut duplicate = context();
    assert!(work.service(&mut duplicate, &snapshot, module).unwrap());
    assert!(duplicate.edits.pending.is_empty());
    inject_failure_after(&mut engine, &mut ctx, 6);
    let turns = settle(&mut work, &mut engine, module);
    assert!(turns > 8, "64 gains and levels cannot be emitted in one service turn");
    let snapshot = engine.process_tree_snapshot();
    for path in ["parameters/output/channels", "values/levels/output/channels"] {
        assert_eq!(
            snapshot
                .child_ids_slice(find_path(&snapshot, module, path).unwrap())
                .len(),
            64
        );
    }
}

#[test]
fn legacy_gain_value_survives_replacement_partial_cleanup_and_retry() {
    let (mut engine, module) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let parent = find_path(&snapshot, module, "parameters/output/channels").unwrap();
    let id = snapshot.child_ids_slice(parent)[0];
    let uuid = snapshot.node(id).unwrap().uuid;
    let mut folder = Folder::new("Legacy gain");
    folder.node_data_mut().meta.uuid = uuid;
    engine.edits.push(Edit::ReplaceNode {
        node: id,
        new_node: Box::new(folder),
    });
    engine.apply_edits().unwrap();
    let mut gain = Parameter::new("Volume", ParamValue::Float(-17.0), ParameterChangeCheck::ValueChange);
    gain.node_data_mut().meta.decl_id = DeclId("volume_db".into());
    engine.edits.push(Edit::AddNodeTree {
        parent: id,
        tree: NodeTree::new(gain),
        prev_sibling: None,
    });
    for i in 0..40 {
        engine.edits.push(Edit::AddNodeTree {
            parent: id,
            tree: NodeTree::new(Folder::new(format!("Legacy {i}"))),
            prev_sibling: None,
        });
    }
    engine.apply_edits().unwrap();
    let mut work = DerivedWork::default();
    let mut replaced = false;
    for _ in 0..16 {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = context();
        work.service(&mut ctx, &snapshot, module).unwrap();
        if ctx
            .edits
            .pending
            .iter()
            .any(|e| matches!(e.edit, Edit::ReplaceNode { node, .. } if node == id))
        {
            apply(&mut engine, &mut ctx);
            replaced = true;
            break;
        }
        apply(&mut engine, &mut ctx);
    }
    assert!(replaced);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.node(id).unwrap().param_value, Some(ParamValue::Float(-17.0)));
    let mut ctx = context();
    work.service(&mut ctx, &snapshot, module).unwrap();
    assert_eq!(
        ctx.edits
            .pending
            .iter()
            .filter(|e| matches!(e.edit, Edit::RemoveNode { .. }))
            .count(),
        service::WRITE_TURN
    );
    inject_failure_after(&mut engine, &mut ctx, 4);
    settle(&mut work, &mut engine, module);
    let snapshot = engine.process_tree_snapshot();
    let state = snapshot.node(id).unwrap();
    assert_eq!(state.uuid, uuid);
    assert_eq!(state.param_value, Some(ParamValue::Float(-17.0)));
    assert_eq!(state.label, "Legacy gain Gain");
    assert!(snapshot.child_ids_slice(id).is_empty());
}

#[test]
fn changing_the_channel_count_after_admission_rejects_old_additions() {
    let (mut engine, module) = fixture();
    set(
        &mut engine,
        module,
        "connection/output_routing/channel_count",
        ParamValue::Int(16),
    );
    let mut work = DerivedWork::default();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = context();
    work.service(&mut ctx, &snapshot, module).unwrap();
    set(
        &mut engine,
        module,
        "connection/output_routing/channel_count",
        ParamValue::Int(3),
    );
    engine.edits.pending.extend(ctx.edits.drain());
    assert!(engine.apply_edits().is_err());
    settle(&mut work, &mut engine, module);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(
        snapshot
            .child_ids_slice(find_path(&snapshot, module, "parameters/output/channels").unwrap())
            .len(),
        3
    );
}

#[test]
fn lifetime_invalidation_rejects_queued_structure_without_retaining_the_pass() {
    let (mut engine, module) = fixture();
    set(
        &mut engine,
        module,
        "connection/output_routing/channel_count",
        ParamValue::Int(16),
    );
    let mut work = DerivedWork::default();
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = context();
    work.service(&mut ctx, &snapshot, module).unwrap();
    work.invalidate();
    assert!(!work.busy());
    engine.edits.pending.extend(ctx.edits.drain());
    assert!(engine.apply_edits().is_err());
}

#[test]
fn excessive_name_retries_at_a_coarse_deadline_and_does_not_ask_for_snapshots() {
    let (mut engine, module) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let parent = find_path(&snapshot, module, "parameters/output/channels").unwrap();
    let node = snapshot.child_ids_slice(parent)[0];
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            label: Some("x".repeat(service::NAME_BYTES + 1)),
            ..NodeMetaPatch::default()
        },
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let mut work = DerivedWork::default();
    let mut ctx = context();
    assert!(work.service(&mut ctx, &snapshot, module).is_err());
    assert!(ctx.edits.pending.is_empty());
    assert!(work.busy() && work.waiting());
    assert!(!work.needs_snapshot());
    let node = module_mut(&mut engine, module);
    node.derived = work;
    node.configuration_dirty = true;
    assert!(!node.worker_wake_requires_tree_snapshot());
}

#[test]
fn large_stale_folder_and_concurrent_addition_converge_with_the_same_turn_bound() {
    let (mut engine, module) = fixture();
    set(
        &mut engine,
        module,
        "connection/input_device",
        ParamValue::Enum(NO_AUDIO_DEVICE.into()),
    );
    settle(&mut DerivedWork::default(), &mut engine, module);
    let parent = find_path(&engine.process_tree_snapshot(), module, "parameters/input/channels").unwrap();
    for i in 0..4096 {
        engine.edits.push(Edit::AddNodeTree {
            parent,
            tree: NodeTree::new(Folder::new(format!("Stale {i}"))),
            prev_sibling: None,
        });
    }
    for _ in 0..512 {
        engine.apply_edits().unwrap();
        if engine.process_tree_snapshot().child_ids_slice(parent).len() == 4096 {
            break;
        }
    }
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(parent).len(), 4096);
    let snapshot = engine.process_tree_snapshot();
    let mut work = DerivedWork::default();
    let mut ctx = context();
    let started = std::time::Instant::now();
    work.service(&mut ctx, &snapshot, module).unwrap();
    let elapsed = started.elapsed();
    let removals = ctx
        .edits
        .pending
        .iter()
        .filter(|e| matches!(e.edit, Edit::RemoveNode { .. }))
        .count();
    assert_eq!(removals, service::WRITE_TURN);
    eprintln!("4096 stale gains: one preparation turn {elapsed:?}, {removals} removals");
    apply(&mut engine, &mut ctx);
    engine.edits.push(Edit::AddNodeTree {
        parent,
        tree: NodeTree::new(Folder::new("Late stale gain")),
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    work.request();
    assert!(settle(&mut work, &mut engine, module) >= 512);
    assert!(engine.process_tree_snapshot().child_ids_slice(parent).is_empty());
}

#[test]
fn correcting_an_obsolete_folder_before_application_rejects_its_removal() {
    let (mut engine, module) = fixture();
    let parent = find_path(&engine.process_tree_snapshot(), module, "values").unwrap();
    let mut obsolete = Folder::new("Old diagnostics");
    obsolete.node_data_mut().meta.decl_id = DeclId("diagnostics".into());
    let uuid = obsolete.node_data().meta.uuid;
    engine.edits.push(Edit::AddNodeTree {
        parent,
        tree: NodeTree::new(obsolete),
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let mut work = DerivedWork::default();
    let mut found = false;
    for _ in 0..16 {
        let snapshot = engine.process_tree_snapshot();
        let id = snapshot.node_id_by_uuid(uuid).unwrap();
        let mut ctx = context();
        work.service(&mut ctx, &snapshot, module).unwrap();
        if ctx
            .edits
            .pending
            .iter()
            .any(|e| matches!(e.edit, Edit::RemoveNode { node } if node == id))
        {
            // Moving it out of the obsolete scope must defeat the prepared read.
            engine.edits.push(Edit::MoveNode {
                node: id,
                new_parent: engine.root,
                new_prev_sibling: None,
            });
            engine.edits.pending.extend(ctx.edits.drain());
            assert!(engine.apply_edits().is_err());
            found = true;
            break;
        }
        apply(&mut engine, &mut ctx);
    }
    assert!(found);
    settle(&mut work, &mut engine, module);
    let snapshot = engine.process_tree_snapshot();
    let id = snapshot.node_id_by_uuid(uuid).unwrap();
    assert_eq!(snapshot.node(id).unwrap().parent, Some(engine.root));
}

#[test]
fn spectral_bool_and_duplicate_enabled_intent_survive_failed_cleanup() {
    let (mut engine, module) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let parent = find_path(&snapshot, module, "parameters/processing").unwrap();
    let spectral = find_path(&snapshot, module, "parameters/processing/spectral_analysis").unwrap();
    let uuid = snapshot.node(spectral).unwrap().uuid;
    let mut old = Parameter::new(
        "Spectral Analysis",
        ParamValue::Bool(true),
        ParameterChangeCheck::ValueChange,
    );
    old.node_data_mut().meta.uuid = uuid;
    old.node_data_mut().meta.decl_id = DeclId("parameters/processing/spectral_analysis".into());
    engine.edits.push(Edit::ReplaceNode {
        node: spectral,
        new_node: Box::new(old),
    });
    for _ in 0..35 {
        let mut duplicate = Parameter::new(
            "Spectral Analysis",
            ParamValue::Bool(false),
            ParameterChangeCheck::ValueChange,
        );
        duplicate.node_data_mut().meta.decl_id = DeclId("spectral_analysis".into());
        engine.edits.push(Edit::AddNodeTree {
            parent,
            tree: NodeTree::new(duplicate),
            prev_sibling: None,
        });
    }
    engine.apply_edits().unwrap();
    let mut work = DerivedWork::default();
    let mut replaced = false;
    for _ in 0..32 {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = context();
        work.service(&mut ctx, &snapshot, module).unwrap();
        replaced |= ctx
            .edits
            .pending
            .iter()
            .any(|e| matches!(e.edit, Edit::ReplaceNode { node, .. } if node == spectral));
        apply(&mut engine, &mut ctx);
        if replaced {
            break;
        }
    }
    assert!(replaced);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.node(spectral).unwrap().node_type, "folder");
    assert!(snapshot.node(spectral).unwrap().enabled);
    let mut ctx = context();
    work.service(&mut ctx, &snapshot, module).unwrap();
    if !ctx.edits.pending.is_empty() {
        inject_failure_after(&mut engine, &mut ctx, 3);
    }
    settle(&mut work, &mut engine, module);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.node(spectral).unwrap().uuid, uuid);
    assert!(snapshot.node(spectral).unwrap().enabled);
    assert!(find_path(&snapshot, module, "values/spectral_analysis").is_some());
    assert_eq!(
        snapshot
            .child_ids_slice(parent)
            .iter()
            .filter(|id| snapshot.node(**id).unwrap().label == "Spectral Analysis")
            .count(),
        1
    );
}

#[test]
fn value_only_updates_cannot_keep_a_large_authored_scan_restarting() {
    let (mut engine, module) = fixture();
    let parent = find_path(&engine.process_tree_snapshot(), module, "parameters").unwrap();
    for i in 0..512 {
        engine.edits.push(Edit::AddNodeTree {
            parent,
            tree: NodeTree::new(Folder::new(format!("Custom {i}"))),
            prev_sibling: None,
        });
    }
    for _ in 0..32 {
        engine.apply_edits().unwrap();
    }
    let mut work = DerivedWork::default();
    let mut completed = false;
    for turn in 0..128 {
        set(
            &mut engine,
            module,
            "values/playback_status/active_voices",
            ParamValue::Int(turn),
        );
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = context();
        let busy = work.service(&mut ctx, &snapshot, module).unwrap();
        apply(&mut engine, &mut ctx);
        if !busy {
            completed = true;
            break;
        }
    }
    assert!(
        completed,
        "meter/value changes must not invalidate every completed scan"
    );
}

#[test]
fn maximum_channel_structure_finishes_through_passive_null_worker_wakes() {
    let (mut engine, module) = super::super::tests::create_sound_card();
    set(
        &mut engine,
        module,
        "connection/output_routing/channel_count",
        ParamValue::Int(256),
    );
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        engine.run_tick(std::time::Duration::from_millis(20)).unwrap();
        let snapshot = engine.process_tree_snapshot();
        let complete = ["parameters/output/channels", "values/levels/output/channels"]
            .iter()
            .all(|path| find_path(&snapshot, module, path).is_some_and(|id| snapshot.child_ids_slice(id).len() == 256));
        if complete && !module_mut(&mut engine, module).derived.busy() {
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "passive structure must not be stranded"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!module_mut(&mut engine, module).needs_update());
}
