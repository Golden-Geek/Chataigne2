use super::*;
use crate::script::{ScriptNode, ScriptNodeConfig, ScriptSource};

fn script(engine: &mut Engine<OperationNode>, source: &str) -> NodeId {
    let mut script = ScriptNode::new(
        "Caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text: source.into() },
        },
    );
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    let uuid = script.node_data().meta.uuid;
    engine.add_node(script.into(), Some(engine.root));
    engine.apply_edits().unwrap();
    engine.node_id_by_uuid(uuid).unwrap()
}

fn replies(engine: &Engine<OperationNode>) -> Vec<Value> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            crate::events::EventKind::Custom(event) if event.topic == "script-operation-reply" => {
                Some((*event.payload).clone())
            }
            _ => None,
        })
        .collect()
}

const CALLBACK: &str = "function onAppOperationResult(reply) { gc.emit('script-operation-reply', reply); }";

#[test]
fn result_callback_parameter_writes_follow_runtime_history_policy() {
    let mut engine = engine();
    let mut param = crate::parameter::Parameter::new(
        "reply",
        ParamValue::Str(String::new()),
        crate::parameter::ParameterChangeCheck::ValueChange,
    );
    param.node_data_mut().meta.decl_id = crate::node::DeclId("reply".into());
    let uuid = param.node_data().meta.uuid;
    engine.add_node(param.into(), Some(engine.root));
    engine.apply_edits().unwrap();
    script(
        &mut engine,
        "function init() { gc.invokeAppOperation('create','fixture','create','new-node'); }\nfunction onAppOperationResult(reply) { local.getChild('reply').value = reply.status; }",
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(engine.undo_len(), history + 1);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(
        snapshot
            .node(snapshot.node_id_by_uuid(uuid).unwrap())
            .unwrap()
            .param_value,
        Some(ParamValue::Str("applied".into()))
    );
}

#[test]
fn history_callback_requests_receive_rejection_without_authored_effects() {
    let mut engine = engine();
    let source = script(&mut engine, CALLBACK);
    let owner = std::sync::Arc::new(());
    let nodes = engine.nodes.len();
    let history = engine.undo_len();
    let request = golden_script::ScriptAppOperationRequest::reserve("history", "fixture", "create")
        .unwrap()
        .finish(json!("must-not-exist"))
        .unwrap();
    let mut ctx = ProcessCtx::new(crate::process_ctx::ExecutionPhase::EngineTick, engine.time);
    ctx.enqueue_script_app_operation(source, std::sync::Arc::downgrade(&owner), request);
    engine.absorb_history_callback_edits(&mut ctx).unwrap();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(engine.nodes.len(), nodes);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(replies(&engine).pop().unwrap()["status"], "rejected");
}

#[test]
fn quickjs_script_receives_the_exact_committed_identity_and_one_undo_step() {
    let mut engine = engine();
    script(
        &mut engine,
        &format!(
            "function init() {{ gc.invokeAppOperation('create-1','fixture','create','script-created'); }} {CALLBACK}"
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let reply = replies(&engine).pop().expect("script callback result");
    assert_eq!(reply["status"], "applied");
    assert_eq!(reply["request_id"], "create-1");
    let created = NodeId(reply["value"]["created"].as_u64().unwrap());
    let uuid = engine.nodes.get(created).unwrap().node_data().meta.uuid;
    assert_eq!(serde_json::to_value(uuid).unwrap(), reply["value"]["uuid"]);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    assert!(!engine.nodes.contains(created));
    assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
    assert_eq!(engine.nodes.get(created).unwrap().node_data().meta.uuid, uuid);
}

#[test]
fn domain_and_app_settlement_rejections_leave_no_created_node_or_history() {
    for (operation, payload) in [
        ("reject", "candidate"),
        ("create", "reject at app settlement"),
        ("deepResult", "result-too-deep"),
    ] {
        let mut engine = engine();
        script(
            &mut engine,
            &format!(
                "function init() {{ gc.invokeAppOperation('reject-1','fixture','{operation}',{payload:?}); }} {CALLBACK}"
            ),
        );
        let nodes = engine.nodes.len();
        let history = engine.undo_len();
        service_script_app_operations(&mut engine).unwrap();
        let reply = replies(&engine).pop().unwrap();
        assert_eq!(reply["status"], "rejected");
        assert!(reply.get("value").is_none());
        assert_eq!(engine.nodes.len(), nodes);
        assert_eq!(engine.undo_len(), history);
    }
}

#[test]
fn rolled_back_originating_callbacks_cannot_apply_deferred_app_operations() {
    let mut engine = engine();
    let source = script(&mut engine, CALLBACK);
    let owner = std::sync::Arc::new(());
    let nodes = engine.nodes.len();
    let result: Result<(), crate::engine::EngineEditError> = engine.apply_atomic_edit_operation(|engine| {
        let request = golden_script::ScriptAppOperationRequest::reserve("rolled-back", "fixture", "create")
            .unwrap()
            .finish(json!("must-not-exist"))
            .unwrap();
        let mut ctx = ProcessCtx::new(crate::process_ctx::ExecutionPhase::EngineTick, engine.time);
        ctx.enqueue_script_app_operation(source, std::sync::Arc::downgrade(&owner), request);
        engine.absorb_edits(&mut ctx)?;
        engine.apply_edits()?;
        service_script_app_operations(engine)?;
        assert_eq!(
            engine.nodes.len(),
            nodes,
            "app work must wait until after the outer commit"
        );
        engine.edits.push(Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: crate::parameter::ParameterEventBehaviour::Append,
        });
        engine.apply_edits()
    });
    assert!(result.is_err());
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(engine.nodes.len(), nodes);
    let reply = replies(&engine).pop().unwrap();
    assert_eq!(reply["status"], "rejected");
    assert!(reply["message"].as_str().unwrap().contains("did not commit"));
}

#[test]
fn queued_requests_do_not_survive_script_deletion_or_runtime_reload() {
    for remove in [false, true] {
        let mut engine = engine();
        let source = script(
            &mut engine,
            &format!(
                "function init() {{ gc.invokeAppOperation('old','fixture','create','must-not-exist'); }} {CALLBACK}"
            ),
        );
        if remove {
            engine.edits.push(Edit::RemoveNode { node: source });
            engine.apply_edits().unwrap();
        } else {
            engine
                .ui_set_script_config(
                    source,
                    crate::script::ScriptUiConfig {
                        source: crate::script::ScriptUiSource::Inline {
                            text: "function init() {}".into(),
                        },
                    },
                    true,
                )
                .unwrap();
            engine.run_tick(std::time::Duration::from_millis(20)).unwrap();
        }
        let nodes = engine.nodes.len();
        service_script_app_operations(&mut engine).unwrap();
        assert_eq!(engine.nodes.len(), nodes);
        assert!(replies(&engine).is_empty());
    }
}

#[test]
fn result_callback_requests_wait_for_the_next_bounded_service_turn() {
    let mut engine = engine();
    script(
        &mut engine,
        r#"
        function init() { gc.invokeAppOperation('first','fixture','create','first'); }
        function onAppOperationResult(reply) {
            gc.emit('script-operation-reply', reply);
            if (reply.request_id === 'first') gc.invokeAppOperation('second','fixture','create','second');
        }
    "#,
    );
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(replies(&engine).len(), 1);
    assert_eq!(engine.pending_script_app_operations.len(), 1);
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(replies(&engine).len(), 2);
    assert!(engine.pending_script_app_operations.is_empty());
}
