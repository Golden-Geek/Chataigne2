use super::*;
use crate::app::AppNode;
use crate::media::{authoring::manager, commands::MEDIA_COMMAND_TYPE, scripting};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    application::service_script_app_operations,
    edit::Edit,
    events::{EventFrame, EventKind},
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    script::{ScriptMethodCall, ScriptNode, ScriptNodeConfig, ScriptSource},
    ui_sync::UiCreateUserItemInitialParam,
};

fn command(engine: &mut AppEngine, label: &str) -> NodeId {
    let parent = manager(engine).unwrap();
    let preset = engine
        .nodes
        .get(parent)
        .unwrap()
        .user_creatable_items()
        .into_iter()
        .find(|item| item.label == label)
        .unwrap();
    let ack = apply_project_intent(
        engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: preset.node_type,
            label: Some(label.into()),
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
        .find(|(_, node)| node.get_type() == MEDIA_COMMAND_TYPE && node.node_data().meta.label == label)
        .map(|(id, _)| id)
        .unwrap()
}

fn call(engine: &mut AppEngine, node: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
    // Exclusive-engine callers also apply ordinary edits queued by the method itself.
    engine.apply_edits().unwrap();
}

fn script(engine: &mut AppEngine, source: String) {
    let parent = manager(engine).unwrap();
    let mut script = ScriptNode::new(
        "Media Caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text: source },
        },
    );
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(script.into(), Some(parent));
    engine.apply_edits().unwrap();
}

fn replies(engine: &AppEngine) -> Vec<serde_json::Value> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "media-method-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn catalog_commands_execute_through_the_public_trigger_and_undo_route() {
    for (label, kind) in [
        ("Create Solid Color", MediaKind::SolidColor),
        ("Create Picture", MediaKind::Picture),
        ("Create Shared Texture", MediaKind::SharedTexture),
    ] {
        let mut engine = fixture();
        script(&mut engine, "function init() { local.listen({level:1}); } function onAppOperationResult(reply) { gc.emit('media-method-reply', reply); }".into());
        let command = command(&mut engine, label);
        let node = engine.nodes.get(command).unwrap();
        assert!(!node.needs_update());
        assert!(!node.inbox_requires_tree_snapshot(&EventFrame::default()));
        let trigger = engine
            .process_tree_snapshot()
            .find_child_by_decl_id(command, "execute")
            .unwrap();
        let before = engine.undo_len();
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: trigger,
                value: ParamValue::Trigger(),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
        let snapshot = crate::media::catalog::inspect(&engine).unwrap();
        assert_eq!(snapshot.media.len(), 1);
        assert_eq!(snapshot.media[0].media_kind, kind);
        assert_eq!(
            snapshot.media[0].channels[0].definition,
            ChannelDefinition::for_media(kind)
        );
        assert_eq!(engine.undo_len(), before + 1);
        let result = replies(&engine);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["status"], "applied");
        assert!(engine.undo().unwrap());
        assert!(crate::media::catalog::inspect(&engine).unwrap().media.is_empty());
        assert!(engine.nodes.contains(command));
        assert!(engine.redo().unwrap());
        assert_eq!(crate::media::catalog::inspect(&engine).unwrap(), snapshot);
    }
}

#[test]
fn hosted_quickjs_methods_deliver_committed_results_then_read_only_inspection() {
    let mut engine = fixture();
    let request = serde_json::to_string(&MediaEdit::CreateMedia {
        media_kind: MediaKind::SharedTexture,
        label: Some("Script source".into()),
    })
    .unwrap();
    script(
        &mut engine,
        format!(
            r#"
        function init() {{ local.listen({{level:1}}); local.editMedia('create', {request:?}); }}
        function onAppOperationResult(reply) {{
            gc.emit('media-method-reply', reply);
            if (reply.request_id === 'create') local.inspectMedia('catalog');
        }}
    "#
        ),
    );
    let before = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let first = replies(&engine);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["status"], "applied");
    assert_eq!(first[0]["value"]["kind"], "Created");
    let reference: ChannelReference = serde_json::from_value(first[0]["value"]["reference"].clone()).unwrap();
    assert_eq!(crate::media::channel(&engine, reference).unwrap().label, "Channel 1");
    service_script_app_operations(&mut engine).unwrap();
    let replies = replies(&engine);
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[1]["request_id"], "catalog");
    assert_eq!(replies[1]["value"]["kind"], "Manager");
    assert_eq!(engine.undo_len(), before + 1);
}

#[test]
fn hosted_method_domain_rejection_delivers_one_callback_without_a_partial_edit() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Picture);
    let request = serde_json::to_string(&MediaEdit::RemoveChannel { reference }).unwrap();
    script(
        &mut engine,
        format!(
            r#"
        function init() {{ local.listen({{level:1}}); local.editMedia('reject', {request:?}); }}
        function onAppOperationResult(reply) {{ gc.emit('media-method-reply', reply); }}
    "#
        ),
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(engine.undo_len(), history);
    let result = replies(&engine);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["status"], "rejected");
    assert!(result[0].get("value").is_none());
    assert!(crate::media::channel(&engine, reference).is_ok());
}

#[test]
fn node_methods_keep_standard_script_behavior_and_reject_bad_requests_without_history() {
    let mut engine = fixture();
    let parent = manager(&engine).unwrap();
    call(
        &mut engine,
        parent,
        "setDescription",
        vec![ParamValue::Str("Media module".into())],
    );
    assert_eq!(
        engine
            .nodes
            .get(parent)
            .unwrap()
            .node_data()
            .meta
            .description
            .as_deref(),
        Some("Media module")
    );
    for request in ["{".into(), "x".repeat(scripting::MAX_REQUEST_JSON_BYTES + 1)] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: parent,
            call: ScriptMethodCall::prepare(
                "editMedia",
                vec![ParamValue::Str("invalid".into()), ParamValue::Str(request)],
            )
            .unwrap(),
        });
        assert!(engine.apply_edits().is_err());
        service_script_app_operations(&mut engine).unwrap();
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(engine.undo_len(), history);
    }
}

#[test]
fn deleting_or_disabling_a_command_cancels_queued_work_and_undo_does_not_revive_it() {
    for disable in [false, true] {
        let mut engine = fixture();
        let command = command(&mut engine, "Create Picture");
        call(&mut engine, command, "execute", vec![]);
        if disable {
            engine.edits.push(Edit::PatchMeta {
                node: command,
                patch: golden_core::node::NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                },
            });
        } else {
            engine.edits.push(Edit::RemoveNode { node: command });
        }
        engine.apply_edits().unwrap();
        service_script_app_operations(&mut engine).unwrap();
        assert!(crate::media::catalog::inspect(&engine).unwrap().media.is_empty());
        if disable {
            engine.edits.push(Edit::CallNodeScriptMethod {
                node: command,
                call: ScriptMethodCall::prepare("execute", vec![]).unwrap(),
            });
            assert!(engine.apply_edits().is_err());
            service_script_app_operations(&mut engine).unwrap();
            assert!(crate::media::catalog::inspect(&engine).unwrap().media.is_empty());
        }
        assert!(engine.undo().unwrap());
        service_script_app_operations(&mut engine).unwrap();
        assert!(crate::media::catalog::inspect(&engine).unwrap().media.is_empty());
    }
}

#[test]
fn a_failed_originating_edit_batch_cannot_execute_its_queued_media_command() {
    let mut engine = fixture();
    let command = command(&mut engine, "Create Solid Color");
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
    service_script_app_operations(&mut engine).unwrap();
    assert!(crate::media::catalog::inspect(&engine).unwrap().media.is_empty());
}

#[test]
fn command_project_roundtrip_preserves_authored_request_without_running_it_on_load() {
    let mut engine = fixture();
    let command = command(&mut engine, "Create Shared Texture");
    let uuid = engine.nodes.get(command).unwrap().node_data().meta.uuid;
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    service_script_app_operations(&mut loaded).unwrap();
    assert!(crate::media::catalog::inspect(&loaded).unwrap().media.is_empty());
    let command = loaded.node_id_by_uuid(uuid).unwrap();
    call(&mut loaded, command, "execute", vec![]);
    service_script_app_operations(&mut loaded).unwrap();
    assert_eq!(
        crate::media::catalog::inspect(&loaded).unwrap().media[0].media_kind,
        MediaKind::SharedTexture
    );
}

#[test]
fn production_actor_services_an_ordinary_command_trigger_and_undo_without_manual_pumping() {
    use golden_core::{app::prepare_engine_for_runtime, application::ProductionRuntime, ui_sync::UiProjectFileSpec};
    let mut engine = fixture();
    let command = command(&mut engine, "Create Solid Color");
    let trigger = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(command, "execute")
        .unwrap();
    prepare_engine_for_runtime(&mut engine).unwrap();
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let before = runtime.capture_app(|engine| engine.undo_len()).unwrap();
    let result = runtime
        .apply_ui_transaction(
            UiEditIntent::SetParam {
                node: trigger,
                value: ParamValue::Trigger(),
                behaviour: ParameterEventBehaviour::Append,
            },
            None,
        )
        .unwrap();
    assert!(result.acknowledgement.success, "{:?}", result.acknowledgement);
    let (snapshot, history) = runtime
        .capture_app(|engine| (crate::media::catalog::inspect(engine).unwrap(), engine.undo_len()))
        .unwrap();
    assert_eq!(snapshot.media.len(), 1);
    assert_eq!(snapshot.media[0].media_kind, MediaKind::SolidColor);
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
            .capture_app(|engine| crate::media::catalog::inspect(engine).unwrap().media.is_empty())
            .unwrap()
    );
}

#[test]
fn factory_snippets_are_app_owned_event_driven_and_execute_in_quickjs() {
    let mut engine = fixture();
    let ScriptSource::Inline { text } = scripting::template().source else {
        panic!()
    };
    assert!(text.contains("local.editMedia"));
    assert!(text.contains("onAppOperationResult"));
    assert!(!text.contains("function update("));
    script(&mut engine, text);
    service_script_app_operations(&mut engine).unwrap();
    assert!(crate::media::catalog::inspect(&engine).unwrap().media.is_empty());
    let parent = manager(&engine).unwrap();
    let host = engine.nodes.get(parent).unwrap();
    assert!(host.script_host_policy().unwrap().enabled);
    let factory = host.create_user_item("script").unwrap();
    assert_eq!(
        factory.engine_script_state().unwrap().config,
        (&scripting::template()).into()
    );
    assert!(host.create_user_item("chataigne.os_module").is_none());
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: "script".into(),
            label: Some("Factory script".into()),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let factory = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Factory script")
        .unwrap()
        .1;
    assert_eq!(
        factory.engine_script_state().unwrap().config,
        (&scripting::template()).into()
    );
}
