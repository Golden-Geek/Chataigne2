use super::*;
use golden_core::{engine::EngineEditError, node::NodeMetaPatch};
fn stage(engine: &mut AppEngine, id: NodeId, control: &Control) -> ProcessCtx {
    control.rows.lock().unwrap().push_back(row(7));
    until(|| module(engine, id).service.runtime.as_ref().unwrap().has_pending());
    let snapshot = engine.process_tree_snapshot();
    module(engine, id).engine_sync_bound_param_handles(&mut |node| snapshot.node(node)?.param_value.clone());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(snapshot);
    module(engine, id).service_runtime(&mut ctx);
    ctx
}
fn rejected(engine: &mut AppEngine, id: NodeId, ctx: &mut ProcessCtx) {
    engine.absorb_edits(ctx).unwrap();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(
        value(engine, path(engine, id, "values/info/last_event")),
        ParamValue::Str(String::new())
    );
}
#[test]
fn queued_retirement_keeps_original_source_until_rejected_application_disposes_it() {
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
fn queued_leaf_and_ancestor_rename_or_reparent_reject_before_value_projection() {
    for name in [
        "values/info/last_event",
        "values/info",
        "values/left_hand/left_palm_position",
        "values/left_hand",
        "values/right_hand",
        "values/metrics",
        "values",
    ] {
        for rename in [true, false] {
            let (mut engine, id, control, native) = create();
            quiet(&mut engine);
            let mut ctx = stage(&mut engine, id, &control);
            let target = path(&engine, id, name);
            if rename {
                engine.edits.push(Edit::PatchMeta {
                    node: target,
                    patch: NodeMetaPatch {
                        short_name: Some("moved_tracking".into()),
                        ..Default::default()
                    },
                });
            } else {
                engine.edits.push(Edit::MoveNode {
                    node: target,
                    new_parent: path(&engine, id, "parameters"),
                    new_prev_sibling: None,
                });
            }
            engine.apply_edits().unwrap();
            engine.absorb_edits(&mut ctx).unwrap();
            assert!(matches!(
                engine.apply_edits(),
                Err(EngineEditError::EditPreconditionFailed { .. })
            ));
            let connected = path(&engine, id, "connection/connected");
            assert_eq!(value(&engine, connected), ParamValue::Bool(false));
            finish(&mut engine, id, &native);
        }
    }
}
#[test]
fn module_disable_rejects_queued_native_state_and_releases_retained_input() {
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
fn source_byte_pressure_preserves_pending_status_while_unrelated_actor_edits_progress() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let pool = control.sources.lock().unwrap().as_ref().unwrap().clone();
    let held = pool.try_retain(0u8, 64 * ultraleap_runtime::EVENT_WEIGHT).unwrap();
    control.rows.lock().unwrap().push_back(row(7));
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().pressure());
    tick(&mut engine);
    let root = engine.root;
    engine.edits.push(Edit::PatchMeta {
        node: root,
        patch: NodeMetaPatch {
            short_name: Some("unrelated_progress".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert_eq!(
        engine.nodes.get(root).unwrap().node_data().meta.short_name,
        "unrelated_progress"
    );
    drop(held);
    let last = path(&engine, id, "values/info/last_event");
    until(|| {
        tick(&mut engine);
        value(&engine, last) == ParamValue::Str("status 7".into())
    });
    finish(&mut engine, id, &native);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn eight_disable_enable_cycles_keep_one_native_owner_and_recover_input() {
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
        assert_eq!(native.metrics().active, 0);
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
        control.rows.lock().unwrap().push_back(row(index));
        let target = path(&engine, id, "values/info/last_event");
        until(|| {
            tick(&mut engine);
            value(&engine, target) == ParamValue::Str(format!("status {index}"))
        });
        assert_eq!(native.metrics().active, 1);
    }
    finish(&mut engine, id, &native);
}
