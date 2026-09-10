use super::*;

use golden_application::ProjectTransactions;

#[test]
fn public_graph_editing_returns_the_authoritative_rejection() {
    let runtime = runtime();
    let before = runtime.history_state();
    let root = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .expect("runtime snapshot")
        .nodes[0]
        .node_id;

    let error = runtime
        .apply_graph_edit(UiEditIntent::RemoveNode { node: root })
        .expect_err("removing the root must be rejected by the public graph facade");

    assert_eq!(error.code(), Some("cannot_mutate_root"));
    assert!(error.message().is_some_and(|message| message.contains("root")));
    assert_eq!(error.history(), &before);
    assert!(!error.acknowledgement().success);
    assert_eq!(error.acknowledgement().status, crate::ui_sync::UiAckStatus::Rejected);
    assert_eq!(runtime.history_state(), before);
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

    let ui_undo = runtime.apply_ui_transaction(UiEditIntent::Undo, Some("ui-consumer"));
    assert!(!ui_undo.acknowledgement.success);
    assert_eq!(ui_undo.acknowledgement.error_code.as_deref(), Some("undo_unavailable"));

    let undo = ProjectTransactions::undo(&runtime).expect_err("empty history cannot be undone");
    assert_eq!(undo.code(), Some("undo_unavailable"));
    assert_eq!(undo.history(), &runtime.history_state());

    let redo = ProjectTransactions::redo(&runtime).expect_err("empty redo history cannot be reapplied");
    assert_eq!(redo.code(), Some("redo_unavailable"));
    assert_eq!(redo.history(), &runtime.history_state());
}

#[test]
fn successful_facades_return_the_acknowledged_actor_revision() {
    let runtime = runtime();

    let revision = runtime
        .apply_graph_edit(UiEditIntent::ReevaluateGraph)
        .expect("graph edit should succeed");
    assert_eq!(revision, runtime.history_state());

    let acknowledgement = ProjectTransactions::apply_transaction(&runtime, UiEditIntent::ReevaluateGraph)
        .expect("project transaction should succeed");
    assert!(acknowledgement.success);
    assert_eq!(acknowledgement.history, runtime.history_state());
}
