use super::*;

#[test]
fn managed_region_metadata_and_anchors_remain_formula_owned() {
    let (mut engine, formula) = fixture();
    let node = anode(&mut engine, &formula, "constant");
    let metadata = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(formula.node, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
        .unwrap();
    let definition = chataigne_alchemist::ManagedRegionDefinition {
        id: chataigne_alchemist::ManagedRegionId::new("inputs"),
        kind: chataigne_alchemist::ManagedRegionKind::InputSet,
        label: "Inputs".into(),
        input_socket: None,
        output_socket: Some(chataigne_alchemist::ManagedSocketRef::new(
            ANodeId::from_uuid(engine.nodes.get(node).unwrap().node_data().meta.uuid.0),
            "value",
        )),
        accepted_roles: vec![SurfaceItemKind::Input],
        filter_value_mode: Default::default(),
    };
    let raw = serde_json::to_string(&vec![definition]).unwrap();
    engine.edits.push(Edit::SetParam {
        node: metadata,
        value: ParamValue::Str(raw.clone()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    let fragment = copy(&mut engine, &formula, vec![node]);
    let result: FormulaPasteResult = invoke(
        &mut engine,
        FORMULA_PASTE_OPERATION,
        FormulaPasteRequest {
            formula: formula.clone(),
            fragment,
            offset: None,
        },
    )
    .unwrap();
    assert_eq!(result.selected_nodes.len(), 1);
    assert_eq!(
        engine
            .nodes
            .get(metadata)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Str(raw)
    );
    let definitions = formula_managed_regions_from_snapshot(&engine.process_tree_snapshot(), formula.node).unwrap();
    assert_eq!(
        definitions[0].output_socket.as_ref().unwrap().node.as_uuid(),
        engine.nodes.get(node).unwrap().node_data().meta.uuid.0
    );
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    assert!(engine.nodes.get(node).is_some());
    assert!(engine.nodes.get(result.selected_nodes[0]).is_none());
}
