use super::*;
use golden_core::{engine::EngineEditError, node::NodeMetaPatch};

fn path(engine: &AppEngine, root: NodeId, path: &str) -> NodeId {
    super::super::super::tests::find_path(engine, root, path).unwrap()
}

fn stage(engine: &mut AppEngine, id: NodeId, control: &Control) -> ProcessCtx {
    control.records.lock().unwrap().push_back(event(true));
    until(|| module(engine, id).input.runtime.as_ref().unwrap().has_pending());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    module(engine, id).service_input(&mut ctx);
    assert!(!module(engine, id).input.runtime.as_ref().unwrap().has_pending());
    ctx
}

fn callback_count(engine: &AppEngine) -> usize {
    engine
        .inbox
        .events
        .iter()
        .filter(|event| {
            matches!(&event.kind, EventKind::Custom(event)
            if event.payload["callback"] == MOUSE_BUTTON_CHANGED_CALLBACK)
        })
        .count()
}

fn service_without_notifications(engine: &mut AppEngine, id: NodeId) {
    let snapshot = engine.process_tree_snapshot();
    module(engine, id).engine_sync_bound_param_handles(&mut |node| snapshot.node(node)?.param_value.clone());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(snapshot);
    module(engine, id).service_input(&mut ctx);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
}

fn rejected(engine: &mut AppEngine, id: NodeId, ctx: &mut ProcessCtx) {
    let before = callback_count(engine);
    engine.absorb_edits(ctx).unwrap();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    let left = module(engine, id).left.id();
    assert_eq!(
        engine.nodes.get(left).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Bool(false)
    );
    assert_eq!(callback_count(engine), before);
}

#[test]
fn queued_native_retirement_retains_the_original_source_until_rejected_disposal() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let pool = control.sources.lock().unwrap().as_ref().unwrap().clone();
    let mut ctx = stage(&mut engine, id, &control);
    module(&mut engine, id).stop_input_backend();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!pool.wait_for_idle(Duration::from_millis(10)));
    rejected(&mut engine, id, &mut ctx);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn queued_leaf_and_ancestor_rename_or_reparent_reject_before_values_and_callbacks() {
    for target in [
        "values/pointer/position",
        "values/pointer",
        "values/buttons/left",
        "values/buttons",
        "values",
    ] {
        for rename in [true, false] {
            let (mut engine, id, control, native) = create();
            quiet(&mut engine);
            let mut ctx = stage(&mut engine, id, &control);
            let target = path(&engine, id, target);
            if rename {
                engine.edits.push(Edit::PatchMeta {
                    node: target,
                    patch: NodeMetaPatch {
                        short_name: Some("moved_input".into()),
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
            rejected(&mut engine, id, &mut ctx);
            finish(&mut engine, id, &native);
        }
    }
}

#[test]
fn queued_selection_receive_capture_and_send_changes_reject_input_projection() {
    for (target, value) in [
        ("connection/device", ParamValue::Enum("none".into())),
        ("connection/receive_enabled", ParamValue::Bool(false)),
        ("connection/capture_os_input", ParamValue::Bool(true)),
        ("connection/send_enabled", ParamValue::Bool(false)),
    ] {
        let (mut engine, id, control, native) = create();
        quiet(&mut engine);
        let mut ctx = stage(&mut engine, id, &control);
        engine.edits.push(Edit::SetParam {
            node: path(&engine, id, target),
            value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().unwrap();
        rejected(&mut engine, id, &mut ctx);
        finish(&mut engine, id, &native);
    }
}

#[test]
fn capture_configuration_reopens_and_retires_old_publications_before_notification() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    for (target, value, selection) in [
        ("connection/capture_os_input", ParamValue::Bool(true), "auto"),
        ("connection/device", ParamValue::Enum("fake:mouse".into()), "fake:mouse"),
        ("connection/capture_os_input", ParamValue::Bool(false), "fake:mouse"),
    ] {
        let old = module(&mut engine, id).input.catalog.as_ref().unwrap().clone();
        let opened = control.opens.load(Ordering::Acquire);
        engine.edits.push(Edit::SetParam {
            node: path(&engine, id, target),
            value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().unwrap();
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let snapshot = engine.process_tree_snapshot();
        module(&mut engine, id).engine_sync_bound_param_handles(&mut |node| snapshot.node(node)?.param_value.clone());
        ctx.set_tree_snapshot(snapshot);
        module(&mut engine, id).service_input(&mut ctx);
        assert!(!old.current());
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        until(|| {
            service_without_notifications(&mut engine, id);
            control.opens.load(Ordering::Acquire) > opened
                && module(&mut engine, id)
                    .current_catalog()
                    .is_some_and(|catalog| catalog.selection == selection)
        });
        control.records.lock().unwrap().push_back(event(true));
        let left = module(&mut engine, id).left.id();
        until(|| {
            service_without_notifications(&mut engine, id);
            engine.nodes.get(left).unwrap().engine_param_snapshot().unwrap().value == ParamValue::Bool(true)
        });
        quiet(&mut engine);
        assert_eq!(
            engine.nodes.get(left).unwrap().engine_param_snapshot().unwrap().value,
            ParamValue::Bool(true)
        );
    }
    finish(&mut engine, id, &native);
}

#[test]
fn persisted_label_selection_publishes_its_canonical_domain_before_normalization() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let device = module(&mut engine, id).device.id();
    let uuid = engine.nodes.get(device).unwrap().node_data().meta.uuid;
    let legacy = "fake:mouse|Old Mouse Label";
    let mut options = mouse_device_options(&[]);
    options.push(enum_option(legacy, "Old Mouse Label", 10));
    engine.edits.push(Edit::SetParamEnumOptions {
        node: device,
        expected_uuid: uuid,
        expected_value: ParamValue::Enum("auto".into()),
        selected_value: None,
        options: options.into(),
    });
    engine.edits.push(Edit::SetParam {
        node: device,
        value: ParamValue::Enum(legacy.into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).device.get_ref().as_str() == "fake:mouse"
            && module(&mut engine, id)
                .current_catalog()
                .is_some_and(|catalog| catalog.selection == "fake:mouse")
    });
    assert_eq!(engine.nodes.get(device).unwrap().node_data().meta.uuid, uuid);
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    let mut options = engine
        .nodes
        .get(device)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints
        .enum_options
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    options.push(enum_option("absent:mouse", "Missing Mouse", 20));
    engine.edits.push(Edit::SetParamEnumOptions {
        node: device,
        expected_uuid: uuid,
        expected_value: ParamValue::Enum("fake:mouse".into()),
        selected_value: None,
        options: options.into(),
    });
    engine.edits.push(Edit::SetParam {
        node: device,
        value: ParamValue::Enum("absent:mouse".into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    until(|| {
        tick(&mut engine);
        module(&mut engine, id)
            .current_catalog()
            .is_some_and(|catalog| catalog.selection == "absent:mouse" && catalog.selected.is_none())
    });
    let receive = module(&mut engine, id).receive_enabled.id();
    engine.edits.push(Edit::SetParam {
        node: receive,
        value: ParamValue::Bool(false),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    tick(&mut engine);
    finish(&mut engine, id, &native);
}
