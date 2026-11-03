use super::*;

#[test]
fn query_success_null_and_rejection_preserve_redo_events_and_document_identity() {
    let mut engine = engine();
    assert!(apply_project_intent(&mut engine, intent("create", json!("undone"))).success);
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    let history = engine.ui_history_state();
    let cursor = engine.ui_event_log().last().map(|event| event.time);
    let root = engine.nodes.get(engine.root).unwrap().node_data().meta.uuid;
    for payload in [Value::Null, json!("null"), json!("reject")] {
        let ack = apply_project_intent(&mut engine, intent("inspect", payload.clone()));
        assert_eq!(ack.success, payload != json!("reject"));
        assert_eq!(ack.history, history);
        assert_eq!((ack.earliest_event_time, ack.latest_event_time), (None, None));
        if payload == json!("null") {
            assert_eq!(ack.result.unwrap().value, Value::Null);
        } else if payload == json!("reject") {
            assert!(ack.result.is_none());
            assert_eq!(ack.error_code.as_deref(), Some("app_operation_failed"));
        } else {
            assert_eq!(ack.result.unwrap().value, json!({"nodes":1,"active":false}));
        }
        assert_eq!(engine.nodes.len(), 1, "query rejection cannot fall through to creation");
        assert_eq!(engine.nodes.get(engine.root).unwrap().node_data().meta.uuid, root);
        assert_eq!(engine.ui_event_log().last().map(|event| event.time), cursor);
    }
    assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
    assert_eq!(engine.nodes.len(), 2);
}

#[test]
fn queries_read_pending_edits_and_keep_an_explicit_group_open() {
    let mut engine = engine();
    engine.add_node(Folder::new("queued").into(), None);
    let read = apply_project_intent(&mut engine, intent("inspect", Value::Null));
    assert!(read.success, "{read:?}");
    assert_eq!(read.result.unwrap().value["nodes"], 2);
    assert!(engine.edits.pending.is_empty());
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::BeginEdit {
                client_edit_id: "group".into(),
                label: None,
            }
        )
        .success
    );
    assert!(apply_project_intent(&mut engine, intent("create", json!("prefix"))).success);
    let history = engine.ui_history_state();
    let read = apply_project_intent(&mut engine, intent("inspect", Value::Null));
    assert!(read.success);
    assert_eq!(read.result.unwrap().value, json!({"nodes":3,"active":true}));
    assert_eq!(read.history, history);
    let rejected = apply_project_intent(&mut engine, intent("inspect", json!("reject")));
    assert!(!rejected.success);
    assert_eq!(rejected.history, history);
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::EndEdit {
                client_edit_id: "group".into(),
            }
        )
        .success
    );
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    assert_eq!(engine.nodes.len(), 2, "one Undo removes the group's accepted prefix");
}

#[test]
fn actor_queries_publish_no_authored_delta_and_share_the_headless_contract() {
    let runtime = ProductionRuntime::new(engine(), UiProjectFileSpec::default()).unwrap();
    let read = runtime
        .apply_ui_transaction(intent("inspect", Value::Null), Some("reader"))
        .unwrap();
    assert!(read.acknowledgement.success);
    assert_eq!(
        read.acknowledgement.result.unwrap().value,
        json!({"nodes":1,"active":false})
    );
    assert!(read.events.events.is_empty());
    let rejected = runtime
        .apply_ui_transaction(intent("inspect", json!("reject")), Some("reader"))
        .unwrap();
    assert!(!rejected.acknowledgement.success);
    assert!(rejected.acknowledgement.result.is_none());
    assert!(rejected.events.events.is_empty());
    assert_eq!(runtime.call_engine(|engine| engine.undo_len()).unwrap(), 0);
}

#[test]
fn failed_pending_edits_reject_before_query_execution() {
    let mut engine = engine();
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: crate::parameter::ParameterEventBehaviour::Append,
    });
    let read = apply_project_intent(&mut engine, intent("inspect", Value::Null));
    assert!(!read.success);
    assert!(read.result.is_none());
    assert_eq!(engine.nodes.len(), 1);
}
