use super::super::tests::fixture::{body, frame, sample};
use super::*;
use crate::app::{AppEngine, AppNode};
use golden_core::{edit::Edit, node::Folder, process_ctx::ExecutionPhase};
use golden_io::PayloadPool;
use kinect2_runtime::{KinectJoint, KinectRuntimePoll, KinectVec3};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
};
mod fixture;
use fixture::*;
mod features;
pub(in super::super) use features::{
    hand_metrics_compute_distance_rotation_and_speed, reference_space_changes_joint_output_origin,
};
mod projection;
#[test]
fn healthy_idle_has_no_actor_callbacks_wakes_or_snapshots() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    for _ in 0..100 {
        tick(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(
            (
                stats.nodes_due,
                stats.callbacks_fired,
                stats.wake_callbacks_fired,
                stats.snapshot_builds
            ),
            (0, 0, 0, 0)
        );
    }
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}
#[test]
fn missing_snapshot_keeps_input_pending_for_real_projection() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().push_back(frame(7));
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_runtime(&mut ctx);
    assert!(module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
    let target = path(&engine, id, "values/info/tracking_id");
    until(|| {
        tick(&mut engine);
        value(&engine, target) == ParamValue::Str("8".into())
    });
    finish(&mut engine, id, &native);
}
#[test]
fn continuous_input_retains_512_tracking_boundaries_with_unrelated_actor_progress() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().extend((0..512).map(frame));
    let target = path(&engine, id, "values/info/last_event");
    let mut count = 0;
    until(|| {
        let mut ctx = project(&mut engine, id);
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
                    assert_eq!(text, &format!("Tracking body {}", count + 1));
                    count += 1;
                }
            }
        }
        assert!(count - before <= 16);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
        engine.edits.push(Edit::PatchMeta {
            node: engine.root,
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
fn stopped_worker_recovers_on_backoff_before_blocked_cleanup_finishes() {
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
    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
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
