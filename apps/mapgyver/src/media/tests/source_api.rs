use super::*;
use crate::{
    app::AppNode,
    media::{authoring::manager, source_api},
};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    edit::Edit,
    node::Node,
    parameter::ParamValue,
    script::{ScriptMethodCall, ScriptSource},
};

#[test]
fn app_template_creation_is_explicit_atomic_passive_and_roundtrips_with_stable_identity() {
    let mut engine = fixture();
    let parent = manager(&engine).unwrap();
    let catalog = engine.catalog_creatable_items(parent);
    assert!(catalog.iter().all(|item| item.node_type != "script"));
    let history = engine.undo_len();
    let MediaEditResult::ScriptCreated { script } = apply(&mut engine, MediaEdit::CreateScript { label: None }) else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let id = engine.node_id_by_uuid(script).unwrap();
    let Some(AppNode::Script(node)) = engine.nodes.get(id) else {
        panic!()
    };
    assert_eq!(node.node_data().parent, Some(parent));
    let ScriptSource::Inline { text } = &node.config.source else {
        panic!()
    };
    let template = text.clone();
    assert!(template.contains("onAppOperationResult"));
    for method in source_api::METHODS {
        assert!(template.contains(method), "template omits {method}");
        assert!(
            engine
                .nodes
                .get(parent)
                .unwrap()
                .engine_script_descriptor()
                .methods
                .iter()
                .any(|name| name == method)
        );
    }
    assert!(node.ui_state().manifest.is_some(), "template must load");
    assert_eq!(node.execution_rule(), golden_core::engine::NodeExecutionRule::passive());
    assert_eq!(engine.catalog_creatable_items(parent), catalog);
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(script).is_none());
    assert!(engine.redo().unwrap());
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    let Some(AppNode::Script(node)) = loaded.nodes.get(loaded.node_id_by_uuid(script).unwrap()) else {
        panic!()
    };
    assert_eq!(node.config.source, ScriptSource::Inline { text: template });
    assert_eq!(node.execution_rule(), golden_core::engine::NodeExecutionRule::passive());
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::CreateScript {
                label: Some(" ".into())
            }
        )
        .success
    );
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn named_method_argument_rejection_never_queues_work_or_changes_history() {
    let mut engine = fixture();
    let parent = manager(&engine).unwrap();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let media = ParamValue::Str(reference.media.0.0.to_string());
    for (method, args) in [
        ("createMedia", vec![ParamValue::Str("bad".into())]),
        (
            "addChannel",
            vec![ParamValue::Str("not-a-uuid".into()), ParamValue::Str("".into())],
        ),
        (
            "configureMediaResolution",
            vec![
                media.clone(),
                ParamValue::Bool(true),
                ParamValue::Float(1.5),
                ParamValue::Int(10),
            ],
        ),
        (
            "configureMediaResolution",
            vec![
                media.clone(),
                ParamValue::Bool(true),
                ParamValue::Int(0),
                ParamValue::Int(10),
            ],
        ),
        (
            "configureMediaResolution",
            vec![
                media.clone(),
                ParamValue::Bool(true),
                ParamValue::Float(f64::from(u32::MAX) + 1.0),
                ParamValue::Int(10),
            ],
        ),
        (
            "configureMediaFitMode",
            vec![media.clone(), ParamValue::Str("invalid".into())],
        ),
        (
            "importPicture",
            vec![ParamValue::Str("../escape.png".into()), ParamValue::Bool(false)],
        ),
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let args = std::iter::once(ParamValue::Str("bad".into())).chain(args).collect();
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: parent,
            call: ScriptMethodCall::prepare(method, args).unwrap(),
        });
        assert!(engine.apply_edits().is_err(), "{method} must reject");
        golden_core::application::service_script_app_operations(&mut engine).unwrap();
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    assert!(source_api::operation("other", &[]).unwrap().is_none());
}

#[test]
fn script_creation_has_one_committed_callback_and_history_does_not_replay_it() {
    use golden_core::{
        application::service_script_app_operations,
        events::EventKind,
        script::{ScriptNode, ScriptNodeConfig},
    };
    let mut engine = fixture();
    let parent = manager(&engine).unwrap();
    let mut caller = ScriptNode::new("Template caller", ScriptNodeConfig { source: ScriptSource::Inline { text:
        "function init(){local.listen({level:1});local.editMedia('template', JSON.stringify({kind:'CreateScript',label:null}));} function onAppOperationResult(reply){gc.emit('template-reply',reply);}".into()
    } });
    caller.budgets.max_wall_time_us_per_load = 1_000_000;
    caller.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(caller.into(), Some(parent));
    engine.apply_edits().unwrap();
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let responses = |engine: &AppEngine| {
        engine
            .ui_event_log()
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::Custom(event) if event.topic == "template-reply" => Some((*event.payload).clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let reply = responses(&engine);
    assert_eq!(reply.len(), 1);
    assert_eq!(reply[0]["status"], "applied");
    let MediaEditResult::ScriptCreated { script } = serde_json::from_value(reply[0]["value"].clone()).unwrap() else {
        panic!()
    };
    assert!(engine.node_id_by_uuid(script).is_some());
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(script).is_none());
    assert!(engine.redo().unwrap());
    service_script_app_operations(&mut engine).unwrap();
    assert!(engine.node_id_by_uuid(script).is_some());
    assert_eq!(responses(&engine), reply);
}
