use super::*;

#[test]
fn incoming_message_auto_adds_value_nodes_under_values_folder() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get_mut(module_id).expect("module should exist")
    else {
        panic!("expected GenericOscModule node");
    };
    module.disable_transport_for_test();
    module.enqueue_incoming_message_for_test(OscDecodedMessage {
        address: "/foo".to_string(),
        payload: OscValuePayload::Single(ParamValue::Int(42)),
    });

    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should process queued incoming OSC message");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get(module_id).expect("module should still exist")
    else {
        panic!("expected GenericOscModule node");
    };
    assert!(
        module.auto_add_enabled_for_test(),
        "auto-add should remain enabled by default"
    );
    assert!(
        !module.has_pending_incoming_messages_for_test(),
        "incoming message queue should be drained after runtime tick"
    );

    let values_folder = find_path(&engine, module_id, "values").expect("module should contain a Values folder");
    let value_child_labels = child_labels(&engine, values_folder);
    let values_param = find_child_by_key(&engine, values_folder, "foo").unwrap_or_else(|| {
        panic!("incoming OSC address should auto-create a parameter under Values; children were {value_child_labels:?}")
    });
    let values_node = engine
        .nodes
        .get(values_param)
        .expect("auto-created value node should exist");

    assert_eq!(values_node.node_data().meta.label, "foo");
    assert_eq!(
        values_node.node_data().meta.user_permissions,
        NodeUserPermissions::all()
    );
    assert_eq!(
        values_node.engine_param_snapshot().map(|snapshot| snapshot.value),
        Some(ParamValue::Int(42))
    );
}

#[test]
fn incoming_multi_message_auto_adds_missing_path_with_batched_trees() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get_mut(module_id).expect("module should exist")
    else {
        panic!("expected GenericOscModule node");
    };
    module.disable_transport_for_test();
    module.enqueue_incoming_message_for_test(OscDecodedMessage {
        address: "/rig/arm/pose".to_string(),
        payload: OscValuePayload::Multi(vec![
            ParamValue::Float(1.0),
            ParamValue::Float(2.0),
            ParamValue::Float(3.0),
        ]),
    });

    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should materialize missing parent path and multi-value leaf subtree");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get(module_id).expect("module should still exist")
    else {
        panic!("expected GenericOscModule node");
    };
    assert!(
        !module.has_pending_incoming_messages_for_test(),
        "batched missing path and multi-value leaf creation should drain the incoming queue"
    );

    let first_value =
        find_path(&engine, module_id, "values/rig/arm/pose/value 1").expect("first multi-value parameter should exist");
    let second_value = find_path(&engine, module_id, "values/rig/arm/pose/value 2")
        .expect("second multi-value parameter should exist");
    let third_value =
        find_path(&engine, module_id, "values/rig/arm/pose/value 3").expect("third multi-value parameter should exist");

    assert_eq!(param_value(&engine, first_value), ParamValue::Float(1.0));
    assert_eq!(param_value(&engine, second_value), ParamValue::Float(2.0));
    assert_eq!(param_value(&engine, third_value), ParamValue::Float(3.0));
}

#[test]
fn incoming_messages_auto_add_shared_missing_path_in_one_tick() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get_mut(module_id).expect("module should exist")
    else {
        panic!("expected GenericOscModule node");
    };
    module.disable_transport_for_test();
    module.enqueue_incoming_message_for_test(OscDecodedMessage {
        address: "/rig/arm/x".to_string(),
        payload: OscValuePayload::Single(ParamValue::Float(1.25)),
    });
    module.enqueue_incoming_message_for_test(OscDecodedMessage {
        address: "/rig/arm/y".to_string(),
        payload: OscValuePayload::Single(ParamValue::Float(2.5)),
    });

    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should materialize queued shared OSC path values");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get(module_id).expect("module should still exist")
    else {
        panic!("expected GenericOscModule node");
    };
    assert!(
        !module.has_pending_incoming_messages_for_test(),
        "batched shared missing path creation should drain the incoming queue"
    );

    let x_value = find_path(&engine, module_id, "values/rig/arm/x").expect("x parameter should exist");
    let y_value = find_path(&engine, module_id, "values/rig/arm/y").expect("y parameter should exist");

    assert_eq!(param_value(&engine, x_value), ParamValue::Float(1.25));
    assert_eq!(param_value(&engine, y_value), ParamValue::Float(2.5));
}
