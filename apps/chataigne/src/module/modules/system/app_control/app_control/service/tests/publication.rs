use super::*;

#[test]
fn construction_and_connected_idle_have_no_scheduled_or_polling_actor_work() {
    let node = AppControlModule::create();
    assert!(node.service.runtime.is_none());
    assert!(!node.needs_update());
    let (mut engine, id) = create();
    assert!(module(&mut engine, id).service.runtime.is_none());
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots);
    quiet(&mut engine, id);
    assert_eq!(control.samples.load(Ordering::Acquire), 1);
    assert_eq!(
        value(&engine, path(&engine, id, "connection/connected")),
        ParamValue::Bool(true)
    );
    for _ in 0..100 {
        turn(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_rebuilds, 0);
    }
    finish(&mut engine, id, &slots);
}

#[test]
fn reports_publish_eight_rows_per_turn_and_keep_original_quota_until_all_rows_are_applied() {
    let (mut engine, id) = create();
    watch_apps(&mut engine, id, 20);
    let slots = slots();
    let reports = payloads(1);
    install(
        &mut engine,
        id,
        fake(Control::default()),
        &slots,
        reports.clone(),
        payloads(256),
    );
    quiet(&mut engine, id);
    let cpus = (0..20)
        .map(|n| path(&engine, id, &format!("values/watched_apps_values/Watch{n}/cpu_ratio")))
        .collect::<Vec<_>>();
    let before = cpus.iter().map(|id| value(&engine, *id)).collect::<Vec<_>>();
    let report = take_report(&mut engine, id);
    let expected = ParamValue::Float(report.rows.apps[0].metrics.cpu_ratio);
    assert_ne!(before[0], expected);
    module(&mut engine, id).service.report = Some((report, 0));
    for (done, remaining) in [(8, 12), (16, 4), (20, 0)] {
        let mut ctx = context(&engine);
        let snapshot = ctx.tree_snapshot_arc().unwrap();
        module(&mut engine, id).project_report(&mut ctx, &snapshot);
        assert_eq!(
            module(&mut engine, id)
                .service
                .report
                .as_ref()
                .map_or(0, |(report, row)| report.config.apps.len() - row),
            remaining
        );
        assert!(reports.try_reserve(1).is_err());
        flush(&mut engine, &mut ctx);
        for (n, cpu) in cpus.iter().enumerate() {
            assert_eq!(
                value(&engine, *cpu),
                if n < done { expected.clone() } else { before[n].clone() }
            );
        }
    }
    finish(&mut engine, id, &slots);
    assert!(reports.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn queued_report_retirement_rejects_values_and_holds_quota_through_rejection() {
    let (mut engine, id) = create();
    watch_apps(&mut engine, id, 1);
    let slots = slots();
    let reports = payloads(1);
    install(
        &mut engine,
        id,
        fake(Control::default()),
        &slots,
        reports.clone(),
        payloads(256),
    );
    quiet(&mut engine, id);
    let cpu = path(&engine, id, "values/watched_apps_values/Watch0/cpu_ratio");
    let before = value(&engine, cpu);
    let report = take_report(&mut engine, id);
    module(&mut engine, id).service.report = Some((report, 0));
    let mut ctx = context(&engine);
    let snapshot = ctx.tree_snapshot_arc().unwrap();
    module(&mut engine, id).project_report(&mut ctx, &snapshot);
    engine.absorb_edits(&mut ctx).unwrap();
    finish(&mut engine, id, &slots);
    assert!(reports.try_reserve(1).is_err());
    assert!(matches!(
        engine.apply_edits(),
        Err(golden_core::engine::EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(value(&engine, cpu), before);
    assert!(reports.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn changing_a_watch_after_projection_rejects_its_queued_sample() {
    let (mut engine, id) = create();
    let watch = watch_apps(&mut engine, id, 1)[0];
    let slots = slots();
    attach(&mut engine, id, Control::default(), &slots);
    quiet(&mut engine, id);
    let cpu = path(&engine, id, "values/watched_apps_values/Watch0/cpu_ratio");
    let before = value(&engine, cpu);
    let report = take_report(&mut engine, id);
    module(&mut engine, id).service.report = Some((report, 0));
    let mut ctx = context(&engine);
    let snapshot = ctx.tree_snapshot_arc().unwrap();
    module(&mut engine, id).project_report(&mut ctx, &snapshot);
    set(&mut engine, watch, ParamValue::File("fixture/new.exe".into()));
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(matches!(
        engine.apply_edits(),
        Err(golden_core::engine::EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(value(&engine, cpu), before);
    finish(&mut engine, id, &slots);
}

#[test]
fn two_accepted_folder_reports_keep_distinct_callbacks_and_append_trigger_occurrences() {
    let (mut engine, id) = create();
    let root = path(&engine, id, "parameters/watched_folders_targets");
    engine.add_user_item(crate::app::AppControlWatchedFolder::new().into(), Some(root));
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let item = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let target = path(&engine, item, "target");
    let changed = path(&engine, item, "changed");
    set(&mut engine, target, ParamValue::File("fixture-folder".into()));
    let slots = slots();
    let control = Control::default();
    control.folder_changes.store(true, Ordering::Release);
    attach(&mut engine, id, control, &slots);
    quiet(&mut engine, id);
    let first = take_report(&mut engine, id);
    let second = take_report(&mut engine, id);
    let mut ctx = context(&engine);
    let snapshot = ctx.tree_snapshot_arc().unwrap();
    for report in [first, second] {
        module(&mut engine, id).service.report = Some((report, 0));
        module(&mut engine, id).project_report(&mut ctx, &snapshot);
    }
    flush(&mut engine, &mut ctx);
    assert_eq!(callbacks(&engine.inbox.events, "watchFolderChanged"), 2);
    let triggers = engine
        .inbox
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                golden_core::events::EventKind::ParamChanged {
                    param,
                    new_value: ParamValue::Trigger(),
                    ..
                } if param == changed
            )
        })
        .count();
    assert_eq!(triggers, 2);
    finish(&mut engine, id, &slots);
}

#[test]
fn a_large_command_context_refreshes_in_bounded_cursor_turns_without_replacing_the_live_selection() {
    let (mut engine, id) = create();
    let watch = watch_apps(&mut engine, id, 1)[0];
    let slots = slots();
    attach(&mut engine, id, Control::default(), &slots);
    quiet(&mut engine, id);
    let mut tree = golden_core::edit::NodeTree::new(Folder::new("Context"));
    for n in 0..512 {
        tree.push_child(golden_core::edit::NodeTree::new(Folder::new(format!("filler{n}"))));
    }
    tree.push_child(golden_core::edit::NodeTree::new(
        AppControlLaunchProcessCommand::create(),
    ));
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: id,
        prev_sibling: None,
    });
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let context_root = path(&engine, id, "Context");
    let command = engine.nodes.get(context_root).unwrap().node_data().last_child.unwrap();
    let choice = path(&engine, command, "watched_app");
    engine.edits.push(Edit::PatchMeta {
        node: watch,
        patch: NodeMetaPatch {
            label: Some("Renamed".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    let mut ctx = context(&engine);
    let snapshot = ctx.tree_snapshot_arc().unwrap();
    let apps = module(&mut engine, id).collect_watched_apps(&snapshot);
    module(&mut engine, id).sync_command_target_options(&mut ctx, &snapshot, &apps);
    assert!(module(&mut engine, id).service.catalog.is_some());
    flush(&mut engine, &mut ctx);
    let mut turns = 1;
    while module(&mut engine, id).service.catalog.is_some() {
        let mut ctx = context(&engine);
        let snapshot = ctx.tree_snapshot_arc().unwrap();
        module(&mut engine, id).service_catalog(&mut ctx, &snapshot);
        flush(&mut engine, &mut ctx);
        turns += 1;
        assert!(turns < 32);
    }
    assert!(turns > 4);
    let state = engine.nodes.get(choice).unwrap().engine_param_snapshot().unwrap();
    assert!(state
        .constraints
        .enum_options
        .iter()
        .any(|option| option.variant_id == "Renamed"));
    assert_eq!(state.value, ParamValue::Enum("Watch0".into()));
    finish(&mut engine, id, &slots);
}

#[test]
fn oversized_watch_paths_are_refused_and_a_later_bounded_configuration_recovers() {
    let (mut engine, id) = create();
    let watch = watch_apps(&mut engine, id, 1)[0];
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots);
    quiet(&mut engine, id);
    set(&mut engine, watch, ParamValue::File("x".repeat(1024 * 1024)));
    quiet(&mut engine, id);
    assert_eq!(
        module(&mut engine, id).service.config.as_ref().unwrap().apps[0].target_path,
        "fixture/0.exe"
    );
    assert!(control
        .configs
        .lock()
        .unwrap()
        .iter()
        .all(|config| config.apps[0].target_path == "fixture/0.exe"));
    set(&mut engine, watch, ParamValue::File("fixture/recovered.exe".into()));
    quiet(&mut engine, id);
    assert_eq!(
        module(&mut engine, id).service.config.as_ref().unwrap().apps[0].target_path,
        "fixture/recovered.exe"
    );
    finish(&mut engine, id, &slots);
}

#[test]
fn sixty_five_watches_reject_without_partial_authoring_and_sixty_four_materialize_complete_trees() {
    let (mut engine, id) = create();
    let watches = watch_apps(&mut engine, id, 65);
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots);
    quiet(&mut engine, id);
    assert!(module(&mut engine, id).service.config.is_none());
    assert!(module(&mut engine, id).watched_app_value_folders.is_empty());
    assert!(control.configs.lock().unwrap().is_empty());
    engine.edits.push(Edit::RemoveNode { node: watches[64] });
    engine.apply_edits().unwrap();
    quiet(&mut engine, id);
    assert_eq!(module(&mut engine, id).service.config.as_ref().unwrap().apps.len(), 64);
    assert_eq!(module(&mut engine, id).watched_app_value_folders.len(), 64);
    for folder in module(&mut engine, id)
        .watched_app_value_folders
        .values()
        .copied()
        .collect::<Vec<_>>()
    {
        assert_eq!(engine.process_tree_snapshot().node(folder).unwrap().child_count, 12);
    }
    finish(&mut engine, id, &slots);
}
