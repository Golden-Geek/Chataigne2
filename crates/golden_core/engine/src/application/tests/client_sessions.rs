use super::control_failures::ControlTestNode;
use super::*;
use std::sync::mpsc;

fn runtime() -> ProductionRuntime<ControlTestNode> {
    ProductionRuntime::new(
        Engine::new(crate::node::Folder::new("root").into()),
        UiProjectFileSpec::default(),
    )
    .unwrap()
}

fn begin() -> UiEditIntent {
    UiEditIntent::BeginEdit {
        client_edit_id: "reused edit id".into(),
        label: None,
    }
}

fn label(node: NodeId, label: &str) -> UiEditIntent {
    UiEditIntent::PatchMeta {
        node,
        patch: crate::node::NodeMetaPatch {
            label: Some(label.into()),
            ..Default::default()
        }
        .into(),
    }
}

#[test]
fn handshake_keeps_physical_lifetime_and_finalizes_an_anonymous_edit() {
    let runtime = runtime();
    let root = runtime.call_engine(|engine| engine.root).unwrap();
    let physical = UiClientLease::new(None);
    runtime
        .submit_ui_client_transaction_batch(vec![begin(), label(root, "anonymous")], &physical, true)
        .unwrap()
        .wait()
        .unwrap();
    let named = physical.with_client_instance_id("browser");
    let batch = runtime
        .submit_ui_client_transaction_batch(vec![begin(), label(root, "named")], &named, true)
        .unwrap()
        .wait()
        .unwrap();
    assert!(batch.transactions.iter().all(|result| result.acknowledgement.success));
    assert!(runtime.history_state().unwrap().active_edit_session);
    physical.close();
    assert!(named.is_closed());
    let history = runtime.history_state().unwrap();
    assert!(!history.active_edit_session);
    assert_eq!(history.undo_len, 2);
}

#[test]
fn closed_connections_finalize_accepted_history_and_publish_the_released_session() {
    for id in [None, Some("browser")] {
        let runtime = runtime();
        let root = runtime.call_engine(|engine| engine.root).unwrap();
        let client = UiClientLease::new(id);
        let batch = runtime
            .submit_ui_client_transaction_batch(vec![begin(), label(root, "accepted")], &client, true)
            .unwrap()
            .wait()
            .unwrap();
        assert!(batch.transactions.iter().all(|result| result.acknowledgement.success));
        assert!(runtime.history_state().unwrap().active_edit_session);
        client.close();
        let history = runtime.history_state().unwrap();
        assert!(!history.active_edit_session);
        assert_eq!(
            history.undo_len, 1,
            "disconnect must retain accepted edits as undoable history"
        );
        assert!(
            !runtime
                .read_model()
                .snapshot_for_scope(UiSubscriptionScope::WholeGraph)
                .history
                .active_edit_session
        );
        assert!(
            runtime
                .apply_ui_transaction(UiEditIntent::Undo, None)
                .unwrap()
                .acknowledgement
                .success
        );
        assert_eq!(
            runtime
                .call_engine(move |engine| engine.nodes.get(root).unwrap().node_data().meta.label.clone())
                .unwrap(),
            "root"
        );
    }
}

#[test]
fn closed_queued_requests_cannot_edit_a_replacement_connection_with_reused_ids() {
    let runtime = runtime();
    let root = runtime.call_engine(|engine| engine.root).unwrap();
    let old = UiClientLease::new(Some("browser"));
    runtime
        .submit_ui_client_transaction_batch(vec![begin(), label(root, "first")], &old, true)
        .unwrap()
        .wait()
        .unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let active = runtime
        .inner
        .control
        .handle()
        .submit(move |_| {
            entered_tx.send(()).unwrap();
            let _ = release_rx.recv();
        })
        .unwrap();
    entered_rx.recv().unwrap();
    let stale = runtime
        .submit_ui_client_transaction_batch(vec![label(root, "stale")], &old, true)
        .unwrap();
    old.close();
    release_tx.send(()).unwrap();
    active.wait().unwrap();
    assert_eq!(
        stale.wait().unwrap_err().control_error().unwrap().kind(),
        ControlErrorKind::Cancelled
    );
    let replacement = UiClientLease::new(Some("browser"));
    let batch = runtime
        .submit_ui_client_transaction_batch(vec![begin(), label(root, "replacement")], &replacement, true)
        .unwrap()
        .wait()
        .unwrap();
    assert!(batch.transactions.iter().all(|result| result.acknowledgement.success));
    old.close();
    runtime.run_tick(Duration::from_millis(1)).unwrap();
    assert!(runtime.history_state().unwrap().active_edit_session);
    assert_eq!(
        runtime
            .call_engine(move |engine| engine.nodes.get(root).unwrap().node_data().meta.label.clone())
            .unwrap(),
        "replacement"
    );
    replacement.close();
    assert_eq!(runtime.history_state().unwrap().undo_len, 2);
}

#[test]
fn a_failed_tick_still_publishes_released_sessions_and_its_accepted_edit_prefix() {
    let runtime = runtime();
    let root = runtime.call_engine(|engine| engine.root).unwrap();
    let client = UiClientLease::new(Some("browser"));
    runtime
        .submit_ui_client_transaction_batch(vec![begin(), label(root, "accepted")], &client, true)
        .unwrap()
        .wait()
        .unwrap();
    runtime
        .call_engine(move |engine| {
            engine.edits.push(crate::edit::Edit::PatchMeta {
                node: root,
                patch: crate::node::NodeMetaPatch {
                    label: Some("tick prefix".into()),
                    ..Default::default()
                },
            });
            engine.edits.push(crate::edit::Edit::SetParam {
                node: root,
                value: ParamValue::Int(1),
                behaviour: Default::default(),
            });
        })
        .unwrap();
    client.close();
    assert!(runtime.run_tick(Duration::from_millis(1)).is_err());
    let snapshot = runtime.read_model().snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    assert!(!snapshot.history.active_edit_session);
    assert_eq!(snapshot.history.undo_len, 1);
    assert!(serde_json::to_string(&snapshot).unwrap().contains("tick prefix"));
    runtime.run_tick(Duration::from_millis(1)).unwrap();
}

#[test]
fn project_replacement_discards_old_lifetime_ownership_even_when_session_ids_are_reused() {
    let runtime = runtime();
    let old = UiClientLease::new(Some("browser"));
    runtime
        .submit_ui_client_transaction_batch(vec![begin()], &old, true)
        .unwrap()
        .wait()
        .unwrap();
    runtime
        .replace_project(ProjectReplacement {
            engine: Engine::new(crate::node::Folder::new("replacement project").into()),
            project_file: UiProjectFileSpec::default(),
            reason: "test replacement".into(),
            recover: false,
        })
        .unwrap();
    let replacement = UiClientLease::new(Some("browser"));
    assert!(
        runtime
            .submit_ui_client_transaction_batch(vec![begin()], &replacement, true)
            .unwrap()
            .wait()
            .unwrap()
            .transactions[0]
            .acknowledgement
            .success
    );
    old.close();
    assert!(runtime.history_state().unwrap().active_edit_session);
    replacement.close();
    assert!(!runtime.history_state().unwrap().active_edit_session);
}
