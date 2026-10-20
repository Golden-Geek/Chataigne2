use super::delivery::{WarningDelivery, SERVICE_ITEMS};
use super::*;
use golden_core::{
    edit::Edit,
    engine::Engine,
    node::{Folder, Node, NodeWarning},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use golden_io::{PayloadPool, RetainedPayload};
use std::time::Duration;

fn warning(node: NodeId, message: &str) -> RuntimeWarning {
    RuntimeWarning {
        node,
        id: "sound-card-unresolved-channel".into(),
        message: message.into(),
        detail: None,
    }
}

fn plan(engine: &Engine<Folder>, desired: &[RuntimeWarning], pool: &PayloadPool) -> RetainedPayload<WarningPlan> {
    let plan = WarningPlan::prepare(&engine.process_tree_snapshot(), engine.root, desired).unwrap();
    let bytes = plan.bytes;
    pool.try_retain(plan, bytes).unwrap()
}

fn context(engine: &Engine<Folder>) -> ProcessCtx {
    ProcessCtx::new(ExecutionPhase::EngineTick, engine.time)
}

fn settle(engine: &mut Engine<Folder>, delivery: &mut WarningDelivery) {
    for _ in 0..200 {
        let mut ctx = context(engine);
        delivery.service(&mut ctx, false);
        assert!(ctx.edits.pending.len() <= SERVICE_ITEMS);
        engine.absorb_edits(&mut ctx).unwrap();
        let _ = engine.apply_edits();
        if !delivery.has_pending() {
            return;
        }
    }
    panic!("warning service did not settle");
}

fn pool() -> PayloadPool {
    PayloadPool::new("warning-test", 16, 16 * 1024 * 1024).unwrap()
}

#[test]
fn reconciliation_uses_applied_state_updates_text_and_preserves_other_warning_owners() {
    let mut engine = Engine::new(Folder::new("root"));
    let pool = pool();
    engine.edits.push(Edit::SetNodeWarning {
        node: engine.root,
        warning: NodeWarning::new("foreign").with_id("another-owner"),
    });
    engine.apply_edits().unwrap();
    let desired = warning(engine.root, "first");
    let mut delivery = WarningDelivery::default();
    delivery.start(plan(&engine, std::slice::from_ref(&desired), &pool));
    settle(&mut engine, &mut delivery);
    assert!(plan(&engine, &[desired], &pool).patches.is_empty());
    let changed = warning(engine.root, "changed");
    delivery.start(plan(&engine, &[changed], &pool));
    settle(&mut engine, &mut delivery);
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .unwrap()
            .node_data()
            .meta
            .presentation
            .warnings[1]
            .message,
        "changed"
    );
    delivery.start(plan(&engine, &[], &pool));
    settle(&mut engine, &mut delivery);
    let warnings = &engine
        .nodes
        .get(engine.root)
        .unwrap()
        .node_data()
        .meta
        .presentation
        .warnings;
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].id, "another-owner");
}

#[test]
fn large_warning_plan_has_bounded_actor_turns_and_no_duplicate_pending_edits() {
    let mut engine = Engine::new(Folder::new("root"));
    for index in 0..200 {
        engine.add_node(Folder::new(format!("child{index}")), None);
    }
    engine.apply_edits().unwrap();
    let children = engine.process_tree_snapshot().child_ids(engine.root);
    let desired = children
        .iter()
        .map(|node| warning(*node, "missing"))
        .collect::<Vec<_>>();
    let pool = pool();
    let mut delivery = WarningDelivery::default();
    delivery.start(plan(&engine, &desired, &pool));
    let mut ctx = context(&engine);
    delivery.service(&mut ctx, false);
    assert_eq!(ctx.edits.pending.len(), SERVICE_ITEMS);
    delivery.service(&mut ctx, false);
    assert_eq!(ctx.edits.pending.len(), SERVICE_ITEMS);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    settle(&mut engine, &mut delivery);
    assert!(plan(&engine, &desired, &pool).patches.is_empty());
    assert!(!delivery.take_failure());
}

#[test]
fn failed_prefix_retries_unapplied_warnings_in_isolation() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.add_node(Folder::new("child"), None);
    engine.apply_edits().unwrap();
    let child = engine.process_tree_snapshot().child_ids(engine.root)[0];
    let desired = [warning(engine.root, "root"), warning(child, "child")];
    let pool = pool();
    let mut delivery = WarningDelivery::default();
    delivery.start(plan(&engine, &desired, &pool));
    let mut ctx = context(&engine);
    delivery.service(&mut ctx, false);
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    settle(&mut engine, &mut delivery);
    assert!(plan(&engine, &desired, &pool).patches.is_empty());
    assert!(!delivery.take_failure());
}

#[test]
fn removed_target_cannot_poison_valid_suffix_or_spin_continuations() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.add_node(Folder::new("removed"), None);
    engine.add_node(Folder::new("kept"), None);
    engine.apply_edits().unwrap();
    let children = engine.process_tree_snapshot().child_ids(engine.root);
    let desired = [warning(children[0], "removed"), warning(children[1], "kept")];
    let pool = pool();
    let prepared = plan(&engine, &desired, &pool);
    engine.edits.push(Edit::RemoveNode { node: children[0] });
    engine.apply_edits().unwrap();
    let mut delivery = WarningDelivery::default();
    delivery.start(prepared);
    settle(&mut engine, &mut delivery);
    assert!(delivery.take_failure());
    assert!(!delivery.take_failure());
    assert!(plan(&engine, &desired[1..], &pool).patches.is_empty());
    let mut ctx = context(&engine);
    delivery.service(&mut ctx, false);
    assert!(ctx.edits.pending.is_empty());
}

#[test]
fn quiescence_settles_admitted_prefix_before_replacement_diff_and_cancels_unqueued_suffix() {
    let mut engine = Engine::new(Folder::new("root"));
    for index in 0..40 {
        engine.add_node(Folder::new(format!("child{index}")), None);
    }
    engine.apply_edits().unwrap();
    let desired = engine
        .process_tree_snapshot()
        .child_ids(engine.root)
        .into_iter()
        .map(|node| warning(node, "old"))
        .collect::<Vec<_>>();
    let pool = pool();
    let mut delivery = WarningDelivery::default();
    delivery.start(plan(&engine, &desired, &pool));
    let mut ctx = context(&engine);
    delivery.service(&mut ctx, false);
    assert!(!delivery.quiesce());
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = context(&engine);
    delivery.service(&mut ctx, true);
    assert!(ctx.edits.pending.is_empty());
    assert!(delivery.quiesce());
    let replacement = plan(&engine, &[], &pool);
    assert_eq!(replacement.patches.len(), SERVICE_ITEMS);
    delivery.start(replacement);
    settle(&mut engine, &mut delivery);
    assert!(plan(&engine, &[], &pool).patches.is_empty());
}

#[test]
fn warning_storage_remains_charged_through_admitted_receipts() {
    let mut engine = Engine::new(Folder::new("root"));
    let pool = PayloadPool::new("warning-quota-test", 1, 64 * 1024).unwrap();
    let mut delivery = WarningDelivery::default();
    delivery.start(plan(&engine, &[warning(engine.root, "retained")], &pool));
    let mut ctx = context(&engine);
    delivery.service(&mut ctx, false);
    assert!(!delivery.quiesce());
    assert!(pool.try_reserve(1).is_err());
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = context(&engine);
    delivery.service(&mut ctx, true);
    assert!(delivery.quiesce());
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(pool.try_reserve(1).is_ok());
}
