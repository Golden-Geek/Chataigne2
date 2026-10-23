use super::*;
use golden_core::{engine::EngineEditError, node::NodeMetaPatch};
fn rejected(engine: &mut AppEngine, id: NodeId, ctx: &mut ProcessCtx) {
    engine.absorb_edits(ctx).unwrap();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(
        value(engine, path(engine, id, "values/info/tracking_id")),
        ParamValue::Str(String::new())
    );
}
#[test]
fn queued_retirement_keeps_source_quota_until_rejected_projection_disposal() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let pool = control.sources.lock().unwrap().as_ref().unwrap().clone();
    let mut ctx = stage(&mut engine, id, &control);
    module(&mut engine, id).stop_runtime();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!pool.wait_for_idle(Duration::from_millis(10)));
    rejected(&mut engine, id, &mut ctx);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn queued_joint_hand_info_ancestor_and_setting_rename_or_reparent_rejects_projection() {
    for name in [
        "values/joints/head",
        "values/joints",
        "values/hands/hands_speed",
        "values/hands",
        "values/info",
        "values",
        "parameters/reference_space",
        "parameters",
        "connection",
    ] {
        for rename in [true, false] {
            let (mut engine, id, control, native) = create();
            quiet(&mut engine);
            let available = path(&engine, id, "values/info/sensor_available");
            let mut ctx = stage(&mut engine, id, &control);
            let target = path(&engine, id, name);
            if rename {
                engine.edits.push(Edit::PatchMeta {
                    node: target,
                    patch: NodeMetaPatch {
                        short_name: Some("moved_kinect".into()),
                        ..Default::default()
                    },
                });
            } else {
                engine.edits.push(Edit::MoveNode {
                    node: target,
                    new_parent: engine.root,
                    new_prev_sibling: None,
                });
            }
            engine.apply_edits().unwrap();
            engine.absorb_edits(&mut ctx).unwrap();
            assert!(matches!(
                engine.apply_edits(),
                Err(EngineEditError::EditPreconditionFailed { .. })
            ));
            assert_eq!(value(&engine, available), ParamValue::Bool(false));
            finish(&mut engine, id, &native);
        }
    }
}
#[test]
fn queued_reference_space_edit_rejects_stale_values_and_tracking_message() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let mut ctx = stage(&mut engine, id, &control);
    set_space(&mut engine, id, KINECT2_SPACE_TORSO);
    rejected(&mut engine, id, &mut ctx);
    assert_eq!(
        value(&engine, path(&engine, id, "values/info/last_event")),
        ParamValue::Str(String::new())
    );
    finish(&mut engine, id, &native);
}
#[test]
fn fresh_snapshot_cancels_old_frame_before_reference_notification_and_reprojects_current_space() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().push_back(frame(7));
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
    set_space(&mut engine, id, KINECT2_SPACE_TORSO);
    let mut ctx = project(&mut engine, id);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(
        value(&engine, path(&engine, id, "values/info/tracking_id")),
        ParamValue::Str(String::new())
    );
    let target = path(&engine, id, "values/joints/head");
    until(|| {
        tick(&mut engine);
        matches!(value(&engine,target),ParamValue::Vec3(x,_,_) if (x-0.2).abs()<1e-6)
    });
    quiet(&mut engine);
    assert!(matches!(value(&engine,target),ParamValue::Vec3(x,_,_) if (x-0.2).abs()<1e-6));
    assert_eq!(
        value(&engine, path(&engine, id, "values/info/last_event")),
        ParamValue::Str("Tracking body 8".into())
    );
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}
#[test]
fn disable_rejects_queued_skeleton_projection() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let mut ctx = stage(&mut engine, id, &control);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    rejected(&mut engine, id, &mut ctx);
    finish(&mut engine, id, &native);
}
#[test]
fn source_byte_pressure_retains_frame_and_allows_unrelated_actor_progress() {
    source_pressure(true);
}
#[test]
fn source_item_pressure_retains_frame_and_allows_unrelated_actor_progress() {
    source_pressure(false);
}
fn source_pressure(bytes: bool) {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let pool = control.sources.lock().unwrap().as_ref().unwrap().clone();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let mut held = Vec::new();
    if bytes {
        held.push(pool.try_reserve(64 * kinect2_runtime::EVENT_WEIGHT).unwrap());
    } else {
        while let Ok(reservation) = pool.try_reserve(1) {
            held.push(reservation);
        }
    }
    control.rows.lock().unwrap().push_back(frame(7));
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().pressure());
    tick(&mut engine);
    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: NodeMetaPatch {
            short_name: Some("unrelated_progress".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    drop(held);
    let target = path(&engine, id, "values/info/tracking_id");
    until(|| {
        tick(&mut engine);
        value(&engine, target) == ParamValue::Str("8".into())
    });
    finish(&mut engine, id, &native);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn eight_enable_disable_cycles_recover_tracking_with_one_native_owner() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    for index in 0..8 {
        engine.edits.push(Edit::PatchMeta {
            node: id,
            patch: NodeMetaPatch {
                enabled: Some(false),
                ..Default::default()
            },
        });
        engine.apply_edits().unwrap();
        assert!(native.wait_for_idle(Duration::from_secs(2)));
        engine.edits.push(Edit::PatchMeta {
            node: id,
            patch: NodeMetaPatch {
                enabled: Some(true),
                ..Default::default()
            },
        });
        engine.apply_edits().unwrap();
        until(|| {
            tick(&mut engine);
            control.opens.load(Ordering::Acquire) == index + 2
        });
        emit(&mut engine, id, &control, frame(index));
        assert_eq!(
            value(&engine, path(&engine, id, "values/info/tracking_id")),
            ParamValue::Str((index + 1).to_string())
        );
        assert_eq!(native.metrics().active, 1);
    }
    finish(&mut engine, id, &native);
}
