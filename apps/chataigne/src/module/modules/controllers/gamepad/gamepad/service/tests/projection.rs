use super::*;
use golden_core::{engine::EngineEditError, node::NodeMetaPatch};

#[test]
fn blank_persisted_selection_normalizes_once_and_keeps_later_catalog_domains() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let selected = path(&engine, id, "connection/device");
    let snapshot = engine.process_tree_snapshot();
    let node = snapshot.node(selected).unwrap();
    let mut options = gamepad_device_options(&[device()]);
    options.push(enum_option("", "Blank", 7));
    engine.edits.push(Edit::SetParamEnumOptions {
        node: selected,
        expected_uuid: node.uuid,
        expected_value: node.param_value.clone().unwrap(),
        selected_value: Some(ParamValue::Enum(String::new())),
        options: options.into(),
    });
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    module(&mut engine, id).engine_sync_bound_param_handles(&mut |node| snapshot.node(node)?.param_value.clone());
    let mut configure = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    configure.set_tree_snapshot(snapshot);
    assert!(module(&mut engine, id).configure_runtime(&mut configure));
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().queued() == 1);
    control
        .rows
        .lock()
        .unwrap()
        .extend([Record::Catalog(vec![device()]), Record::Catalog(vec![])]);
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().queued() == 3);
    let mut ctx = project(&mut engine, id);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(value(&engine, selected), ParamValue::Enum("auto".into()));
    let actual = engine
        .nodes
        .get(selected)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints
        .enum_options;
    let expected = module(&mut engine, id)
        .service
        .catalog
        .as_ref()
        .unwrap()
        .prepared
        .catalog
        .options
        .clone();
    assert!(actual.same_storage(&expected));
    assert_eq!(actual.len(), 2);
    finish(&mut engine, id, &native);
}
fn rejected(engine: &mut AppEngine, id: NodeId, ctx: &mut ProcessCtx) {
    engine.absorb_edits(ctx).unwrap();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(
        value(engine, path(engine, id, "values/buttons/south")),
        ParamValue::Bool(false)
    );
    assert!(!engine.inbox.events.iter().any(|e|matches!(&e.kind,golden_core::events::EventKind::Custom(c) if c.payload["callback"]==GAMEPAD_BUTTON_PRESSED_CALLBACK)));
}
#[test]
fn queued_retirement_retains_original_capacity_until_rejected_input_disposal() {
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
fn queued_leaf_and_ancestor_rename_or_reparent_reject_before_any_value_or_callback() {
    for name in [
        "values/buttons/south",
        "values/buttons",
        "values/axes/left_stick",
        "values/axes",
        "values/info",
        "values",
        "connection",
        "parameters/axis_processing/left_stick_x",
        "parameters/axis_processing",
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
                        short_name: Some("moved_gamepad".into()),
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
            assert!(!engine.inbox.events.iter().any(|e|matches!(&e.kind,golden_core::events::EventKind::Custom(c) if c.payload["callback"]==GAMEPAD_BUTTON_PRESSED_CALLBACK)));
            finish(&mut engine, id, &native);
        }
    }
}
#[test]
fn queued_selection_and_calibration_changes_reject_before_callbacks_and_values() {
    for (name, new_value) in [
        ("connection/device", ParamValue::Enum("none".into())),
        (
            "parameters/axis_processing/left_stick_x/left_stick_x_offset",
            ParamValue::Float(0.2),
        ),
    ] {
        let (mut engine, id, control, native) = create();
        quiet(&mut engine);
        let mut ctx = stage(&mut engine, id, &control);
        let target = path(&engine, id, name);
        set(&mut engine, target, new_value);
        rejected(&mut engine, id, &mut ctx);
        finish(&mut engine, id, &native);
    }
}
#[test]
fn fresh_snapshot_configuration_rejects_old_input_before_its_notification() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().push_back(edge(true));
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
    let selected = path(&engine, id, "connection/device");
    set(&mut engine, selected, ParamValue::Enum("none".into()));
    let mut ctx = project(&mut engine, id);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(
        value(&engine, path(&engine, id, "values/buttons/south")),
        ParamValue::Bool(false)
    );
    until(|| {
        tick(&mut engine);
        module(&mut engine, id)
            .service
            .catalog
            .as_ref()
            .is_some_and(|r| r.current() && r.prepared.catalog.selection == "none")
    });
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}
#[test]
fn disable_rejects_queued_publications() {
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
fn source_byte_pressure_preserves_pending_edge_and_unrelated_actor_progress() {
    source_pressure(true);
}
#[test]
fn source_item_pressure_preserves_pending_edge_and_unrelated_actor_progress() {
    source_pressure(false);
}
fn source_pressure(bytes: bool) {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let pool = control.sources.lock().unwrap().as_ref().unwrap().clone();
    let mut held = Vec::new();
    if bytes {
        held.push(
            pool.try_reserve(32 * 1024 * 1024 - gamepad_runtime::CATALOG_WEIGHT - 64 * 1024)
                .unwrap(),
        );
    } else {
        while let Ok(reservation) = pool.try_reserve(1) {
            held.push(reservation);
        }
    }
    control.rows.lock().unwrap().push_back(edge(true));
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
    let button = path(&engine, id, "values/buttons/south");
    until(|| {
        tick(&mut engine);
        value(&engine, button) == ParamValue::Bool(true)
    });
    finish(&mut engine, id, &native);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn eight_enable_disable_cycles_recover_input_with_one_native_owner() {
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
        control.rows.lock().unwrap().push_back(edge(true));
        let button = path(&engine, id, "values/buttons/south");
        until(|| {
            tick(&mut engine);
            value(&engine, button) == ParamValue::Bool(true)
        });
        assert_eq!(native.metrics().active, 1);
    }
    finish(&mut engine, id, &native);
}
