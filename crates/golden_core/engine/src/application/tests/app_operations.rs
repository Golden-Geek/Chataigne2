use super::*;
use crate::{
    edit::{Edit, NodeTree},
    node::{Folder, NodeCreationContext},
    process_ctx::ProcessCtx,
};
use serde_json::{Value, json};

#[crate::node("operation_ready_failure")]
struct ReadyFailure {}

#[crate::node("operation_ready_failure", from_struct)]
impl Node for ReadyFailure {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: NodeCreationContext) {
        ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
    }
}

crate::define_node_enum!(
    enum OperationNode {
        ReadyFailure,
    }
);

impl ProjectLifecycle for OperationNode {
    fn apply_app_operation(
        engine: &mut Engine<Self>,
        namespace: &str,
        operation: &str,
        payload: Value,
    ) -> Result<Value, String> {
        if namespace != "fixture" {
            return Err("unknown namespace".into());
        }
        let node = Folder::new(payload.as_str().unwrap_or("created"));
        let uuid = node.node_data().meta.uuid;
        let tree = if operation == "lifecycleFailure" {
            NodeTree::new(ReadyFailure::new())
        } else {
            NodeTree::new(node)
        };
        engine.edits.push(Edit::AddNodeTree {
            tree,
            parent: engine.root,
            prev_sibling: None,
        });
        engine.apply_edits().map_err(|error| error.to_string())?;
        if operation == "reject" {
            return Err("domain rejected after insertion".into());
        }
        let id = engine
            .nodes
            .iter()
            .find(|(_, node)| node.node_data().meta.uuid == uuid)
            .map(|(id, _)| id);
        Ok(json!({"source": 99, "created": id.map(|id| id.0), "uuid": uuid}))
    }
    fn settle_pending_operations(engine: &mut Engine<Self>) -> Result<(), String> {
        if engine
            .nodes
            .iter()
            .any(|(_, node)| node.node_data().meta.label == "reject at app settlement")
        {
            return Err("app settlement rejected".into());
        }
        Ok(())
    }
}

fn engine() -> Engine<OperationNode> {
    Engine::new(Folder::new("root").into())
}
fn intent(operation: &str, payload: Value) -> UiEditIntent {
    UiEditIntent::InvokeAppOperation {
        namespace: "fixture".into(),
        operation: operation.into(),
        payload,
    }
}

#[test]
fn committed_results_identify_exact_nodes_without_any_projection_or_discovery() {
    let mut engine = engine();
    let first = apply_project_intent(&mut engine, intent("create", json!("first")));
    let second = apply_project_intent(&mut engine, intent("create", json!("second")));
    assert!(first.success && second.success);
    let first_id = NodeId(first.result.as_ref().unwrap().value["created"].as_u64().unwrap());
    let second_id = NodeId(second.result.as_ref().unwrap().value["created"].as_u64().unwrap());
    assert_ne!(first_id, second_id);
    assert_eq!(engine.nodes.get(first_id).unwrap().node_data().meta.label, "first");
    assert_eq!(engine.nodes.get(second_id).unwrap().node_data().meta.label, "second");
    assert!(!second.history.active_edit_session);
    assert_eq!(second.history.undo_len, 2);
    assert!(first.latest_event_time.is_some());
    assert!(second.latest_event_time > first.latest_event_time);
}

#[test]
fn domain_lifecycle_and_app_settlement_failures_rollback_without_a_result_or_history_entry() {
    for (operation, payload) in [
        ("reject", json!("created")),
        ("lifecycleFailure", Value::Null),
        ("create", json!("reject at app settlement")),
    ] {
        let mut engine = engine();
        let before = engine.nodes.len();
        let ack = apply_project_intent(&mut engine, intent(operation, payload));
        assert!(!ack.success, "{operation}: {ack:?}");
        assert_eq!(ack.error_code.as_deref(), Some("app_operation_failed"));
        assert!(ack.result.is_none());
        assert_eq!(engine.nodes.len(), before);
        assert_eq!(ack.history.undo_len, 0);
        assert!(!ack.history.active_edit_session);
        assert!(engine.edits.pending.is_empty());
    }
}

#[test]
fn operation_undo_and_redo_preserve_the_reported_identity() {
    let mut engine = engine();
    let ack = apply_project_intent(&mut engine, intent("create", json!("undoable")));
    assert!(ack.success);
    let id = NodeId(ack.result.as_ref().unwrap().value["created"].as_u64().unwrap());
    let uuid = engine.nodes.get(id).unwrap().node_data().meta.uuid;
    let undo = apply_project_intent(&mut engine, UiEditIntent::Undo);
    assert!(undo.success && undo.result.is_none());
    assert!(engine.nodes.get(id).is_none());
    assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
    assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.uuid, uuid);
}

#[test]
fn failed_operation_inside_an_explicit_group_preserves_the_accepted_prefix() {
    let mut engine = engine();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::BeginEdit {
                client_edit_id: "group".into(),
                label: None
            }
        )
        .success
    );
    assert!(apply_project_intent(&mut engine, intent("create", json!("accepted"))).success);
    let rejected = apply_project_intent(&mut engine, intent("reject", Value::Null));
    assert!(!rejected.success && rejected.result.is_none());
    assert!(rejected.history.active_edit_session);
    assert_eq!(engine.nodes.len(), 2);
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::EndEdit {
                client_edit_id: "group".into()
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), 1);
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    assert_eq!(engine.nodes.len(), 1);
}

#[test]
fn production_actor_returns_the_same_committed_result_and_failure_contract() {
    let runtime = ProductionRuntime::new(engine(), UiProjectFileSpec::default()).unwrap();
    let applied = runtime
        .apply_ui_transaction(intent("create", json!("runtime")), Some("client"))
        .unwrap();
    assert!(applied.acknowledgement.success);
    let id = applied.acknowledgement.result.as_ref().unwrap().value["created"]
        .as_u64()
        .unwrap();
    assert!(
        runtime
            .call_engine(move |engine| engine.nodes.get(NodeId(id)).is_some())
            .unwrap()
    );
    let rejected = runtime
        .apply_ui_transaction(intent("create", json!("reject at app settlement")), Some("client"))
        .unwrap();
    assert!(!rejected.acknowledgement.success);
    assert!(rejected.acknowledgement.result.is_none());
    assert_eq!(runtime.call_engine(|engine| engine.nodes.len()).unwrap(), 2);
}

#[test]
fn invalid_names_and_engine_only_entrypoint_reject_before_app_execution() {
    let mut engine = engine();
    for namespace in ["".into(), " ".into(), "x".repeat(129)] {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::InvokeAppOperation {
                namespace,
                operation: "create".into(),
                payload: Value::Null,
            },
        );
        assert!(!ack.success && ack.result.is_none());
        assert_eq!(ack.error_code.as_deref(), Some("invalid_app_operation"));
    }
    let ack = engine.apply_ui_intent(intent("create", Value::Null));
    assert!(!ack.success);
    assert_eq!(ack.error_code.as_deref(), Some("app_operation_requires_project"));
    assert_eq!(engine.nodes.len(), 1);
}
