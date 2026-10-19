use super::*;

use golden_application::ProjectTransactions;

#[test]
fn public_graph_editing_returns_the_authoritative_rejection() {
    let runtime = runtime();
    let before = runtime.history_state().unwrap();
    let root = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .expect("runtime snapshot")
        .nodes[0]
        .node_id;

    let error = runtime
        .apply_graph_edit(UiEditIntent::RemoveNode { node: root })
        .expect_err("removing the root must be rejected by the public graph facade");

    assert_eq!(error.graph_rejection().unwrap().code(), Some("cannot_mutate_root"));
    assert!(
        error
            .graph_rejection()
            .unwrap()
            .message()
            .is_some_and(|message| message.contains("root"))
    );
    assert_eq!(error.graph_rejection().unwrap().history(), &before);
    assert!(!error.graph_rejection().unwrap().acknowledgement().success);
    assert_eq!(
        error.graph_rejection().unwrap().acknowledgement().status,
        crate::ui_sync::UiAckStatus::Rejected
    );
    assert_eq!(runtime.history_state().unwrap(), before);
    assert_eq!(
        runtime
            .snapshot(UiSubscriptionScope::WholeGraph)
            .expect("post-rejection snapshot")
            .nodes
            .len(),
        1
    );
}

#[test]
fn transaction_undo_and_redo_report_unavailable_as_typed_errors() {
    let runtime = runtime();

    let ui_undo = runtime
        .apply_ui_transaction(UiEditIntent::Undo, Some("ui-consumer"))
        .unwrap();
    assert!(!ui_undo.acknowledgement.success);
    assert_eq!(ui_undo.acknowledgement.error_code.as_deref(), Some("undo_unavailable"));

    let undo = ProjectTransactions::undo(&runtime).expect_err("empty history cannot be undone");
    assert_eq!(undo.graph_rejection().unwrap().code(), Some("undo_unavailable"));
    assert_eq!(
        undo.graph_rejection().unwrap().history(),
        &runtime.history_state().unwrap()
    );

    let redo = ProjectTransactions::redo(&runtime).expect_err("empty redo history cannot be reapplied");
    assert_eq!(redo.graph_rejection().unwrap().code(), Some("redo_unavailable"));
    assert_eq!(
        redo.graph_rejection().unwrap().history(),
        &runtime.history_state().unwrap()
    );
}

#[test]
fn successful_facades_return_the_acknowledged_actor_revision() {
    let runtime = runtime();

    let revision = runtime
        .apply_graph_edit(UiEditIntent::ReevaluateGraph)
        .expect("graph edit should succeed");
    assert_eq!(revision, runtime.history_state().unwrap());

    let acknowledgement = ProjectTransactions::apply_transaction(&runtime, UiEditIntent::ReevaluateGraph)
        .expect("project transaction should succeed");
    assert!(acknowledgement.success);
    assert_eq!(acknowledgement.history, runtime.history_state().unwrap());
}

#[test]
fn rejected_compound_intents_preserve_production_projection_dirty_revision_and_next_save() {
    let (mut engine, container, _) = atomic_duplicate_engine();
    engine.add_node(LifecycleMutationFailure::new().into(), None);
    engine.apply_edits().unwrap();
    let source = engine
        .nodes
        .iter()
        .find_map(|(id, node)| (node.get_type() == "lifecycle_mutation_failure").then_some(id))
        .unwrap();
    let root = engine.root;
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(FacadeTestNode::project_file_spec(), None),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("atomic-creation.noisette");
    let save = || crate::application::ProjectSaveRequest {
        path: path.to_string_lossy().into_owned(),
        ui_state: None,
    };
    runtime.save_project(save()).unwrap();
    let authored_before: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let before = runtime.snapshot(UiSubscriptionScope::WholeGraph).unwrap();
    let history = runtime.history_state().unwrap();
    let persistence = runtime.project_persistence_status().unwrap();

    let intents = [
        UiEditIntent::CreateUserItem {
            parent: container,
            node_type: "float".into(),
            label: Some("Rejected".into()),
            initial_params: vec![UiCreateUserItemInitialParam {
                decl_id: DeclId("missing".into()),
                value: ParamValue::Float(2.0),
            }],
        },
        UiEditIntent::DuplicateNode {
            source,
            new_parent: root,
            new_prev_sibling: None,
            initial_params: Vec::new(),
        },
        UiEditIntent::SendNodeEvent {
            node: source,
            topic: "reject_authored_event".into(),
            payload: serde_json::json!({}),
        },
    ];
    for intent in intents {
        let rejected = runtime
            .apply_ui_transaction(intent, Some("atomic-publication-test"))
            .unwrap();
        assert!(!rejected.acknowledgement.success);
        assert!(!rejected.preferences_changed);
        let after = runtime.snapshot(UiSubscriptionScope::WholeGraph).unwrap();
        assert_eq!(
            serde_json::to_value(after.nodes).unwrap(),
            serde_json::to_value(&before.nodes).unwrap()
        );
        assert_eq!(runtime.history_state().unwrap(), history);
        let after_persistence = runtime.project_persistence_status().unwrap();
        assert_eq!(after_persistence.document_revision, persistence.document_revision);
        assert_eq!(
            after_persistence.saved_document_revision,
            persistence.saved_document_revision
        );
        assert_eq!(after_persistence.dirty, persistence.dirty);
        assert!(!runtime.save_project(save()).unwrap().dirty);
        let authored_after: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(authored_after, authored_before);
    }
}
