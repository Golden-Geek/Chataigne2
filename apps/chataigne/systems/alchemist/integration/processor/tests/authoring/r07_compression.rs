use super::*;

use golden_core::app::ProjectNode;

use crate::app::systems_alchemist_processor::{
    commit_mapping_compression, prepare_mapping_compression,
    processor_compression_enabled, processor_frozen_source,
    set_mapping_compression, MappingCompressionError,
    settle_pending_mapping_transitions,
    PROCESSOR_COMPRESSION_ENABLED_DECL_ID,
    PROCESSOR_FROZEN_SOURCE_DECL_ID,
};

#[test]
fn compression_removes_real_workflow_nodes_and_expansion_restores_their_uuids() {
    let (mut engine, _, processor) = mapping_engine();
    let source_uuid = source_param(&mut engine, "Compression source", 0.42);
    let sink_uuid = source_param(&mut engine, "Compression sink", 0.0);
    let inputs = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(
        &mut engine,
        input,
        "source",
        ParamValue::Reference(NodeReference::new(source_uuid)),
    );
    let outputs = region(&engine, processor, "outputs");
    let command = create_parameter_output(&mut engine, outputs, sink_uuid);

    let before = engine.process_tree_snapshot();
    let archived_value = before
        .find_child_by_decl_id(command, "value")
        .expect("Set Parameter value should be addressable while editable");
    let region_uuids = ["inputs", "filters", "outputs"]
        .map(|id| before.node(region(&engine, processor, id)).unwrap().uuid);
    let workflow_ids = region_uuids.map(|uuid| before.node_id_by_uuid(uuid).unwrap());
    assert!(!processor_compression_enabled(&before, processor));
    drop(before);

    set_mapping_compression(&mut engine, processor, true)
        .expect("valid Mapping should compress");
    let compressed = engine.process_tree_snapshot();
    assert!(processor_compression_enabled(&compressed, processor));
    assert!(region_uuids
        .iter()
        .all(|uuid| compressed.node_id_by_uuid(*uuid).is_none()));
    assert!(workflow_ids
        .iter()
        .all(|node| compressed.node(*node).is_none()));
    let frozen = processor_frozen_source(&compressed, processor)
        .expect("compressed source should decode")
        .expect("compressed source should exist");
    assert_eq!(frozen.authored_regions.len(), 3);
    let encoded = compressed
        .find_child_by_decl_id(processor, PROCESSOR_FROZEN_SOURCE_DECL_ID)
        .and_then(|node| compressed.node(node))
        .and_then(|node| node.param_value.as_ref())
        .and_then(|value| match value {
            ParamValue::Str(value) => Some(value),
            _ => None,
        })
        .unwrap();
    assert!(!encoded.is_empty());
    drop(compressed);
    assert!(
        !engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: archived_value,
                value: ParamValue::Float(9.0),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success,
        "stale workflow NodeIds must not remain editable while compressed"
    );
    let frozen_control = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(processor, PROCESSOR_FROZEN_SOURCE_DECL_ID)
        .unwrap();
    assert!(
        !engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: frozen_control,
                value: ParamValue::Str("{}".to_owned()),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success,
        "the frozen archive must reject public edits"
    );
    let locked = engine.process_tree_snapshot();
    let formula = locked.find_child_by_decl_id(processor, "formula").unwrap();
    let formula_before = locked.node(formula).unwrap().param_value.clone().unwrap();
    drop(locked);
    assert!(
        !engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: formula,
                value: ParamValue::Reference(NodeReference::default()),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success,
        "compressed Mapping authoring controls must reject public edits"
    );
    let locked = engine.process_tree_snapshot();
    assert_eq!(locked.node(formula).unwrap().param_value, Some(formula_before));
    drop(locked);

    set_mapping_compression(&mut engine, processor, false)
        .expect("compressed Mapping should expand");
    let expanded = engine.process_tree_snapshot();
    assert!(!processor_compression_enabled(&expanded, processor));
    assert!(region_uuids
        .iter()
        .all(|uuid| expanded.node_id_by_uuid(*uuid).is_some()));
    let frozen_value = expanded
        .find_child_by_decl_id(processor, PROCESSOR_FROZEN_SOURCE_DECL_ID)
        .and_then(|node| expanded.node(node))
        .and_then(|node| node.param_value.as_ref());
    assert_eq!(frozen_value, Some(&ParamValue::Str(String::new())));
}

#[test]
fn compression_commit_rejects_stale_preparation_without_changing_representation() {
    let (mut engine, _, processor) = mapping_engine();
    let sink_uuid = source_param(&mut engine, "Stale sink", 0.0);
    let prepared = prepare_mapping_compression(&engine, processor)
        .expect("empty Mapping should prepare");
    let outputs = region(&engine, processor, "outputs");
    create_parameter_output(&mut engine, outputs, sink_uuid);

    let error = commit_mapping_compression(&mut engine, prepared)
        .expect_err("authored change must stale the prepared candidate");
    assert_eq!(error, MappingCompressionError::Stale);
    let snapshot = engine.process_tree_snapshot();
    assert!(!processor_compression_enabled(&snapshot, processor));
    assert!(snapshot
        .find_child_by_decl_id(processor, PROCESSOR_COMPRESSION_ENABLED_DECL_ID)
        .is_some());
    assert!(snapshot.node(outputs).is_some());
}

#[test]
fn compressed_runtime_tracks_live_sources_without_replaying_on_mode_changes() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let source_uuid = source_param(&mut engine, "Runtime source", 1.0);
    let sink_uuid = source_param(&mut engine, "Runtime sink", 0.0);
    let inputs = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(
        &mut engine,
        input,
        "source",
        ParamValue::Reference(NodeReference::new(source_uuid)),
    );
    let outputs = region(&engine, processor, "outputs");
    let output = create_parameter_output(&mut engine, outputs, sink_uuid);
    let bindings = OutputBindingConfig {
        send_policy: OutputSendPolicy::OnChange,
        ..OutputBindingConfig::default()
    };
    set_config(
        &mut engine,
        output,
        "bindings",
        ParamValue::Str(bindings.to_authoring_json().unwrap()),
    );

    for _ in 0..8 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let source = engine
        .process_tree_snapshot()
        .node_id_by_uuid(source_uuid)
        .unwrap();
    let sink = engine
        .process_tree_snapshot()
        .node_id_by_uuid(sink_uuid)
        .unwrap();
    assert_eq!(
        engine.process_tree_snapshot().node(sink).unwrap().param_value,
        Some(ParamValue::Float(1.0))
    );
    let sink_events = |engine: &AppEngine| {
        engine
            .ui_event_log()
            .iter()
            .filter(|event| {
                matches!(event.kind, golden_core::events::EventKind::ParamChanged { param, .. } if param == sink)
            })
            .count()
    };
    let initial_events = sink_events(&engine);

    set_mapping_compression(&mut engine, processor, true).unwrap();
    for _ in 0..4 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_eq!(
        sink_events(&engine),
        initial_events,
        "compression must not replay an unchanged accepted output"
    );
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: source,
                value: ParamValue::Float(2.0),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success
    );
    for _ in 0..4 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_eq!(
        engine.process_tree_snapshot().node(sink).unwrap().param_value,
        Some(ParamValue::Float(2.0))
    );
    let compressed_events = sink_events(&engine);
    assert_eq!(compressed_events, initial_events + 1);

    set_mapping_compression(&mut engine, processor, false).unwrap();
    for _ in 0..4 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_eq!(
        sink_events(&engine),
        compressed_events,
        "expansion must not replay an unchanged accepted output"
    );
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::SetParam {
                node: source,
                value: ParamValue::Float(3.0),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .success
    );
    for _ in 0..4 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_eq!(
        engine.process_tree_snapshot().node(sink).unwrap().param_value,
        Some(ParamValue::Float(3.0))
    );
    assert_eq!(sink_events(&engine), compressed_events + 1);
}

#[test]
fn production_runtime_trigger_uses_the_backend_transition_and_reports_blockers() {
    let (mut engine, _, processor) = mapping_engine();
    let outputs = region(&engine, processor, "outputs");
    create_item(&mut engine, outputs, GENERIC_LOG_COMMAND_NODE_TYPE);
    let snapshot = engine.process_tree_snapshot();
    let compress = snapshot.find_child_by_decl_id(processor, "compress").unwrap();
    let compression = snapshot
        .find_child_by_decl_id(processor, PROCESSOR_COMPRESSION_ENABLED_DECL_ID)
        .unwrap();
    let error = snapshot
        .find_child_by_decl_id(processor, "compression_error")
        .unwrap();
    drop(snapshot);
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(
            ProjectFileSpec::new("Mapping compression", "mapping-compression"),
            None,
        ),
    );
    let result = runtime.apply_ui_transaction(
        UiEditIntent::SetParam {
            node: compress,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        },
        Some("mapping-compression-test"),
    );
    assert!(result.acknowledgement.success, "{result:?}");
    let snapshot = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    let node = |id| snapshot.nodes.iter().find(|node| node.node_id == id).unwrap();
    let UiNodeDataDto::Parameter { param: compression } = &node(compression).data
    else {
        panic!("compression status should remain an ordinary parameter");
    };
    assert_eq!(compression.value, ParamValue::Bool(false));
    let UiNodeDataDto::Parameter { param: error } = &node(error).data else {
        panic!("compression error should remain an ordinary parameter");
    };
    assert!(
        matches!(&error.value, ParamValue::Str(value) if value.contains("unsupported_command")),
        "unexpected transition error: {:?}",
        error.value
    );
    assert!(node(processor).children.contains(&outputs));
}

#[test]
fn compressed_mapping_persists_and_restores_after_reload() {
    let (mut engine, _, processor) = mapping_engine();
    let processor_uuid = engine.process_tree_snapshot().node(processor).unwrap().uuid;
    let region_uuids = ["inputs", "filters", "outputs"].map(|id| {
        let region = region(&engine, processor, id);
        engine.process_tree_snapshot().node(region).unwrap().uuid
    });
    set_mapping_compression(&mut engine, processor, true).unwrap();
    let encoded = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&encoded).unwrap();
    let loaded_processor = loaded.node_id_by_uuid(processor_uuid).unwrap();
    let snapshot = loaded.process_tree_snapshot();
    assert!(processor_compression_enabled(&snapshot, loaded_processor));
    assert!(region_uuids
        .iter()
        .all(|uuid| snapshot.node_id_by_uuid(*uuid).is_none()));
    assert!(processor_frozen_source(&snapshot, loaded_processor)
        .unwrap()
        .is_some());
    drop(snapshot);

    set_mapping_compression(&mut loaded, loaded_processor, false).unwrap();
    let restored = loaded.process_tree_snapshot();
    assert!(!processor_compression_enabled(&restored, loaded_processor));
    assert!(region_uuids
        .iter()
        .all(|uuid| restored.node_id_by_uuid(*uuid).is_some()));
}

#[test]
fn compression_and_expansion_each_undo_and_redo_as_one_transition() {
    let (mut engine, _, processor) = mapping_engine();
    let region_uuids = ["inputs", "filters", "outputs"].map(|id| {
        let region = region(&engine, processor, id);
        engine.process_tree_snapshot().node(region).unwrap().uuid
    });
    set_mapping_compression(&mut engine, processor, true).unwrap();
    assert!(
        engine
            .apply_ui_intent(UiEditIntent::Undo)
            .success,
        "one undo should reverse compression"
    );
    let editable = engine.process_tree_snapshot();
    assert!(!processor_compression_enabled(&editable, processor));
    assert!(region_uuids
        .iter()
        .all(|uuid| editable.node_id_by_uuid(*uuid).is_some()));
    drop(editable);
    assert!(engine.apply_ui_intent(UiEditIntent::Redo).success);
    let compressed = engine.process_tree_snapshot();
    assert!(processor_compression_enabled(&compressed, processor));
    assert!(region_uuids
        .iter()
        .all(|uuid| compressed.node_id_by_uuid(*uuid).is_none()));
    drop(compressed);

    set_mapping_compression(&mut engine, processor, false).unwrap();
    assert!(engine.apply_ui_intent(UiEditIntent::Undo).success);
    let recompressed = engine.process_tree_snapshot();
    assert!(processor_compression_enabled(&recompressed, processor));
    assert!(region_uuids
        .iter()
        .all(|uuid| recompressed.node_id_by_uuid(*uuid).is_none()));
    drop(recompressed);
    assert!(engine.apply_ui_intent(UiEditIntent::Redo).success);
    let reexpanded = engine.process_tree_snapshot();
    assert!(!processor_compression_enabled(&reexpanded, processor));
    assert!(region_uuids
        .iter()
        .all(|uuid| reexpanded.node_id_by_uuid(*uuid).is_some()));
}

#[test]
fn duplicated_compressed_mapping_gets_a_disjoint_archived_identity_map() {
    let (mut engine, _, processor) = mapping_engine();
    set_mapping_compression(&mut engine, processor, true).unwrap();
    let snapshot = engine.process_tree_snapshot();
    let parent = snapshot.node(processor).unwrap().parent.unwrap();
    let original = processor_frozen_source(&snapshot, processor)
        .unwrap()
        .unwrap();
    let original_uuids = original
        .authored_regions
        .iter()
        .map(|region| region.document.root.uuid)
        .collect::<Vec<_>>();
    drop(snapshot);
    engine
        .duplicate_subtree_with(
            processor,
            parent,
            Some(processor),
            None,
            |node| node.project_encode_data(),
            AppNode::project_decode_node,
        )
        .expect("compressed Mapping should duplicate through the project codec");
    settle_pending_mapping_transitions(&mut engine).unwrap();

    let snapshot = engine.process_tree_snapshot();
    let duplicate = snapshot
        .child_ids(parent)
        .into_iter()
        .find(|node| {
            *node != processor
                && snapshot
                    .node(*node)
                    .is_some_and(|node| node.node_type == StateProcessor::NODE_TYPE)
        })
        .expect("compressed Mapping duplicate should exist");
    let duplicate_source = processor_frozen_source(&snapshot, duplicate)
        .unwrap()
        .unwrap();
    let duplicate_uuids = duplicate_source
        .authored_regions
        .iter()
        .map(|region| region.document.root.uuid)
        .collect::<Vec<_>>();
    assert!(original_uuids
        .iter()
        .all(|uuid| !duplicate_uuids.contains(uuid)));
    drop(snapshot);

    set_mapping_compression(&mut engine, processor, false).unwrap();
    set_mapping_compression(&mut engine, duplicate, false).unwrap();
    let expanded = engine.process_tree_snapshot();
    assert!(original_uuids
        .iter()
        .all(|uuid| expanded.node_id_by_uuid(*uuid).is_some()));
    assert!(duplicate_uuids
        .iter()
        .all(|uuid| expanded.node_id_by_uuid(*uuid).is_some()));
}
