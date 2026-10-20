use super::*;
use golden_core::{
    edit::Edit,
    events::CustomEventRetention,
    node::{Folder, Node},
    parameter::ParameterEventBehaviour,
};

fn fixture() -> (crate::app::AppEngine, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    engine.add_node(crate::app::HttpModule::create().into(), None);
    for _ in 0..5 {
        engine.apply_edits().unwrap();
    }
    let module = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine.add_node(crate::app::HttpRequestCommand::create().into(), Some(module));
    for _ in 0..5 {
        engine.apply_edits().unwrap();
    }
    let snapshot = engine.process_tree_snapshot();
    let command = *snapshot
        .child_ids_slice(module)
        .iter()
        .find(|id| snapshot.node(**id).unwrap().node_type == "http_request_command")
        .unwrap();
    (engine, command)
}

#[test]
fn command_capture_defers_invalid_body_parsing_and_freezes_the_invocation_view() {
    let (mut engine, command) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let path = snapshot.find_child_by_decl_id(command, "path").unwrap();
    let body = snapshot.find_child_by_decl_id(command, "body").unwrap();
    let mode = snapshot.find_child_by_decl_id(command, "body_mode").unwrap();
    let view = Arc::new(snapshot.with_param_values([
        (body, ParamValue::Str("[".into())),
        (mode, ParamValue::Enum("json".into())),
    ]));
    let bad = HttpInput::capture_command(&view, command, HttpCommandKind::Request, None, |id| {
        engine.node_waker(id).map_err(|error| error.to_string())
    })
    .unwrap();
    assert!(std::thread::spawn(move || bad.materialize()).join().unwrap().is_err());
    let event = golden_core::script::ScriptCustomEvent::prepare(
        crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
        serde_json::json!({"command_id":command, "param_overrides":[
            {"param_id":path, "value":ParamValue::Str("/frozen".into())},
            {"param_id":body, "value":ParamValue::Str("{\"value\":7}".into())}
        ]}),
    )
    .unwrap();
    let (topic, payload) = event.into_shared_parts();
    let event = CustomEvent::from_shared_payload(topic, None, payload, CustomEventRetention::Transient);
    let input = HttpInput::capture_command(&view, command, HttpCommandKind::Request, Some(event), |id| {
        engine.node_waker(id).map_err(|error| error.to_string())
    })
    .unwrap();
    engine.edits.push(Edit::SetParam {
        node: path,
        value: ParamValue::Str("/later".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    let result = std::thread::spawn(move || input.materialize()).join().unwrap().unwrap();
    assert_eq!(result.payload.path, "/frozen");
    assert_eq!(
        result.payload.body,
        crate::app::module::common::http::HttpRequestBody::Json {
            value: serde_json::json!({"value":7})
        }
    );
}

#[test]
fn command_capture_rejects_spare_source_capacity_before_any_conversion() {
    let (engine, command) = fixture();
    let snapshot = engine.process_tree_snapshot();
    let body = snapshot.find_child_by_decl_id(command, "body").unwrap();
    let view = Arc::new(snapshot.with_param_values([(body, ParamValue::Str(String::with_capacity(4 * 1024 * 1024)))]));
    assert!(
        HttpInput::capture_command(&view, command, HttpCommandKind::Request, None, |id| engine
            .node_waker(id)
            .map_err(|error| error.to_string()))
        .unwrap_err()
        .contains("4 MiB")
    );
}

#[test]
fn retired_source_fences_new_admission_without_revoking_an_already_accepted_input() {
    let (mut engine, command) = fixture();
    let snapshot = engine.process_tree_snapshot();
    assert!(
        HttpInput::capture_command(&snapshot, command, HttpCommandKind::UploadFile, None, |id| engine
            .node_waker(id)
            .map_err(|error| error.to_string()))
        .is_err()
    );
    let input = HttpInput::capture_command(&snapshot, command, HttpCommandKind::Request, None, |id| {
        engine.node_waker(id).map_err(|error| error.to_string())
    })
    .unwrap();
    assert!(input.eligible_for_admission());
    engine.edits.push(Edit::RemoveNode { node: command });
    engine.apply_edits().unwrap();
    assert!(!input.eligible_for_admission());
    // Native admission is the ownership boundary; conversion does not revalidate
    // source lifecycle after an input has already crossed it.
    assert!(std::thread::spawn(move || input.materialize()).join().unwrap().is_ok());
}
