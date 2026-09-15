use super::*;

#[test]
fn mapping_correction_baseline_records_the_current_shadow_authoring_boundary() {
    let (mut engine, _, processor) = mapping_engine();
    let snapshot = engine.process_tree_snapshot();
    let managed_root = snapshot
        .find_child_by_decl_id(processor, PROCESSOR_MANAGED_REGIONS_DECL_ID)
        .expect("Mapping should have a managed-region root");
    assert!(
        !snapshot
            .node(managed_root)
            .expect("managed-region root should exist")
            .presentation
            .show_in_inspector_content,
        "the correction baseline intentionally records the hidden authoring root"
    );

    let visible_inputs = snapshot
        .child_ids(processor)
        .into_iter()
        .find(|child| {
            snapshot
                .node(*child)
                .is_some_and(|node| node.node_type == crate::app::InputsManager::NODE_TYPE)
        })
        .expect("Mapping should expose an Inputs surface manager");
    let hidden_inputs = region(&engine, processor, "inputs");
    assert_ne!(visible_inputs, hidden_inputs);

    let authored_input = create_item(
        &mut engine,
        hidden_inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.child_ids(hidden_inputs).contains(&authored_input));
    assert!(
        snapshot.child_ids(visible_inputs).is_empty(),
        "the visible surface manager is currently separate from authoritative managed items"
    );
}

#[test]
fn mapping_correction_baseline_preserves_tuple_reduction_and_queued_delivery() {
    let (mut engine, _, processor) = mapping_engine();
    activate_mapping_processor(&mut engine, processor);
    let sink = source_param(&mut engine, "Tuple result", 0.0);
    let inputs = region(&engine, processor, "inputs");
    for (label, value) in [("X", 2.0), ("Y", 3.0), ("Z", 5.0)] {
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

    let filters = region(&engine, processor, "filters");
    let sum_type = engine
        .nodes
        .get(filters)
        .expect("Filters region should exist")
        .user_creatable_items()
        .into_iter()
        .find(|item| item.node_type.starts_with(&format!("{ANODE_CREATE_PREFIX}sum")))
        .map(|item| item.node_type)
        .expect("three Float inputs should expose Sum");
    create_item(&mut engine, filters, &sum_type);

    let outputs = region(&engine, processor, "outputs");
    let output = create_item(
        &mut engine,
        outputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.output_target"),
    );
    set_config(
        &mut engine,
        output,
        "target",
        ParamValue::Reference(NodeReference::new(sink)),
    );

    for _ in 0..12 {
        engine.run_tick(Duration::from_millis(8)).unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let sink = snapshot.node_id_by_uuid(sink).expect("sink should remain addressable");
    assert_eq!(snapshot.node(sink).unwrap().param_value, Some(ParamValue::Float(10.0)));
}
