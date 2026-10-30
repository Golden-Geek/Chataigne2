use super::*;
use crate::{
    app::AppNode,
    screens::{authoring::manager, commands::SCREEN_COMMAND_TYPE, scripting},
};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    application::service_script_app_operations,
    edit::Edit,
    events::EventKind,
    node::{Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    script::{ScriptMethodCall, ScriptNode, ScriptNodeConfig, ScriptSource},
    ui_sync::UiCreateUserItemInitialParam,
};

fn command(engine: &mut AppEngine) -> NodeId {
    let parent = manager(engine).unwrap();
    let preset = engine
        .nodes
        .get(parent)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| item.label == "Create Screen")
        .unwrap();
    let ack = apply_project_intent(
        engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: preset.node_type,
            label: Some("Create Screen".into()),
            initial_params: preset
                .initial_params
                .into_iter()
                .map(|param| UiCreateUserItemInitialParam {
                    decl_id: param.decl_id,
                    value: param.value,
                })
                .collect(),
        },
    );
    assert!(ack.success, "{ack:?}");
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == SCREEN_COMMAND_TYPE)
        .map(|(id, _)| id)
        .unwrap()
}

fn call(engine: &mut AppEngine, node: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
}

fn script(engine: &mut AppEngine, text: String) {
    let parent = manager(engine).unwrap();
    let mut node = ScriptNode::new(
        "Screen Caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text },
        },
    );
    node.budgets.max_wall_time_us_per_load = 1_000_000;
    node.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(node.into(), Some(parent));
    engine.apply_edits().unwrap();
}

fn replies(engine: &AppEngine) -> Vec<serde_json::Value> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "screen-test-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn command_trigger_executes_atomically_and_delivers_a_committed_callback() {
    let mut engine = fixture();
    script(&mut engine, "function init() { local.listen({level:1}); } function onAppOperationResult(reply) { gc.emit('screen-test-reply', reply); }".into());
    let command = command(&mut engine);
    let trigger = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(command, "execute")
        .unwrap();
    let history = engine.undo_len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: trigger,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    let result = replies(&engine);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["status"], "applied");
    assert_eq!(result[0]["value"]["kind"], "ScreenCreated");
    let saved = crate::screens::inspect(&engine).unwrap();
    assert_eq!(saved.screens.len(), 1);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(crate::screens::inspect(&engine).unwrap().screens.is_empty());
    assert!(engine.nodes.contains(command));
    assert!(engine.redo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), saved);
}

#[test]
fn hosted_manager_methods_support_create_inspect_and_domain_rejection() {
    let mut engine = fixture();
    let create = serde_json::to_string(&ScreenEdit::CreateScreen { label: None }).unwrap();
    script(
        &mut engine,
        format!(
            r#"
        function init() {{ local.listen({{level:1}}); local.editScreens('create', {create:?}); }}
        function onAppOperationResult(reply) {{
            gc.emit('screen-test-reply', reply);
            if (reply.request_id === 'create') local.inspectScreens('catalog');
            if (reply.request_id === 'catalog') local.editScreens('reject', JSON.stringify({{
                kind: 'RenameScreen', screen: reply.value.snapshot.screens[0].id, label: ''
            }}));
        }}
    "#
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(replies(&engine).len(), 1);
    service_script_app_operations(&mut engine).unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    service_script_app_operations(&mut engine).unwrap();
    let replies = replies(&engine);
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0]["status"], "applied");
    assert_eq!(replies[1]["value"]["kind"], "Manager");
    assert_eq!(replies[2]["status"], "rejected");
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

#[test]
fn disabled_removed_or_failed_origin_commands_cannot_execute_queued_work() {
    for mode in ["disable", "remove", "rollback"] {
        let mut engine = fixture();
        let command = command(&mut engine);
        if mode == "rollback" {
            engine.edits.push(Edit::CallNodeScriptMethod {
                node: command,
                call: ScriptMethodCall::prepare("execute", vec![]).unwrap(),
            });
            engine.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Int(1),
                behaviour: ParameterEventBehaviour::Append,
            });
            assert!(engine.apply_edits().is_err());
        } else {
            call(&mut engine, command, "execute", vec![]);
            engine.edits.push(if mode == "disable" {
                Edit::PatchMeta {
                    node: command,
                    patch: NodeMetaPatch {
                        enabled: Some(false),
                        ..Default::default()
                    },
                }
            } else {
                Edit::RemoveNode { node: command }
            });
            engine.apply_edits().unwrap();
        }
        service_script_app_operations(&mut engine).unwrap();
        assert!(crate::screens::inspect(&engine).unwrap().screens.is_empty(), "{mode}");
        if mode != "rollback" {
            assert!(engine.undo().unwrap());
            service_script_app_operations(&mut engine).unwrap();
            assert!(crate::screens::inspect(&engine).unwrap().screens.is_empty());
        }
    }
}

#[test]
fn command_reload_keeps_its_request_without_executing_it_and_embedded_template_runs() {
    let mut engine = fixture();
    let command = command(&mut engine);
    let uuid = engine.nodes.get(command).unwrap().node_data().meta.uuid;
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    crate::screens::validate_project(&loaded).unwrap();
    service_script_app_operations(&mut loaded).unwrap();
    assert!(crate::screens::inspect(&loaded).unwrap().screens.is_empty());
    let command = loaded.node_id_by_uuid(uuid).unwrap();
    call(&mut loaded, command, "execute", vec![]);
    service_script_app_operations(&mut loaded).unwrap();
    assert_eq!(crate::screens::inspect(&loaded).unwrap().screens.len(), 1);
    let ScriptSource::Inline { text } = scripting::template().source else {
        panic!()
    };
    assert!(text.contains("local.editScreens"));
    assert!(text.contains("onAppOperationResult"));
    assert!(!text.contains("function update("));
    script(&mut loaded, text);
    service_script_app_operations(&mut loaded).unwrap();
    assert_eq!(crate::screens::inspect(&loaded).unwrap().screens.len(), 1);
}

#[test]
fn production_actor_services_command_trigger_and_undo_without_manual_pumping() {
    use golden_core::{app::prepare_engine_for_runtime, application::ProductionRuntime, ui_sync::UiProjectFileSpec};
    let mut engine = fixture();
    let command = command(&mut engine);
    let trigger = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(command, "execute")
        .unwrap();
    prepare_engine_for_runtime(&mut engine).unwrap();
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let before = runtime.capture_app(|engine| engine.undo_len()).unwrap();
    let applied = runtime
        .apply_ui_transaction(
            UiEditIntent::SetParam {
                node: trigger,
                value: ParamValue::Trigger(),
                behaviour: ParameterEventBehaviour::Append,
            },
            None,
        )
        .unwrap();
    assert!(applied.acknowledgement.success, "{:?}", applied.acknowledgement);
    let (snapshot, history) = runtime
        .capture_app(|engine| (crate::screens::inspect(engine).unwrap(), engine.undo_len()))
        .unwrap();
    assert_eq!(snapshot.screens.len(), 1);
    assert_eq!(snapshot.screens[0].surfaces.len(), 1);
    assert_eq!(history, before + 1);
    assert!(
        runtime
            .apply_ui_transaction(UiEditIntent::Undo, None)
            .unwrap()
            .acknowledgement
            .success
    );
    assert!(
        runtime
            .capture_app(|engine| crate::screens::inspect(engine).unwrap().screens.is_empty())
            .unwrap()
    );
}

#[test]
fn manager_disablement_cancels_pending_requests_and_rejects_new_calls() {
    let mut engine = fixture();
    let manager = manager(&engine).unwrap();
    let request = serde_json::to_string(&ScreenEdit::CreateScreen { label: None }).unwrap();
    call(
        &mut engine,
        manager,
        "editScreens",
        vec![ParamValue::Str("create".into()), ParamValue::Str(request)],
    );
    engine.edits.push(Edit::PatchMeta {
        node: manager,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    service_script_app_operations(&mut engine).unwrap();
    assert!(crate::screens::inspect(&engine).unwrap().screens.is_empty());
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: manager,
        call: ScriptMethodCall::prepare("inspectScreens", vec![ParamValue::Str("disabled".into())]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    assert!(engine.undo().unwrap());
    service_script_app_operations(&mut engine).unwrap();
    assert!(crate::screens::inspect(&engine).unwrap().screens.is_empty());
}
