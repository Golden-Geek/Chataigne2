use super::*;

const FILTER_WARNING_ID: &str = "mapping_filter_validation";
const CHAIN_WARNING_ID: &str = "mapping_filter_chain_validation";

#[test]
fn every_registered_filter_creates_persists_and_undoes_without_inputs() {
    let (mut engine, _, processor) = mapping_engine();
    let filters = region(&engine, processor, "filters");
    let catalog = engine.nodes.get(filters).unwrap().user_creatable_items();
    assert!(!catalog.is_empty());
    assert!(catalog
        .iter()
        .all(|item| !item.node_type.contains("/inputs/")));

    let mut created = Vec::new();
    for item in &catalog {
        let filter = create_item(&mut engine, filters, &item.node_type);
        created.push(engine.process_tree_snapshot().node(filter).unwrap().uuid);
    }
    assert_eq!(
        engine.process_tree_snapshot().child_ids(filters).len(),
        catalog.len()
    );

    let project = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&project).unwrap();
    let loaded_snapshot = loaded.process_tree_snapshot();
    for uuid in &created {
        assert!(
            loaded_snapshot.node_id_by_uuid(*uuid).is_some(),
            "filter {uuid:?} should survive sparse reload"
        );
    }

    for remaining in (0..catalog.len()).rev() {
        assert!(engine.apply_ui_intent(UiEditIntent::Undo).success);
        engine.apply_edits().unwrap();
        assert_eq!(
            engine.process_tree_snapshot().child_ids(filters).len(),
            remaining
        );
    }
}

#[test]
fn unresolved_filter_duplicates_through_the_shared_backend_contract() {
    let (mut engine, _, processor) = mapping_engine();
    let filters = region(&engine, processor, "filters");
    let pack_type = engine
        .nodes
        .get(filters)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| {
            item.node_type
                .starts_with(&format!("{ANODE_CREATE_PREFIX}pack_vec3@managed/"))
        })
        .unwrap()
        .node_type;
    let original = create_item(&mut engine, filters, &pack_type);
    let original_uuid = engine.process_tree_snapshot().node(original).unwrap().uuid;
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(
            ProjectFileSpec::new("Mapping", "mapping"),
            None,
        ),
    );
    let result = runtime.apply_ui_transaction(
        UiEditIntent::DuplicateNode {
            source: original,
            new_parent: filters,
            new_prev_sibling: Some(original),
            initial_params: Vec::new(),
        },
        Some("mapping-unresolved-filter-duplicate"),
    );
    assert!(
        result.acknowledgement.success,
        "unresolved filter duplication should apply: {:?}",
        result.acknowledgement
    );
    let snapshot = runtime
        .read_model()
        .snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    let region = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == filters)
        .unwrap();
    assert_eq!(region.children.len(), 2);
    let duplicate = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == region.children[1])
        .unwrap();
    assert_ne!(duplicate.uuid, original_uuid);
}

#[test]
fn every_registered_filter_creates_after_an_invalid_stage() {
    let (mut engine, _, processor) = mapping_engine();
    let filters = region(&engine, processor, "filters");
    let inputs = region(&engine, processor, "inputs");
    let catalog = engine.nodes.get(filters).unwrap().user_creatable_items();
    let pack_type = catalog
        .iter()
        .find(|item| {
            item.node_type
                .starts_with(&format!("{ANODE_CREATE_PREFIX}pack_vec3@managed/"))
        })
        .unwrap()
        .node_type
        .clone();
    let invalid_pack = create_item(&mut engine, filters, &pack_type);

    for (label, value) in [("X", 1.0), ("Y", 2.0)] {
        let source = source_param(&mut engine, label, value);
        let input = create_item(
            &mut engine,
            inputs,
            &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
        );
        set_config(
            &mut engine,
            input,
            "source",
            ParamValue::Reference(NodeReference::new(source)),
        );
    }
    for _ in 0..3 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_warning(&engine, invalid_pack, FILTER_WARNING_ID, "Filter is incompatible");

    for item in &catalog {
        create_item(&mut engine, filters, &item.node_type);
    }
    assert_eq!(
        engine.process_tree_snapshot().child_ids(filters).len(),
        catalog.len() + 1
    );
}

#[test]
fn invalid_fixed_arity_chain_warns_blocks_dispatch_and_recovers_in_place() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let filters = region(&engine, processor, "filters");
    let inputs = region(&engine, processor, "inputs");
    let outputs = region(&engine, processor, "outputs");

    let catalog = engine.nodes.get(filters).unwrap().user_creatable_items();
    let create_type = |type_id: &str| {
        catalog
            .iter()
            .find(|item| {
                item.node_type
                    .starts_with(&format!("{ANODE_CREATE_PREFIX}{type_id}@managed/"))
            })
            .unwrap()
            .node_type
            .clone()
    };
    let first_pack = create_item(&mut engine, filters, &create_type("pack_vec3"));
    let first_pack_uuid = engine.process_tree_snapshot().node(first_pack).unwrap().uuid;
    create_item(&mut engine, filters, &create_type("extract_vec3"));
    create_item(&mut engine, filters, &create_type("pack_vec3"));

    let sink = Parameter::new(
        "Vector Sink",
        ParamValue::Vec3(0.0, 0.0, 0.0),
        ParameterChangeCheck::ValueChange,
    );
    let sink_uuid = sink.node_data().meta.uuid;
    engine.add_node(sink.into(), None);
    engine.apply_edits().unwrap();
    create_parameter_output(&mut engine, outputs, sink_uuid);

    for _ in 0..3 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_warning(&engine, first_pack, FILTER_WARNING_ID, "Awaiting source schema");
    assert_warning(
        &engine,
        filters,
        CHAIN_WARNING_ID,
        "Mapping filter chain is not executable",
    );

    for (label, value) in [("X", 1.0), ("Y", 2.0)] {
        let source = source_param(&mut engine, label, value);
        let input = create_item(
            &mut engine,
            inputs,
            &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
        );
        set_config(
            &mut engine,
            input,
            "source",
            ParamValue::Reference(NodeReference::new(source)),
        );
    }
    for _ in 0..4 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    assert_warning(&engine, first_pack, FILTER_WARNING_ID, "Filter is incompatible");
    let snapshot = engine.process_tree_snapshot();
    let sink = snapshot.node_id_by_uuid(sink_uuid).unwrap();
    assert_eq!(
        snapshot.node(sink).unwrap().param_value,
        Some(ParamValue::Vec3(0.0, 0.0, 0.0)),
        "an invalid required stage must block command dispatch"
    );
    drop(snapshot);
    assert_eq!(
        engine.nodes.get(filters).unwrap().user_creatable_items(),
        catalog,
        "an invalid preceding stage must not change the creation catalog"
    );

    let z = source_param(&mut engine, "Z", 3.0);
    let third = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(
        &mut engine,
        third,
        "source",
        ParamValue::Reference(NodeReference::new(z)),
    );
    for _ in 0..6 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }

    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.node_id_by_uuid(first_pack_uuid), Some(first_pack));
    assert!(!has_warning(&snapshot, first_pack, FILTER_WARNING_ID));
    assert!(!has_warning(&snapshot, filters, CHAIN_WARNING_ID));
    let sink = snapshot.node_id_by_uuid(sink_uuid).unwrap();
    assert_eq!(
        snapshot.node(sink).unwrap().param_value,
        Some(ParamValue::Vec3(1.0, 2.0, 3.0))
    );
}

fn assert_warning(engine: &AppEngine, node: NodeId, id: &str, message: &str) {
    let snapshot = engine.process_tree_snapshot();
    let warning = snapshot
        .node(node)
        .unwrap()
        .presentation
        .warnings
        .iter()
        .find(|warning| warning.id == id)
        .unwrap_or_else(|| panic!("node {node:?} has no {id} warning"));
    assert_eq!(warning.message, message);
    assert!(warning.detail.as_ref().is_some_and(|detail| !detail.is_empty()));
}

fn has_warning(
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    node: NodeId,
    id: &str,
) -> bool {
    snapshot
        .node(node)
        .unwrap()
        .presentation
        .warnings
        .iter()
        .any(|warning| warning.id == id)
}
