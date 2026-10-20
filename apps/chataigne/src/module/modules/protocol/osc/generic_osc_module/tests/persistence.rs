use super::*;

#[test]
fn sparse_project_serialization_omits_unchanged_osc_defaults() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ModuleManager::new().into(), None);
    engine.apply_edits().expect("module manager should attach");

    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should be attached under root");
    engine.add_user_item(GenericOscModule::create().into(), Some(manager_id));
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let round_tripped = golden_core::app::to_sparse_project_json_pretty(
        &golden_core::app::from_sparse_project_json::<AppNode>(&json).expect("sparse project should decode"),
    )
    .expect("round-tripped sparse project should encode");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&round_tripped).expect("round-tripped project json should parse"),
        serde_json::from_str::<serde_json::Value>(&json).expect("project json should parse"),
        "saving, reopening, then saving again should not add declared default noise"
    );

    let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
    let module_record = value
        .get("root")
        .and_then(|root| root.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .and_then(|manager| manager.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .expect("osc module record should be saved under the module manager");

    assert_eq!(
        module_record.get("type"),
        Some(&serde_json::Value::String("osc_module".to_string()))
    );
    assert!(
        module_record.get("children").is_none(),
        "unchanged OSC connection, parameter, output, command tester, and values defaults should be recreated, not saved"
    );

    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).expect("sparse project should decode");
    let loaded_manager = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should reload");
    let loaded_module = loaded
        .nodes
        .get(loaded_manager)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should reload");

    assert!(
        find_path(&loaded, loaded_module, "connection/connected").is_some(),
        "omitted connection defaults should be restored"
    );
    let loaded_outputs = find_path(&loaded, loaded_module, "connection/outputs")
        .expect("omitted output manager default should be restored");
    let loaded_default_output = loaded
        .nodes
        .get(loaded_outputs)
        .and_then(|outputs| outputs.node_data().first_child)
        .expect("omitted default OSC output should be restored");
    assert_eq!(
        loaded
            .nodes
            .get(loaded_default_output)
            .expect("restored default OSC output should exist")
            .get_type(),
        "osc_output"
    );
    let loaded_command_tester =
        find_path(&loaded, loaded_module, "command_tester").expect("omitted command tester default should be restored");
    let loaded_command_types = loaded
        .nodes
        .get(loaded_command_tester)
        .expect("restored command tester should exist")
        .user_creatable_items()
        .into_iter()
        .map(|item| item.node_type)
        .collect::<Vec<_>>();
    assert_eq!(
        loaded_command_types,
        vec![crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE.to_string()],
        "restored OSC command tester should retain its module-owned command catalog"
    );
    assert!(
        find_path(&loaded, loaded_module, "values").is_some(),
        "omitted values folder should be restored"
    );
}

#[test]
fn sparse_project_serialization_saves_only_changed_declared_osc_port_delta() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ModuleManager::new().into(), None);
    engine.apply_edits().expect("module manager should attach");

    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should be attached under root");
    engine.add_user_item(GenericOscModule::create().into(), Some(manager_id));
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(manager_id)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should be attached under manager");
    let port_id = find_path(&engine, module_id, "connection/input/port").expect("receiver port param should exist");
    set_param(&mut engine, port_id, ParamValue::Int(9001));
    engine.apply_edits().expect("receiver port edit should apply");

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let round_tripped = golden_core::app::to_sparse_project_json_pretty(
        &golden_core::app::from_sparse_project_json::<AppNode>(&json).expect("sparse project should decode"),
    )
    .expect("round-tripped sparse project should encode");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&round_tripped).expect("round-tripped project json should parse"),
        serde_json::from_str::<serde_json::Value>(&json).expect("project json should parse"),
        "saving, reopening, then saving again should not add declared default noise"
    );

    let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
    let module_record = value
        .get("root")
        .and_then(|root| root.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .and_then(|manager| manager.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .expect("osc module record should be saved under the module manager");
    let connection_record = json_child_by_decl(module_record, "connection");
    let input_record = json_child_by_decl(connection_record, "connection/input");
    let port_record = json_child_by_decl(input_record, "connection/input/port");

    assert_eq!(
        connection_record.get("meta"),
        Some(&serde_json::json!({ "decl_id": "connection" })),
        "declared ancestor folders should only carry their declaration path"
    );
    assert_eq!(
        input_record.get("meta"),
        Some(&serde_json::json!({ "decl_id": "connection/input" })),
        "declared receiver folder defaults should stay app-owned"
    );
    assert_eq!(
        port_record.get("meta"),
        Some(&serde_json::json!({ "decl_id": "connection/input/port" })),
        "declared port label and static metadata should stay app-owned"
    );
    assert_eq!(
        port_record.get("data"),
        Some(&serde_json::json!({ "value": { "Int": 9001 } })),
        "only the changed receiver port value should be persisted"
    );

    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).expect("sparse project should decode");
    let loaded_manager = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should reload");
    let loaded_module = loaded
        .nodes
        .get(loaded_manager)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should reload");
    let loaded_port = find_path(&loaded, loaded_module, "connection/input/port").expect("receiver port should reload");
    let loaded_port_node = loaded.nodes.get(loaded_port).expect("receiver port node should exist");
    let snapshot = loaded_port_node
        .engine_param_snapshot()
        .expect("receiver port should remain a parameter");

    assert_eq!(loaded_port_node.node_data().meta.label, "Port");
    assert_eq!(snapshot.value, ParamValue::Int(9001));
    assert_eq!(snapshot.default_value, ParamValue::Int(9000));
    assert_eq!(snapshot.ui_hints.widget.as_deref(), Some("text"));
    assert!(matches!(
        snapshot.constraints.range,
        Some(RangeConstraint::Uniform {
            min: Some(0.0),
            max: Some(65535.0),
        })
    ));
}

#[test]
fn sparse_project_round_trip_preserves_saved_osc_command_tester_children() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ModuleManager::new().into(), None);
    engine.apply_edits().expect("module manager should attach");

    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should be attached under root");
    engine.add_user_item(GenericOscModule::create().into(), Some(manager_id));
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(manager_id)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should be attached under manager");
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    let command_id = create_send_custom_message_command(&mut engine, command_tester_id);
    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    set_param(&mut engine, address_param, ParamValue::Str("/save/reopen".to_string()));
    engine.apply_edits().expect("command edit should apply");

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("sparse project with saved osc commands should decode");

    let loaded_manager = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should reload");
    let loaded_module = loaded
        .nodes
        .get(loaded_manager)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should reload");
    let loaded_command_tester =
        find_path(&loaded, loaded_module, "command_tester").expect("command tester should reload");
    let loaded_command = loaded
        .nodes
        .get(loaded_command_tester)
        .and_then(|node| node.node_data().first_child)
        .expect("saved osc command should reload under the command tester");
    let loaded_address = find_path(&loaded, loaded_command, "address").expect("saved command address should reload");

    assert_eq!(
        loaded.nodes.get(loaded_command).map(|node| node.get_type()),
        Some(crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE),
        "saved osc command should keep its node type through project round-trip"
    );
    assert_eq!(
        loaded
            .nodes
            .get(loaded_address)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Str("/save/reopen".to_string())),
        "saved osc command parameters should survive project round-trip"
    );
}
