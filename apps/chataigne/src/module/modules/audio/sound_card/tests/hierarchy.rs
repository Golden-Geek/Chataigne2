use golden_core::parameter::RangeConstraint;

use super::*;

#[test]
fn default_hierarchy_has_named_ranged_output_gains_and_no_unused_folders() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SoundCardModule::create().into(), None);
    for _ in 0..8 {
        engine
            .apply_edits()
            .expect("Sound Card declarations should materialize");
    }

    let module = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Sound Card module");
    let snapshot = engine.process_tree_snapshot();
    let connection = find_path(&snapshot, module, "connection").expect("connection container");
    let first_connection_child = snapshot
        .child_at(connection, 0)
        .and_then(|child| snapshot.node(child))
        .expect("first connection child");
    assert_eq!(
        first_connection_child.decl_id, "connection/connected",
        "Connected should stay at the top of the connection container",
    );
    assert!(
        find_path(
            &snapshot,
            module,
            "connection/input_routing/routes"
        )
        .is_some()
    );
    assert!(
        find_path(
            &snapshot,
            module,
            "connection/output_routing/routes"
        )
        .is_some()
    );

    let parameters = find_path(&snapshot, module, "parameters").expect("parameters container");
    assert!(
        snapshot
            .child_ids_slice(parameters)
            .iter()
            .filter_map(|child| snapshot.node(*child))
            .all(|child| child.label != "Folder"),
        "Parameters must not retain an unnamed empty folder",
    );

    let input_channels =
        find_path(&snapshot, module, "parameters/input/channels").expect("input channel container");
    assert!(
        snapshot.child_ids_slice(input_channels).is_empty(),
        "input gains should not materialize before an input device is selected",
    );

    let output_channels =
        find_path(&snapshot, module, "parameters/output/channels").expect("output channel container");
    let output_gains = snapshot.child_ids_slice(output_channels);
    assert_eq!(output_gains.len(), 2);
    for (index, channel) in output_gains.iter().enumerate() {
        let channel = snapshot.node(*channel).expect("output channel gain");
        let name = format!("Output {}", index + 1);
        assert_eq!(channel.label, format!("{name} Gain"));
        assert_eq!(channel.short_name, name);
        assert_eq!(
            channel
                .param_constraints
                .as_ref()
                .and_then(|constraints| constraints.range.clone()),
            RangeConstraint::uniform(Some(-120.0), Some(24.0)),
        );
        assert!(matches!(channel.param_value, Some(ParamValue::Float(_))));
        assert!(snapshot.child_ids_slice(channel.id).is_empty());
    }

    assert!(find_path(&snapshot, module, "values/levels").is_some());
    assert!(find_path(&snapshot, module, "values/levels/input").is_none());
    assert!(find_path(&snapshot, module, "values/levels/output").is_some());
    assert!(find_path(&snapshot, module, "values/input").is_none());
    assert!(find_path(&snapshot, module, "values/output").is_none());
    assert!(find_path(&snapshot, module, "values/playback_status").is_some());
    for obsolete in [
        "input_levels",
        "output_levels",
        "global_levels",
        "pitch_results",
        "spectrum_bands",
        "diagnostics",
    ] {
        assert!(
            find_path(&snapshot, module, format!("values/{obsolete}").as_str()).is_none(),
            "obsolete Values folder {obsolete} must not materialize",
        );
    }
}

#[test]
fn selecting_input_materializes_ranged_input_levels_before_output_levels() {
    let (mut engine, module) = create_sound_card();
    select_enum_parameter(
        &mut engine,
        module,
        "connection/input_device",
        SYSTEM_DEFAULT_DEVICE,
    );

    let snapshot = engine.process_tree_snapshot();
    let input_channels =
        find_path(&snapshot, module, "parameters/input/channels").expect("input channel container");
    let input_gains = snapshot.child_ids_slice(input_channels);
    assert_eq!(input_gains.len(), 2);
    for (index, channel) in input_gains.iter().enumerate() {
        let channel = snapshot.node(*channel).expect("input channel gain");
        let name = format!("Input {}", index + 1);
        assert_eq!(channel.label, format!("{name} Gain"));
        assert_eq!(channel.short_name, name);
        assert_eq!(
            channel
                .param_constraints
                .as_ref()
                .and_then(|constraints| constraints.range.clone()),
            RangeConstraint::uniform(Some(-120.0), Some(24.0)),
        );
    }

    let levels = find_path(&snapshot, module, "values/levels").expect("Levels folder");
    let direction_labels = snapshot
        .child_ids_slice(levels)
        .iter()
        .filter_map(|child| snapshot.node(*child))
        .map(|child| child.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(direction_labels, vec!["Input", "Output"]);

    for direction in ["input", "output"] {
        let master = find_path(
            &snapshot,
            module,
            format!("values/levels/{direction}/master_level").as_str(),
        )
        .and_then(|node| snapshot.node(node))
        .expect("direction master level");
        assert_eq!(
            master
                .param_constraints
                .as_ref()
                .and_then(|constraints| constraints.range.clone()),
            RangeConstraint::uniform(Some(0.0), Some(1.0)),
        );

        let channels = find_path(
            &snapshot,
            module,
            format!("values/levels/{direction}/channels").as_str(),
        )
        .expect("direction channel levels");
        for channel in snapshot.child_ids_slice(channels) {
            let channel = snapshot.node(*channel).expect("channel level");
            assert_eq!(
                channel
                    .param_constraints
                    .as_ref()
                    .and_then(|constraints| constraints.range.clone()),
                RangeConstraint::uniform(Some(0.0), Some(1.0)),
            );
        }
    }
}

#[test]
fn processing_values_follow_pitch_and_spectral_enablement_without_legacy_bands() {
    let (mut engine, module) = create_sound_card();
    let snapshot = engine.process_tree_snapshot();
    let processing =
        find_path(&snapshot, module, "parameters/processing").expect("Processing parameter container");
    assert_eq!(
        spectral_processing_children(&snapshot, processing).len(),
        1,
        "fresh modules must declare exactly one Spectral Analysis container",
    );
    let spectral_parameter = find_path(
        &snapshot,
        module,
        "parameters/processing/spectral_analysis",
    )
    .expect("Spectral Analysis parameter container");
    let spectral_state = snapshot
        .node(spectral_parameter)
        .expect("Spectral Analysis parameter container");
    let expected_spectral_uuid = structure::derived_uuid(
        snapshot.node(module).expect("Sound Card module").uuid,
        structure::SPECTRAL_PARAMETERS_UUID_KEY,
    );
    assert_eq!(spectral_state.uuid, expected_spectral_uuid);
    assert_eq!(spectral_state.node_type, "folder");
    assert!(!spectral_state.enabled);
    assert!(spectral_state.can_be_disabled);
    assert_eq!(spectral_state.child_count, 0);
    assert!(find_path(&snapshot, module, "values/pitch_detection").is_none());
    assert!(find_path(&snapshot, module, "values/spectral_analysis").is_none());
    stabilize_sound_card(&mut engine);
    let fresh_stable_snapshot = engine.process_tree_snapshot();
    assert_eq!(
        spectral_processing_children(&fresh_stable_snapshot, processing),
        vec![spectral_parameter],
        "fresh reconciliation must keep one stable Spectral Analysis identity",
    );

    let mut stale_toggle = Parameter::new(
        "Spectral Analysis",
        ParamValue::Bool(false),
        ParameterChangeCheck::ValueChange,
    );
    stale_toggle.node_data_mut().meta.decl_id =
        DeclId("parameters/processing/spectral_analysis".to_owned());
    stale_toggle.node_data_mut().meta.short_name = "spectral_analysis".to_owned();
    engine.add_node(stale_toggle.into(), Some(processing));
    let mut stale_folder = Folder::new("Spectral Analysis");
    stale_folder.node_data_mut().meta.decl_id =
        DeclId("parameters/processing/spectral_analysis".to_owned());
    stale_folder.node_data_mut().meta.short_name = "spectral_analysis".to_owned();
    stale_folder.node_data_mut().meta.can_be_disabled = true;
    stale_folder.node_data_mut().meta.enabled = false;
    engine.add_node(stale_folder.into(), Some(processing));
    engine
        .apply_edits()
        .expect("stale spectral duplicate fixtures should materialize");
    let duplicate_snapshot = engine.process_tree_snapshot();
    assert_eq!(
        spectral_processing_children(&duplicate_snapshot, processing).len(),
        3,
        "fixture should reproduce stale bool and folder duplicates beside the backend-owned folder",
    );
    stabilize_sound_card(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let spectral_children = spectral_processing_children(&snapshot, processing);
    assert_eq!(
        spectral_children.len(),
        1,
        "reconciliation must deterministically remove stale Spectral Analysis declarations",
    );
    let spectral_parameter = spectral_children[0];
    let spectral_state = snapshot
        .node(spectral_parameter)
        .expect("retained Spectral Analysis folder");
    assert_eq!(spectral_state.node_type, "folder");
    assert_eq!(spectral_state.uuid, expected_spectral_uuid);
    assert_eq!(
        spectral_parameter,
        snapshot
            .node_id_by_uuid(expected_spectral_uuid)
            .expect("derived spectral parameter identity"),
    );
    stabilize_sound_card(&mut engine);
    let stable_snapshot = engine.process_tree_snapshot();
    assert_eq!(
        spectral_processing_children(&stable_snapshot, processing),
        vec![spectral_parameter],
        "extra stabilization must not re-add a duplicate Spectral Analysis folder",
    );

    patch_enabled(&mut engine, spectral_parameter, true);
    stabilize_sound_card(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let spectral_values =
        find_path(&snapshot, module, "values/spectral_analysis").expect("Spectral Analysis values");
    assert!(
        snapshot.child_ids_slice(spectral_values).is_empty(),
        "Spectral Analysis values stay empty until the band manager is introduced",
    );

    engine.add_node(Folder::new("Legacy Band").into(), Some(spectral_values));
    engine
        .apply_edits()
        .expect("legacy spectral band fixture should materialize");
    patch_enabled(&mut engine, spectral_parameter, false);
    stabilize_sound_card(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    assert!(
        find_path(&snapshot, module, "values/spectral_analysis").is_none(),
        "disabling Spectral Analysis should remove its complete legacy subtree in one edit",
    );

    let pitch_parameter =
        find_path(&snapshot, module, "parameters/processing/pitch_detection").expect("Pitch Detection parameter");
    set_param_value(&mut engine, pitch_parameter, ParamValue::Bool(true));
    stabilize_sound_card(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    assert!(find_path(&snapshot, module, "values/pitch_detection").is_some());
    assert!(find_path(&snapshot, module, "values/pitch_results").is_none());

    set_param_value(&mut engine, pitch_parameter, ParamValue::Bool(false));
    stabilize_sound_card(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    assert!(find_path(&snapshot, module, "values/pitch_detection").is_none());
}

#[test]
fn channel_names_remain_free_form_while_gain_labels_and_runtime_names_diverge() {
    let (mut engine, module) = create_sound_card();
    let snapshot = engine.process_tree_snapshot();
    let output_channels =
        find_path(&snapshot, module, "parameters/output/channels").expect("output channel container");
    let channel = snapshot.child_ids_slice(output_channels)[0];
    let channel_uuid = snapshot.node(channel).expect("output gain").uuid.0.to_string();
    assert_eq!(
        snapshot.node(channel).map(|state| state.short_name.as_str()),
        Some("Output 1"),
    );

    let authored_name = "Front L/R (Main) #1";
    let mut ctx = test_process_ctx(41);
    engine
        .nodes
        .get(module)
        .expect("Sound Card module")
        .as_any()
        .downcast_ref::<SoundCardModule>()
        .expect("Sound Card module type")
        .rename_channel(
            &mut ctx,
            &snapshot,
            AudioDirection::Output,
            channel_uuid.as_str(),
            authored_name,
        )
        .expect("free-form channel rename");
    for request in ctx.edits.drain() {
        engine.edits.push(request.edit);
    }
    engine.apply_edits().expect("channel rename should apply");

    let snapshot = engine.process_tree_snapshot();
    let channel = snapshot.node(channel).expect("renamed output gain");
    assert_eq!(channel.label, format!("{authored_name} Gain"));
    assert_eq!(channel.short_name, authored_name);
    let references = structure::channel_references(
        &snapshot,
        module,
        "parameters/output/channels",
    );
    assert_eq!(
        references[0].cached_name.as_deref(),
        Some(authored_name),
        "route references must cache the semantic channel name",
    );
    assert_eq!(
        runtime::output_channel_names_for_test(&snapshot, module).expect("runtime output channels")[0],
        authored_name,
        "the audio runtime must not receive the gain-control suffix",
    );
}

fn spectral_processing_children(
    snapshot: &ProcessTreeSnapshot,
    processing: NodeId,
) -> Vec<NodeId> {
    snapshot
        .child_ids_slice(processing)
        .iter()
        .copied()
        .filter(|child| {
            snapshot.node(*child).is_some_and(|node| {
                node.decl_id
                    .rsplit('/')
                    .next()
                    .is_some_and(|decl_id| decl_id == "spectral_analysis")
                    || node.label == "Spectral Analysis"
            })
        })
        .collect()
}
