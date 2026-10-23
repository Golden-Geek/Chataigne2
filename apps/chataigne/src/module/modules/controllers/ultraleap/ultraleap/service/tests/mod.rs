use super::*;
use crate::app::{AppEngine, AppNode};
use golden_core::{edit::Edit, node::Folder, process_ctx::ExecutionPhase};
use golden_io::PayloadPool;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
};
use ultraleap_runtime::UltraleapRuntimePoll;
pub(in super::super) mod fixture;
use fixture::*;

#[test]
fn healthy_service_is_idle_without_actor_updates_wakes_or_snapshots() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    for _ in 0..100 {
        tick(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.nodes_due, 0);
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_builds, 0);
    }
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}
#[test]
fn missing_snapshot_preserves_input_until_a_real_projection_turn() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().push_back(row(1));
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_runtime(&mut ctx);
    assert!(module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
    let target = path(&engine, id, "values/info/last_event");
    until(|| {
        tick(&mut engine);
        value(&engine, target) == ParamValue::Str("status 1".into())
    });
    finish(&mut engine, id, &native);
}
#[test]
fn continuous_ordered_status_input_has_bounded_turns_and_unrelated_actor_progress() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().extend((0..512).map(row));
    let target = path(&engine, id, "values/info/last_event");
    let mut count = 0;
    until(|| {
        let snapshot = engine.process_tree_snapshot();
        module(&mut engine, id).engine_sync_bound_param_handles(&mut |node| snapshot.node(node)?.param_value.clone());
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        ctx.set_tree_snapshot(snapshot);
        module(&mut engine, id).service_runtime(&mut ctx);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        let before = count;
        for event in &engine.inbox.events {
            if let golden_core::events::EventKind::ParamChanged {
                param,
                new_value: ParamValue::Str(text),
                ..
            } = &event.kind
            {
                if *param == target {
                    assert_eq!(text, &format!("status {count}"));
                    count += 1;
                }
            }
        }
        assert!(count - before <= 16);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
        let root = engine.root;
        engine.edits.push(Edit::PatchMeta {
            node: root,
            patch: golden_core::node::NodeMetaPatch {
                short_name: Some(format!("progress_{count}")),
                ..Default::default()
            },
        });
        engine.apply_edits().unwrap();
        count == 512
    });
    assert_eq!(
        engine.nodes.get(engine.root).unwrap().node_data().meta.short_name,
        "progress_512"
    );
    finish(&mut engine, id, &native);
}
#[test]
fn stopped_worker_recovers_on_absolute_backoff_before_blocked_cleanup_releases() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let (release, rx) = std::sync::mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(rx);
    control.panic.store(true, Ordering::Release);
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).service.retry_at.is_some()
    });
    let first = module(&mut engine, id).service.retry_at.unwrap();
    let root = engine.root;
    engine.edits.push(Edit::PatchMeta {
        node: root,
        patch: golden_core::node::NodeMetaPatch {
            short_name: Some("progress".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).service.retry_at.is_some_and(|at| at > first)
    });
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    assert_eq!(native.metrics().active, 1);
    release.send(()).unwrap();
    until(|| {
        tick(&mut engine);
        control.opens.load(Ordering::Acquire) == 2
    });
    quiet(&mut engine);
    finish(&mut engine, id, &native);
}
mod projection;
