use super::*;

#[test]
fn connection_status_values_and_processing_edits_do_not_cancel_configuration() {
    let (mut engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let (request, job) = draft(&snapshot, id, &slots());
    let connected = find_path(&engine, id, "connection/connected").unwrap();
    set(&mut engine, connected, ParamValue::Bool(true));
    let processing = find_path(&engine, id, "parameters/processing/auto_add").unwrap();
    set(&mut engine, processing, ParamValue::Bool(false));
    let values = find_path(&engine, id, "values").unwrap();
    engine.add_node(Folder::new("unrelated traffic").into(), Some(values));
    engine.apply_edits().unwrap();
    assert!(request.matches(&engine.process_tree_snapshot(), id));
    job.run();
    assert!(request.is_complete());
}

#[test]
fn authored_fields_and_subscription_structure_invalidate_results_before_callbacks() {
    let (mut engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let (mut request, job) = draft(&snapshot, id, &slots());
    job.run();
    assert!(request.poll().unwrap().is_some());
    let host = find_path(&engine, id, "connection/remote_host").unwrap();
    set(&mut engine, host, ParamValue::Str("localhost".into()));
    assert!(!request.matches(&engine.process_tree_snapshot(), id));
    engine.add_node(super::super::super::MqttSubscriptionManager::new().into(), None);
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let (request, job) = draft(&snapshot, id, &slots());
    let manager = find_path(&engine, id, "connection/subscriptions").unwrap();
    let child = snapshot.child_ids_slice(manager)[0];
    let destination = snapshot
        .child_ids_slice(snapshot.root())
        .iter()
        .copied()
        .find(|child| snapshot.node(*child).unwrap().node_type == "mqtt_subscription_manager")
        .unwrap();
    engine.edits.push(Edit::MoveNode {
        node: child,
        new_parent: destination,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    assert!(!request.matches(&engine.process_tree_snapshot(), id));
    drop(request);
    job.run();
}

#[test]
fn disabled_and_replaced_subscription_sources_invalidate_capture() {
    let (mut engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let (request, job) = draft(&snapshot, id, &slots());
    let manager = find_path(&engine, id, "connection/subscriptions").unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: manager,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(!request.matches(&engine.process_tree_snapshot(), id));
    drop(request);
    job.run();
    let snapshot = engine.process_tree_snapshot();
    let (request, job) = draft(&snapshot, id, &slots());
    let child = snapshot.child_ids_slice(manager)[0];
    engine.replace_node(child, MqttSubscription::new().into());
    engine.apply_edits().unwrap();
    assert!(!request.matches(&engine.process_tree_snapshot(), id));
    drop(request);
    job.run();
}

#[test]
fn inbox_detects_structural_configuration_edits_without_ancestor_walks() {
    let (mut engine, id) = create_mqtt_module();
    let manager = find_path(&engine, id, "connection/subscriptions").unwrap();
    engine.add_node(MqttSubscription::new().into(), Some(manager));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    let snapshot = engine.process_tree_snapshot();
    let node = module(&mut engine, id);
    assert!(node.transport_dirty);
    assert_eq!(node.configuration_key, Key::capture(&snapshot, id));
    assert!(node.worker_wake_requires_tree_snapshot());
    node.disable_transport_for_test(&snapshot);
}

#[test]
fn ready_result_cannot_start_a_transport_after_the_authored_source_changed() {
    let (mut engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let slots = slots();
    let (request, job) = draft(&snapshot, id, &slots);
    job.run();
    module(&mut engine, id).configuration = Some(request);
    module(&mut engine, id).transport_dirty = true;
    let host = find_path(&engine, id, "connection/remote_host").unwrap();
    set(&mut engine, host, ParamValue::Str("x".repeat(1025)));
    let current = engine.process_tree_snapshot();
    let time = engine.time;
    let node = module(&mut engine, id);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, time);
    node.refresh_transport(&mut ctx, &current);
    assert!(node.transport.is_none());
    assert!(node.last_transport_config.is_none());
    assert!(node
        .configuration
        .as_ref()
        .is_none_or(|request| request.matches(&current, id)));
    assert_eq!(slots.metrics().active, 0);
    node.disable_transport_for_test(&current);
}
