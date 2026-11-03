use super::*;
mod applications;
mod availability;
mod parity;
use crate::app::AppNode;
use crate::media::{authoring::manager, scripting};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    application::service_script_app_operations,
    edit::Edit,
    events::EventKind,
    node::{Node, NodeId},
    parameter::ParamValue,
    script::{ScriptMethodCall, ScriptNode, ScriptNodeConfig, ScriptSource},
};

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
fn composition_instances_use_the_hosted_method_and_committed_callback() {
    use crate::media::composition::protocol::CompositionEdit;
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let request = serde_json::to_string(&MediaEdit::Composition {
        reference: composition,
        edit: CompositionEdit::AddInstances { sources: vec![source] },
    })
    .unwrap();
    script(
        &mut engine,
        format!(
            r#"
        function init() {{ local.listen({{level:1}}); local.editMedia('instances', {request:?}); }}
        function onAppOperationResult(reply) {{ gc.emit('media-method-reply', reply); }}
    "#
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let result = replies(&engine);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["status"], "applied");
    let MediaEditResult::InstancesCreated { instances } = serde_json::from_value(result[0]["value"].clone()).unwrap()
    else {
        panic!()
    };
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].composition, composition);
    assert!(engine.node_id_by_uuid(instances[0].instance.0).is_some());
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(instances[0].instance.0).is_none());
}

#[test]
fn composition_geometry_uses_hosted_method_and_returns_committed_instance() {
    use crate::media::composition::protocol::*;
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let MediaEditResult::InstancesCreated { instances } = apply(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: CompositionEdit::AddInstances { sources: vec![source] },
        },
    ) else {
        panic!()
    };
    let MediaEditResult::Composition { snapshot } = apply(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: CompositionEdit::Inspect,
        },
    ) else {
        panic!()
    };
    let origin = snapshot.instances[0].definition.transform;
    let request = serde_json::to_string(&MediaEdit::Composition {
        reference: composition,
        edit: CompositionEdit::EditInstanceGeometry {
            instance: instances[0].instance,
            edit: InstanceGeometryEdit {
                mode: InstanceGeometryMode::Translate,
                origin,
                start: Point { x: 20.0, y: 20.0 },
                point: Point { x: 50.0, y: 80.0 },
            },
        },
    })
    .unwrap();
    script(
        &mut engine,
        format!(
            r#"function init() {{ local.listen({{level:1}}); local.editMedia('geometry', {request:?}); }} function onAppOperationResult(reply) {{ gc.emit('media-method-reply', reply); }}"#
        ),
    );
    let before = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let replies = replies(&engine);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["status"], "applied");
    let MediaEditResult::InstanceChanged { snapshot } = serde_json::from_value(replies[0]["value"].clone()).unwrap()
    else {
        panic!()
    };
    assert_eq!(snapshot.reference, instances[0]);
    assert_eq!(
        snapshot.definition.transform.position,
        [origin.position[0] + 30.0, origin.position[1] + 60.0]
    );
    assert_eq!(engine.undo_len(), before + 1);
}

#[test]
fn matrix_authored_intents_use_hosted_methods_and_committed_callbacks() {
    use crate::matrix::protocol::*;
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    crate::matrix::tests::clear_scene(&mut engine, matrix.media);
    let request = serde_json::to_string(&MediaEdit::Matrix {
        media: matrix.media,
        edit: MatrixEdit::AddLayers { parent: None, count: 2 },
    })
    .unwrap();
    script(
        &mut engine,
        format!(
            r#"function init() {{ local.listen({{level:1}}); local.editMedia('layers', {request:?}); }} function onAppOperationResult(reply) {{ gc.emit('media-method-reply', reply); }}"#
        ),
    );
    let before = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let result = replies(&engine);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["status"], "applied");
    let MediaEditResult::MatrixCreated { items, .. } = serde_json::from_value(result[0]["value"].clone()).unwrap()
    else {
        panic!()
    };
    let snapshot = crate::matrix::inspect(&engine, matrix.media).unwrap();
    assert_eq!(snapshot.items.iter().map(|item| item.id).collect::<Vec<_>>(), items);
    assert_eq!(engine.undo_len(), before + 1);
    assert!(engine.undo().unwrap());
    assert!(crate::matrix::inspect(&engine, matrix.media).unwrap().items.is_empty());
    assert!(engine.redo().unwrap());
    assert_eq!(crate::matrix::inspect(&engine, matrix.media).unwrap(), snapshot);
}

#[test]
fn hosted_owned_application_configuration_and_inspection_return_committed_identities_without_launching() {
    use crate::applications::protocol::*;
    let mut engine = fixture();
    script(&mut engine, "function init() { local.listen({level:1}); } function onAppOperationResult(reply) { gc.emit('media-method-reply', reply); }".into());
    let manager = manager(&engine).unwrap();
    let application = crate::applications::tests::create(&mut engine);
    let request = serde_json::to_string(&MediaEdit::Applications {
        edit: ApplicationEdit::Configure {
            application,
            definition: ApplicationDefinition {
                arguments: vec!["authored only".into()],
                ..Default::default()
            },
        },
    })
    .unwrap();
    let before = engine.undo_len();
    call(
        &mut engine,
        manager,
        "editMedia",
        vec![ParamValue::Str("create-app".into()), ParamValue::Str(request)],
    );
    service_script_app_operations(&mut engine).unwrap();
    let first = replies(&engine);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["status"], "applied");
    assert_eq!(first[0]["value"]["kind"], "Changed");
    assert!(
        crate::applications::inspect(&engine, application)
            .unwrap()
            .definition
            .executable
            .is_none()
    );
    assert_eq!(engine.undo_len(), before + 1);
    let request = serde_json::to_string(&MediaEdit::Applications {
        edit: ApplicationEdit::Inspect { application },
    })
    .unwrap();
    call(
        &mut engine,
        manager,
        "editMedia",
        vec![ParamValue::Str("app-inspect".into()), ParamValue::Str(request)],
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let inspected = replies(&engine);
    assert_eq!(inspected.len(), 2);
    assert_eq!(inspected[1]["request_id"], "app-inspect");
    assert_eq!(inspected[1]["value"]["kind"], "Application");
    assert_eq!(engine.undo_len(), history);
    call(
        &mut engine,
        manager,
        "editMedia",
        vec![
            ParamValue::Str("app-runtime".into()),
            ParamValue::Str(
                serde_json::to_string(&MediaEdit::Applications {
                    edit: ApplicationEdit::InspectRuntime {},
                })
                .unwrap(),
            ),
        ],
    );
    service_script_app_operations(&mut engine).unwrap();
    let runtime = replies(&engine);
    assert_eq!(runtime[2]["request_id"], "app-runtime");
    assert_eq!(runtime[2]["value"]["kind"], "ApplicationRuntime");
    assert_eq!(runtime[2]["value"]["snapshot"]["applications"], serde_json::json!([]));
    assert_eq!(engine.undo_len(), history);
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
