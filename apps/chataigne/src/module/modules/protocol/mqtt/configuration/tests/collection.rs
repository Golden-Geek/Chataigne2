use super::*;

#[test]
fn fresh_defaults_and_deliberately_empty_loaded_subscriptions_preserve_authored_meaning() {
    let (mut engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let config = build(&snapshot, id).unwrap();
    assert_eq!(config.client_id, format!("chataigne2-{}", id.0));
    assert_eq!(config.subscriptions.len(), 1);
    assert_eq!(config.subscriptions[0].topic_filter, "#");
    let manager = find_path(&engine, id, "connection/subscriptions").unwrap();
    let default = snapshot.child_ids_slice(manager)[0];
    engine.edits.push(Edit::RemoveNode { node: default });
    engine.apply_edits().unwrap();
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    for _ in 0..4 {
        loaded.apply_edits().unwrap();
    }
    let loaded_id = loaded.nodes.get(loaded.root).unwrap().node_data().first_child.unwrap();
    assert!(build(&loaded.process_tree_snapshot(), loaded_id)
        .unwrap()
        .subscriptions
        .is_empty());
}

#[test]
fn fields_are_bounded_before_trim_or_copy_and_disabled_credentials_are_not_copied() {
    let (mut engine, id) = create_mqtt_module();
    let credentials = find_path(&engine, id, "connection/credentials").unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: credentials,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    let username = find_path(&engine, id, "connection/credentials/username").unwrap();
    set(&mut engine, username, ParamValue::Str(" user ".into()));
    let password = find_path(&engine, id, "connection/credentials/password").unwrap();
    set(&mut engine, password, ParamValue::Str(" keep spaces ".into()));
    let snapshot = engine.process_tree_snapshot();
    let config = build(&snapshot, id).unwrap();
    let auth = config.credentials.unwrap();
    assert_eq!(auth.username, "user");
    assert_eq!(auth.password, " keep spaces ");
    let manager = find_path(&engine, id, "connection/subscriptions").unwrap();
    let subscription = snapshot.child_ids_slice(manager)[0];
    let filter = snapshot.find_child_by_decl_id(subscription, "topic_filter").unwrap();
    for (field, maximum) in [
        (find_path(&engine, id, "connection/remote_host").unwrap(), 1024),
        (find_path(&engine, id, "connection/client_id").unwrap(), 256),
        (username, 4096),
        (password, 4096),
        (filter, 1024),
    ] {
        let oversized = snapshot.with_param_values([(field, ParamValue::Str(" ".repeat(maximum + 1)))]);
        assert!(!build(&oversized, id).unwrap_err().retryable);
    }
    let keepalive = find_path(&engine, id, "connection/keep_alive_secs").unwrap();
    assert!(build(&snapshot.with_param_values([(keepalive, ParamValue::Int(65536))]), id).is_err());
    engine.edits.push(Edit::PatchMeta {
        node: credentials,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    let snapshot = engine
        .process_tree_snapshot()
        .with_param_values([(password, ParamValue::Str("x".repeat(8192)))]);
    assert!(build(&snapshot, id).unwrap().credentials.is_none());
}

#[test]
fn subscription_count_order_and_folder_disable_are_checked_on_the_worker() {
    let (mut engine, id) = create_mqtt_module();
    let manager = find_path(&engine, id, "connection/subscriptions").unwrap();
    let default = engine.process_tree_snapshot().child_ids_slice(manager)[0];
    engine.edits.push(Edit::RemoveNode { node: default });
    let mut folder = NodeTree::new(Folder::new("topics"));
    for _ in 0..128 {
        folder.push_child(NodeTree::new(MqttSubscription::new()));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree: folder,
        parent: manager,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let folder = find_path(&engine, id, "connection/subscriptions/topics").unwrap();
    let snapshot = engine.process_tree_snapshot();
    let mut values = Vec::new();
    for (index, child) in snapshot.child_ids_slice(folder).iter().enumerate() {
        let filter = snapshot.find_child_by_decl_id(*child, "topic_filter").unwrap();
        values.push((filter, ParamValue::Str(format!("topics/{index}"))));
    }
    let snapshot = snapshot.with_param_values(values);
    let config = build(&snapshot, id).unwrap();
    assert_eq!(config.subscriptions.len(), 128);
    assert_eq!(config.subscriptions[0].topic_filter, "topics/0");
    assert_eq!(config.subscriptions[127].topic_filter, "topics/127");
    engine.add_node(MqttSubscription::new().into(), Some(folder));
    engine.apply_edits().unwrap();
    assert!(build(&engine.process_tree_snapshot(), id)
        .unwrap_err()
        .message
        .contains("128"));
    engine.edits.push(Edit::PatchMeta {
        node: folder,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(build(&engine.process_tree_snapshot(), id)
        .unwrap()
        .subscriptions
        .is_empty());
}

#[test]
fn subscription_frontier_is_bounded_even_when_there_are_no_active_filters() {
    let (engine, id) = create_mqtt_module();
    let snapshot = engine.process_tree_snapshot();
    let manager = find_path(&engine, id, "connection/subscriptions").unwrap();
    let mut nodes = std::collections::HashMap::new();
    let mut pending = vec![snapshot.root()];
    while let Some(node) = pending.pop() {
        nodes.insert(node, snapshot.node(node).unwrap().clone());
        pending.extend_from_slice(snapshot.child_ids_slice(node));
    }
    let mut template = nodes[&manager].clone();
    template.node_type = "folder".into();
    template.first_child = None;
    template.child_count = 0;
    template.parent = Some(manager);
    for index in 0..=MAX_SCAN {
        let child = NodeId(1_000_000 + index as u64);
        let mut record = template.clone();
        record.id = child;
        record.next_sibling = (index < MAX_SCAN).then_some(NodeId(child.0 + 1));
        nodes.insert(child, record);
    }
    nodes.get_mut(&manager).unwrap().first_child = Some(NodeId(1_000_000));
    nodes.get_mut(&manager).unwrap().child_count = MAX_SCAN + 1;
    let snapshot = ProcessTreeSnapshot::new(snapshot.root(), nodes);
    assert!(build(&snapshot, id).unwrap_err().message.contains("8192"));
}
