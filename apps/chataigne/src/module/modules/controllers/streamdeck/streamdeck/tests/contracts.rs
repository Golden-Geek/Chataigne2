use super::*;
use golden_core::{engine::EngineEditError, events::EventKind, process_ctx::ProcessCtx};
fn context(engine: &crate::app::AppEngine) -> ProcessCtx {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    ctx
}
#[test]
fn healthy_idle_has_no_scheduled_or_worker_callbacks_or_snapshot_rebuilds() {
    let (mut engine, _) = create_streamdeck_module();
    run_tick(&mut engine);
    for _ in 0..100 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.run_tick(Duration::from_millis(2)).unwrap();
        engine.apply_edits().unwrap();
        let stats = engine.tick_stats();
        assert_eq!(stats.nodes_due, 0);
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_builds, 0);
    }
}
#[test]
fn script_simulation_works_without_native_owner_and_preserves_callback_order() {
    let (mut engine, id) = create_streamdeck_module();
    module_mut(&mut engine, id).stop_runtime();
    module_mut(&mut engine, id).service.suppressed = true;
    let mut ctx = context(&engine);
    module_mut(&mut engine, id).script_input(&mut ctx, 0, true).unwrap();
    module_mut(&mut engine, id).script_input(&mut ctx, 0, false).unwrap();
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let names: Vec<_> = engine
        .inbox
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) => event.payload["callback"].as_str(),
            _ => None,
        })
        .filter(|name| name.starts_with("streamDeckKey"))
        .collect();
    assert_eq!(names, vec!["streamDeckKeyPressed", "streamDeckKeyReleased"]);
    assert_eq!(bool_param(&engine, id, "values/keys/Key 1"), Some(false));
    assert!(module_mut(&mut engine, id).service.runtime.is_none());
}
#[test]
fn queued_simulation_rejects_renamed_source_and_target_paths() {
    for path in ["parameters/keys", "values/keys", "values/keys/Key 1"] {
        let (mut engine, id) = create_streamdeck_module();
        let mut ctx = context(&engine);
        module_mut(&mut engine, id).script_input(&mut ctx, 0, true).unwrap();
        let target = find_path(&engine, id, "values/keys/Key 1").unwrap();
        let renamed = find_path(&engine, id, path).unwrap();
        engine.edits.push(Edit::PatchMeta {
            node: renamed,
            patch: NodeMetaPatch {
                short_name: Some("renamed".into()),
                ..Default::default()
            },
        });
        engine.absorb_edits(&mut ctx).unwrap();
        assert!(matches!(
            engine.apply_edits(),
            Err(EngineEditError::EditPreconditionFailed { .. })
        ));
        assert_eq!(
            engine.nodes.get(target).unwrap().engine_param_snapshot().unwrap().value,
            ParamValue::Bool(false)
        );
    }
}
#[test]
fn add_page_captures_template_at_invocation_before_later_parameter_edits() {
    let (mut engine, id) = create_streamdeck_module();
    let color = find_path(&engine, id, "parameters/keys/Key 1/Color").unwrap();
    set_param(&mut engine, color, ParamValue::Color(1.0, 0.0, 0.0, 1.0));
    let mut ctx = context(&engine);
    module_mut(&mut engine, id)
        .engine_call_script_method(&mut ctx, "addPage", &[ParamValue::Str("Captured".into())])
        .unwrap();
    engine.absorb_edits(&mut ctx).unwrap();
    set_param(&mut engine, color, ParamValue::Color(0.0, 0.0, 1.0, 1.0));
    for _ in 0..3 {
        run_tick(&mut engine);
    }
    assert_eq!(
        param_value(&engine, id, "parameters/pages/captured/Key 1/Color"),
        Some(ParamValue::Color(1.0, 0.0, 0.0, 1.0))
    );
}
#[test]
fn model_resize_preserves_existing_parameter_uuid_and_pressed_value() {
    let (mut engine, id) = create_streamdeck_module();
    let target = find_path(&engine, id, "values/keys/Key 1").unwrap();
    let uuid = engine.nodes.get(target).unwrap().node_data().meta.uuid;
    set_param(&mut engine, target, ParamValue::Bool(true));
    set_model(&mut engine, id, "pedal");
    assert_eq!(find_path(&engine, id, "values/keys/Key 1"), Some(target));
    assert_eq!(engine.nodes.get(target).unwrap().node_data().meta.uuid, uuid);
    assert_eq!(bool_param(&engine, id, "values/keys/Key 1"), Some(true));
}
#[test]
fn oversized_page_names_key_indices_and_templates_are_rejected_explicitly() {
    let (mut engine, id) = create_streamdeck_module();
    let mut ctx = context(&engine);
    assert!(module_mut(&mut engine, id)
        .engine_call_script_method(&mut ctx, "addPage", &[ParamValue::Str("a".repeat(513))])
        .is_err());
    assert!(module_mut(&mut engine, id)
        .engine_call_script_method(&mut ctx, "pressKey", &[ParamValue::Int(-1)])
        .is_err());
    assert!(module_mut(&mut engine, id)
        .engine_call_script_method(&mut ctx, "pressKey", &[ParamValue::Int(64)])
        .is_err());
    let text = find_path(&engine, id, "parameters/keys/Key 1/Text").unwrap();
    set_param(&mut engine, text, ParamValue::Str("x".repeat(256 * 1024)));
    let mut ctx = context(&engine);
    assert!(module_mut(&mut engine, id)
        .engine_call_script_method(&mut ctx, "addPage", &[ParamValue::Str("Too large".into())])
        .unwrap_err()
        .contains("template"));
}
#[test]
fn page_authoring_queue_has_explicit_lifetime_admission() {
    let (mut engine, id) = create_streamdeck_module();
    let mut accepted = 0;
    for _ in 0..65 {
        let mut ctx = context(&engine);
        if let Err(error) = module_mut(&mut engine, id).engine_call_script_method(
            &mut ctx,
            "addPage",
            &[ParamValue::Str("Queued".into())],
        ) {
            assert!(error.contains("capacity") || error.contains("queue"));
            break;
        }
        accepted += 1;
    }
    assert!(accepted > 0 && accepted <= 64);
    module_mut(&mut engine, id).layout.cancel();
}

#[test]
fn eight_enable_cycles_retire_the_old_owner_and_recover() {
    let (mut engine, id) = create_streamdeck_module();
    for _ in 0..8 {
        engine.edits.push(Edit::PatchMeta {
            node: id,
            patch: NodeMetaPatch {
                enabled: Some(false),
                ..Default::default()
            },
        });
        engine.apply_edits().unwrap();
        assert!(module_mut(&mut engine, id)
            .service
            .owner
            .wait_for_idle(Duration::from_secs(1)));
        assert!(module_mut(&mut engine, id).service.runtime.is_none());
        engine.edits.push(Edit::PatchMeta {
            node: id,
            patch: NodeMetaPatch {
                enabled: Some(true),
                ..Default::default()
            },
        });
        engine.apply_edits().unwrap();
        for _ in 0..3 {
            run_tick(&mut engine);
        }
        assert!(module_mut(&mut engine, id)
            .service
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.running()));
    }
}

#[test]
fn queued_physical_input_rejects_source_rename_target_rename_and_retirement() {
    for path in [Some("parameters/keys"), Some("values/keys/Key 1"), None] {
        let (mut engine, id) = create_streamdeck_module();
        press_key(&mut engine, id, 0);
        let until = std::time::Instant::now() + Duration::from_secs(2);
        while !module_mut(&mut engine, id).service.runtime.as_ref().unwrap().pending() {
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut ctx = context(&engine);
        module_mut(&mut engine, id).service_turn(&mut ctx);
        let target = find_path(&engine, id, "values/keys/Key 1").unwrap();
        if let Some(path) = path {
            engine.edits.push(Edit::PatchMeta {
                node: find_path(&engine, id, path).unwrap(),
                patch: NodeMetaPatch {
                    short_name: Some("moved".into()),
                    ..Default::default()
                },
            });
        } else {
            module_mut(&mut engine, id).stop_runtime();
        }
        engine.absorb_edits(&mut ctx).unwrap();
        assert!(matches!(
            engine.apply_edits(),
            Err(EngineEditError::EditPreconditionFailed { .. })
        ));
        assert_eq!(
            engine.nodes.get(target).unwrap().engine_param_snapshot().unwrap().value,
            ParamValue::Bool(false)
        );
    }
}

#[test]
fn sixty_four_page_preparation_allows_unrelated_edits_between_bounded_turns() {
    use golden_core::{
        edit::NodeTree,
        parameter::{Parameter, ParameterChangeCheck},
    };
    let (mut engine, id) = create_streamdeck_module();
    let host = find_path(&engine, id, "parameters/pages").unwrap();
    engine.add_node(
        Parameter::new("Witness", ParamValue::Int(0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let witness = find_path(&engine, engine.root, "Witness").unwrap();
    for index in 0..64 {
        let mut page = Folder::new(format!("Page {index}"));
        crate::app::module::enable_module_authoring(page.node_data_mut());
        engine.edits.push(Edit::AddNodeTree {
            parent: host,
            prev_sibling: None,
            tree: NodeTree::new(page),
        });
    }
    engine.apply_edits().unwrap();
    let mut turns = 0;
    while turns == 0 || module_mut(&mut engine, id).layout.busy() {
        assert!(turns < 400);
        set_param(&mut engine, witness, ParamValue::Int(turns));
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.run_tick(Duration::from_millis(2)).unwrap();
        engine.apply_edits().unwrap();
        assert_eq!(
            engine
                .nodes
                .get(witness)
                .unwrap()
                .engine_param_snapshot()
                .unwrap()
                .value,
            ParamValue::Int(turns)
        );
        turns += 1;
    }
    assert!(turns >= 64);
    assert_eq!(enum_option_ids(&engine, active_page_param(&engine, id)).len(), 65);
    assert!(find_path(&engine, id, "values/pages/page_63/Key 6").is_some());
}

#[test]
fn all_models_can_derive_pages_and_keep_their_expected_key_count() {
    for (model, count) in [("mini", 6), ("standard", 15), ("xl", 32), ("plus", 8), ("pedal", 3)] {
        let (mut engine, id) = create_streamdeck_module();
        set_model(&mut engine, id, model);
        let mut ctx = context(&engine);
        module_mut(&mut engine, id)
            .engine_call_script_method(&mut ctx, "addPage", &[ParamValue::Str("Model Page".into())])
            .unwrap();
        engine.absorb_edits(&mut ctx).unwrap();
        for _ in 0..3 {
            run_tick(&mut engine);
        }
        assert!(find_path(&engine, id, &format!("values/pages/model_page/Key {count}")).is_some());
        assert!(find_path(&engine, id, &format!("values/pages/model_page/Key {}", count + 1)).is_none());
    }
}

#[test]
fn device_loss_clears_connection_and_automatically_recovers() {
    let (mut engine, id) = create_streamdeck_module();
    assert_eq!(bool_param(&engine, id, "connection/connected"), Some(true));
    module_mut(&mut engine, id).simulated().set_available(false);
    let until = std::time::Instant::now() + Duration::from_secs(3);
    while bool_param(&engine, id, "connection/connected") != Some(false) {
        assert!(std::time::Instant::now() < until);
        run_tick(&mut engine);
    }
    module_mut(&mut engine, id).simulated().set_available(true);
    while bool_param(&engine, id, "connection/connected") != Some(true) {
        assert!(std::time::Instant::now() < until);
        run_tick(&mut engine);
    }
    press_key(&mut engine, id, 0);
    run_tick(&mut engine);
    assert_eq!(bool_param(&engine, id, "values/keys/Key 1"), Some(true));
}
