use super::*;

#[test]
fn tree_conversion_uses_stable_uuid_ids_across_channel_rename_and_reorder() {
    let (mut engine, module) = create_sound_card_module();
    let inspector = null_audio_inspector();
    let before_snapshot = engine.process_tree_snapshot();
    let before = runtime::build_configuration(&before_snapshot, module, &inspector)
        .expect("fresh Sound Card should convert");
    let input_list = find_path(&engine, module, VIRTUAL_INPUTS_PATH).expect("input list");
    let inputs = typed_children(
        &engine,
        module,
        VIRTUAL_INPUTS_PATH,
        SoundCardVirtualInput::NODE_TYPE,
    );
    let expected = inputs
        .iter()
        .map(|input| {
            AudioChannelId::from_uuid(
                engine
                    .nodes
                    .get(*input)
                    .expect("input")
                    .node_data()
                    .meta
                    .uuid
                    .0,
            )
        })
        .collect::<HashSet<_>>();

    engine.edits.push(Edit::PatchMeta {
        node: inputs[0],
        patch: NodeMetaPatch {
            label: Some("Renamed Input".to_owned()),
            ..NodeMetaPatch::default()
        },
    });
    engine.edits.push(Edit::MoveNode {
        node: inputs[1],
        new_parent: input_list,
        new_prev_sibling: None,
    });
    engine.apply_edits().expect("rename and reorder should apply");
    stabilize(&mut engine);

    let after_snapshot = engine.process_tree_snapshot();
    let after = runtime::build_configuration(&after_snapshot, module, &inspector)
        .expect("renamed Sound Card should convert");
    assert_eq!(
        before
            .configuration
            .virtual_inputs
            .iter()
            .map(|channel| channel.id)
            .collect::<HashSet<_>>(),
        expected
    );
    assert_eq!(
        after
            .configuration
            .virtual_inputs
            .iter()
            .map(|channel| channel.id)
            .collect::<HashSet<_>>(),
        expected
    );
}

#[test]
fn missing_device_preserves_authored_topology_and_device_return_restores_patch() {
    let (engine, module) = create_sound_card_module();
    let output_device =
        find_path(&engine, module, "connection/output_device").expect("output device");
    let missing_target = AudioDeviceTargetId::Device {
        backend: BackendId::new("detached-backend").unwrap(),
        device: golden_audio::AudioDeviceId::new("detached-device").unwrap(),
    };
    let missing_value = runtime::device_target_value(&missing_target);
    let snapshot = engine
        .process_tree_snapshot()
        .with_param_values([(output_device, ParamValue::Enum(missing_value))]);
    let missing_state = AudioDeviceInspectorState {
        backends: vec![AudioBackendStatus {
            backend: BackendId::new("detached-backend").unwrap(),
            label: "Detached Backend".to_owned(),
            state: AudioBackendState::Available,
            detail: None,
        }],
        ..AudioDeviceInspectorState::default()
    };
    let missing = runtime::build_configuration(&snapshot, module, &missing_state)
        .expect("missing selected device should keep the authored graph valid");
    assert!(missing.configuration.output_patch.is_empty());
    assert_eq!(
        typed_children(
            &engine,
            module,
            OUTPUT_PROFILES_PATH,
            super::SoundCardOutputProfile::NODE_TYPE,
        )
        .into_iter()
        .flat_map(|profile| {
            direct_children_of_type(
                &engine,
                profile,
                super::SoundCardOutputPatchRoute::NODE_TYPE,
            )
        })
        .count(),
        2,
        "missing device must not remove authored routes"
    );
    assert!(
        missing.warnings.len() >= 2,
        "every unresolved physical route should remain visibly diagnosed"
    );

    let mut returned_state = missing_state;
    let mut returned_device = NullBackend.discover().unwrap().remove(0);
    returned_device.target = missing_target.clone();
    returned_device.profile_key = golden_audio::profile_key_for(&missing_target, None, None);
    returned_state.devices.push(returned_device);
    let returned = runtime::build_configuration(&snapshot, module, &returned_state)
        .expect("returned device should restore the physical patch");
    assert_eq!(returned.configuration.output_patch.len(), 2);
}

#[test]
fn deleted_channel_leaves_visible_unresolved_route_and_undo_restores_it_atomically() {
    let (mut engine, module) = create_sound_card_module();
    let monitoring =
        find_path(&engine, module, "parameters/monitoring_routes").expect("monitor routes");
    let input = typed_children(
        &engine,
        module,
        VIRTUAL_INPUTS_PATH,
        SoundCardVirtualInput::NODE_TYPE,
    )[0];
    let output = typed_children(
        &engine,
        module,
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    )[0];
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(SoundCardMonitorRoute::with_targets(
            node_reference(&engine, input),
            node_reference(&engine, output),
        )),
        parent: monitoring,
        prev_sibling: None,
    });
    engine.apply_edits().expect("monitor route should apply");
    stabilize(&mut engine);
    let route =
        direct_children_of_type(&engine, monitoring, SoundCardMonitorRoute::NODE_TYPE)[0];

    engine.edits.push(Edit::RemoveNode { node: input });
    engine.apply_edits().expect("channel delete should apply once");
    assert!(
        engine.nodes.contains(route),
        "dependent authored route remains visible and undoable"
    );
    let missing = runtime::build_configuration(
        &engine.process_tree_snapshot(),
        module,
        &null_audio_inspector(),
    )
    .expect("dangling route should be excluded, not reject the valid plan");
    assert!(missing.configuration.monitoring.is_empty());
    assert!(
        missing
            .warnings
            .iter()
            .any(|warning| warning.node == route)
    );

    engine.undo().expect("delete undo should succeed");
    stabilize(&mut engine);
    let restored = runtime::build_configuration(
        &engine.process_tree_snapshot(),
        module,
        &null_audio_inspector(),
    )
    .expect("undo should restore route target");
    assert_eq!(restored.configuration.monitoring.len(), 1);
}

#[test]
fn foreign_route_conversion_rejects_without_producing_a_replacement_plan() {
    let (mut engine, module) = create_sound_card_module();
    engine.add_node(SoundCardModule::create().into(), None);
    stabilize(&mut engine);
    let modules = direct_children_of_type(&engine, engine.root, SoundCardModule::NODE_TYPE);
    let local_inputs = typed_children(
        &engine,
        modules[0],
        VIRTUAL_INPUTS_PATH,
        SoundCardVirtualInput::NODE_TYPE,
    );
    let foreign_inputs = typed_children(
        &engine,
        modules[1],
        VIRTUAL_INPUTS_PATH,
        SoundCardVirtualInput::NODE_TYPE,
    );
    let monitoring = find_path(&engine, module, "parameters/monitoring_routes").unwrap();
    let local_outputs = typed_children(
        &engine,
        module,
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    );
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(SoundCardMonitorRoute::with_targets(
            node_reference(&engine, local_inputs[0]),
            node_reference(&engine, local_outputs[0]),
        )),
        parent: monitoring,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    stabilize(&mut engine);
    let route =
        direct_children_of_type(&engine, monitoring, SoundCardMonitorRoute::NODE_TYPE)[0];
    let input_param = find_path(&engine, route, "virtual_input").unwrap();
    let foreign_reference = node_reference(&engine, foreign_inputs[0]);
    let invalid_snapshot = engine
        .process_tree_snapshot()
        .with_param_values([(input_param, ParamValue::Reference(foreign_reference))]);
    let error = runtime::build_configuration(&invalid_snapshot, module, &null_audio_inspector())
        .expect_err("foreign route must not compile");
    assert_eq!(error.node, route);
}

#[test]
fn matrix_edits_coalesce_into_one_runtime_configuration_generation() {
    let (mut engine, module) = create_sound_card_module();
    run_sound_tick(&mut engine);
    let initial_generation = sound_card_generation(&engine, module);
    let profile = typed_children(
        &engine,
        module,
        OUTPUT_PROFILES_PATH,
        super::SoundCardOutputProfile::NODE_TYPE,
    )[0];
    let routes = direct_children_of_type(
        &engine,
        profile,
        super::SoundCardOutputPatchRoute::NODE_TYPE,
    );
    for (index, route) in routes.iter().enumerate() {
        let gain = find_path(&engine, *route, "gain_db").expect("route gain");
        set_param(
            &mut engine,
            gain,
            ParamValue::Float(-3.0 - index as f64),
        );
    }
    engine.apply_edits().expect("matrix gain batch should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("matrix events should dispatch");
    run_sound_tick(&mut engine);
    assert_eq!(
        sound_card_generation(&engine, module),
        initial_generation + 1
    );
}

#[test]
fn steady_observation_poll_does_not_require_a_tree_snapshot() {
    let (mut engine, _module) = create_sound_card_module();
    run_sound_tick(&mut engine);
    engine.apply_edits().expect("first observation edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("observation events should dispatch");
    run_sound_tick(&mut engine);
    engine.apply_edits().expect("second observation edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("second observation events should dispatch");
    run_sound_tick(&mut engine);
    assert_eq!(
        engine.tick_stats().snapshot_builds,
        0,
        "steady bounded value polling must use cached NodeIds"
    );
}

#[test]
fn null_backend_readiness_updates_module_connection_and_capabilities() {
    let (mut engine, module) = create_sound_card_module();

    for _ in 0..20 {
        run_sound_tick(&mut engine);
        if param_value(&engine, module, "connection/connected")
            .and_then(ParamValue::as_bool)
            .unwrap_or(false)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    assert_eq!(
        param_value(&engine, module, "connection/connected").and_then(ParamValue::as_bool),
        Some(true)
    );
    assert_eq!(
        param_value(&engine, module, "connection/can_receive").and_then(ParamValue::as_bool),
        Some(false)
    );
    assert_eq!(
        param_value(&engine, module, "connection/can_send").and_then(ParamValue::as_bool),
        Some(true)
    );
}

#[test]
fn module_removal_and_project_drop_shutdown_audio_workers() {
    let (mut engine, module) = create_sound_card_module();
    run_sound_tick(&mut engine);
    let removed_control = sound_card_control(&engine, module);
    engine.edits.push(Edit::RemoveNode { node: module });
    engine
        .apply_edits()
        .expect("Sound Card removal should retire its runtime");
    assert!(!engine.nodes.contains(module));
    wait_until_control_stops(&removed_control);

    let (mut replacement, replacement_module) = create_sound_card_module();
    run_sound_tick(&mut replacement);
    let replaced_control = sound_card_control(&replacement, replacement_module);
    drop(replacement);
    wait_until_control_stops(&replaced_control);
}

fn run_sound_tick(engine: &mut crate::app::AppEngine) {
    engine.resolve().expect("Sound Card schedule should resolve");
    engine
        .run_tick(Duration::from_millis(40))
        .expect("Sound Card update should run");
    for _ in 0..4 {
        engine.apply_edits().expect("Sound Card update edits should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("Sound Card update inbox should dispatch");
    }
}

fn sound_card_generation(engine: &crate::app::AppEngine, module: NodeId) -> u64 {
    let crate::app::AppNode::SoundCardModule(module) =
        engine.nodes.get(module).expect("Sound Card module")
    else {
        panic!("node should be a Sound Card module");
    };
    module
        .runtime
        .as_ref()
        .expect("Sound Card runtime")
        .generation()
        .get()
}

fn sound_card_control(
    engine: &crate::app::AppEngine,
    module: NodeId,
) -> golden_audio::AudioControl {
    let crate::app::AppNode::SoundCardModule(module) =
        engine.nodes.get(module).expect("Sound Card module")
    else {
        panic!("node should be a Sound Card module");
    };
    module
        .runtime
        .as_ref()
        .expect("Sound Card runtime")
        .control()
}

fn wait_until_control_stops(control: &golden_audio::AudioControl) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while control
        .submit(golden_audio::AudioCommand::SetEnabled(true))
        .is_ok()
    {
        assert!(
            Instant::now() < deadline,
            "retired Sound Card control worker did not stop"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn null_audio_inspector() -> AudioDeviceInspectorState {
    AudioDeviceInspectorState {
        backends: vec![AudioBackendStatus {
            backend: NullBackend::backend_id(),
            label: "Null / Offline".to_owned(),
            state: AudioBackendState::Available,
            detail: None,
        }],
        devices: NullBackend.discover().expect("null device"),
        ..AudioDeviceInspectorState::default()
    }
}
