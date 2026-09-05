use std::time::Duration;

use std::collections::HashMap;

use golden_core::{
    events::EventKind,
    node::{Folder, Node, NodeId},
    parameter::ParamValue,
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};
use serde_json::json;

use super::{
    transport::{HttpRequestOrigin, HttpResponse},
    HttpModule,
};
use crate::app::module::common::http::{
    HttpMethod, HttpRequestBody, HttpRequestPayload, HTTP_REQUEST_COMMAND_NODE_TYPE,
    HTTP_UPLOAD_FILE_COMMAND_NODE_TYPE,
};

#[test]
fn http_module_command_tester_advertises_http_commands() {
    let (engine, module_id) = create_http_module();
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");

    let available_types = engine
        .catalog_creatable_items(command_tester_id)
        .into_iter()
        .map(|item| item.node_type)
        .collect::<Vec<_>>();

    assert_eq!(
        available_types,
        vec![
            HTTP_REQUEST_COMMAND_NODE_TYPE.to_string(),
            HTTP_UPLOAD_FILE_COMMAND_NODE_TYPE.to_string(),
        ],
        "HTTP command tester should advertise request and upload commands"
    );
}

#[test]
fn http_module_script_descriptor_advertises_request_methods() {
    let descriptor = HttpModule::create().engine_script_descriptor();

    for method in [
        "request",
        "requestJson",
        "get",
        "post",
        "postJson",
        "put",
        "patch",
        "delete",
        "uploadFile",
    ] {
        assert!(
            descriptor.methods.iter().any(|candidate| candidate == method),
            "HTTP script descriptor should advertise '{method}'"
        );
    }
}

#[test]
fn http_module_script_template_scaffolds_http_callbacks_only() {
    let config = crate::app::module::script_api::module_script_config(HttpModule::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("HTTP module script template should resolve to inline source");
    };

    assert!(source.contains("local.request"));
    assert!(source.contains("local.uploadFile"));
    assert!(source.contains("function responseReceived"));
    assert!(source.contains("function requestFailed"));
    assert!(!source.contains("function messageReceived"));
    assert!(!source.contains("function noteOnReceived"));
}

#[test]
fn json_response_auto_adds_values_under_request_path() {
    let (mut engine, module_id) = create_http_module();

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected HttpModule node");
    };
    module.enqueue_response_for_test(test_response(
        "/api/state",
        Some("application/json"),
        br#"{"temperature":21,"active":true,"color":[1.0,0.5,0.0,1.0]}"#.to_vec(),
    ));

    run_http_runtime_ticks(&mut engine, 6);

    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/api/state/temperature")
                .expect("temperature value should be created"),
        ),
        ParamValue::Int(21)
    );
    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/api/state/active")
                .expect("active value should be created"),
        ),
        ParamValue::Bool(true)
    );
    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/api/state/color").expect("color value should be created"),
        ),
        ParamValue::Color(1.0, 0.5, 0.0, 1.0)
    );

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get(module_id).expect("module should still exist") else {
        panic!("expected HttpModule node");
    };
    assert!(
        !module.has_pending_responses_for_test(),
        "HTTP response queue should drain after value application"
    );
}

#[test]
fn non_json_response_does_not_add_values() {
    let (mut engine, module_id) = create_http_module();

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected HttpModule node");
    };
    module.enqueue_response_for_test(test_response(
        "/plain",
        Some("text/plain; charset=utf-8"),
        b"hello".to_vec(),
    ));

    run_http_runtime_ticks(&mut engine, 3);

    assert!(
        find_path(&engine, module_id, "values/plain").is_none(),
        "plain text responses should not create module Values"
    );

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get(module_id).expect("module should still exist") else {
        panic!("expected HttpModule node");
    };
    assert!(
        !module.has_pending_responses_for_test(),
        "non-JSON HTTP responses should still drain after script callback emission"
    );
}

#[test]
fn large_json_response_auto_adds_values_in_one_runtime_tick() {
    let (mut engine, module_id) = create_http_module();

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected HttpModule node");
    };
    module.enqueue_response_for_test(test_response(
        "character",
        Some("application/json"),
        rick_and_morty_like_response(),
    ));

    run_http_runtime_ticks(&mut engine, 1);

    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/character/info/count")
                .expect("response info count should be created"),
        ),
        ParamValue::Int(826)
    );
    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/character/results/20/name")
                .expect("last character name should be created"),
        ),
        ParamValue::Str("Character 20".to_string())
    );
    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/character/results/20/episode/value 8")
                .expect("last character episode list should be created"),
        ),
        ParamValue::Str("https://rickandmortyapi.com/api/episode/8".to_string())
    );
    assert_no_duplicate_child_keys(
        &engine,
        find_path(&engine, module_id, "values").expect("values root should exist"),
    );
    let crate::app::AppNode::HttpModule(module) = engine.nodes.get(module_id).expect("module should still exist") else {
        panic!("expected HttpModule node");
    };
    assert!(
        !module.has_pending_responses_for_test(),
        "large HTTP JSON responses should drain in one runtime tick"
    );
}

#[test]
fn repeated_large_json_responses_update_existing_values_without_duplicate_folders() {
    let (mut engine, module_id) = create_http_module();
    let response = test_response(
        "character",
        Some("application/json"),
        rick_and_morty_like_response(),
    );

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected HttpModule node");
    };
    for _ in 0..4 {
        module.enqueue_response_for_test(response.clone());
    }

    run_http_runtime_ticks(&mut engine, 1);

    assert_no_duplicate_child_keys(
        &engine,
        find_path(&engine, module_id, "values").expect("values root should exist"),
    );
    assert_eq!(
        direct_child_count(
            &engine,
            find_path(&engine, module_id, "values/character/results").expect("results folder should exist"),
        ),
        20,
        "repeated responses should update the existing 20 result folders"
    );
}

#[test]
fn utf8_response_callback_does_not_duplicate_body_as_byte_array() {
    let (mut engine, module_id) = create_http_module();

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected HttpModule node");
    };
    module.enqueue_response_for_test(test_response(
        "/plain",
        Some("text/plain; charset=utf-8"),
        b"hello".to_vec(),
    ));

    run_http_until_drained(&mut engine, module_id, 10);

    let payload = response_received_callback_payload(&engine).expect("response callback should be emitted");
    let args = payload
        .get("args")
        .and_then(serde_json::Value::as_array)
        .expect("response callback should include args");
    assert_eq!(args.get(1), Some(&json!("hello")));

    let response = args.get(2).expect("response details arg should exist");
    assert_eq!(response.get("body"), Some(&json!("hello")));
    assert_eq!(response.get("bodyBytes"), Some(&json!(5)));
    assert!(
        response.get("bytes").is_some_and(serde_json::Value::is_null),
        "UTF-8 response details should not duplicate the body as a byte array"
    );
    assert!(
        response.get("json").is_some_and(serde_json::Value::is_null),
        "plain text response should not include a parsed JSON payload"
    );
}

fn create_http_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(HttpModule::create().into(), None);
    engine.apply_edits().expect("HTTP module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("HTTP defaults should materialize");
    }
    engine.resolve().expect("HTTP runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("HTTP module should be attached under root");

    let crate::app::AppNode::HttpModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected HttpModule node");
    };
    module.disable_transport_for_test();

    (engine, module_id)
}

fn rick_and_morty_like_response() -> Vec<u8> {
    let results = (1..=20)
        .map(|id| {
            json!({
                "id": id,
                "name": format!("Character {id}"),
                "status": "Alive",
                "species": "Human",
                "type": "",
                "gender": "unknown",
                "origin": {
                    "name": "Earth (C-137)",
                    "url": format!("https://rickandmortyapi.com/api/location/{id}"),
                },
                "location": {
                    "name": "Citadel of Ricks",
                    "url": format!("https://rickandmortyapi.com/api/location/{}", id + 20),
                },
                "image": format!("https://rickandmortyapi.com/api/character/avatar/{id}.jpeg"),
                "episode": (1..=8)
                    .map(|episode| format!("https://rickandmortyapi.com/api/episode/{episode}"))
                    .collect::<Vec<_>>(),
                "url": format!("https://rickandmortyapi.com/api/character/{id}"),
                "created": "2017-11-04T18:48:46.250Z",
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_vec(&json!({
        "info": {
            "count": 826,
            "pages": 42,
            "next": "https://rickandmortyapi.com/api/character?page=2",
            "prev": null,
        },
        "results": results,
    }))
    .expect("synthetic response should serialize")
}

fn test_response(path: &str, content_type: Option<&str>, body: Vec<u8>) -> HttpResponse {
    HttpResponse {
        origin: HttpRequestOrigin::Script {
            method: "get".to_string(),
        },
        request: HttpRequestPayload {
            method: HttpMethod::Get,
            path: path.to_string(),
            query: String::new(),
            value_path: String::new(),
            headers: Vec::new(),
            body: HttpRequestBody::Empty,
            description: "test response".to_string(),
        },
        url: format!("http://127.0.0.1:8080{}", path),
        status: 200,
        status_text: "OK".to_string(),
        headers: Vec::new(),
        content_type: content_type.map(str::to_string),
        body: body.clone(),
        received_values: crate::app::module::common::http::response_json_received_values(
            body.as_slice(),
            content_type,
            path,
            "",
        ),
        elapsed_ms: 1,
        attempts: 1,
    }
}

fn run_http_runtime_ticks(engine: &mut crate::app::AppEngine, count: usize) {
    for _ in 0..count {
        engine
            .dispatch_inbox(ExecutionPhase::EngineTick)
            .expect("pending HTTP events should dispatch");
        engine.apply_edits().expect("pending HTTP event reactions should apply");
        engine
            .run_tick(Duration::from_millis(20))
            .expect("HTTP runtime tick should succeed");
        engine.apply_edits().expect("pending HTTP edits should apply");
        engine.resolve().expect("HTTP runtime schedule should resolve");
    }
}

fn run_http_until_drained(engine: &mut crate::app::AppEngine, module_id: NodeId, max_ticks: usize) {
    for _ in 0..max_ticks {
        run_http_runtime_ticks(engine, 1);

        let crate::app::AppNode::HttpModule(module) = engine.nodes.get(module_id).expect("module should still exist") else {
            panic!("expected HttpModule node");
        };
        if !module.has_pending_responses_for_test() {
            return;
        }
    }

    panic!("HTTP response queue did not drain within {max_ticks} ticks");
}

fn response_received_callback_payload(engine: &crate::app::AppEngine) -> Option<&serde_json::Value> {
    engine.ui_event_log().iter().find_map(|event| {
        let EventKind::Custom(custom) = &event.kind else {
            return None;
        };
        if custom.topic != crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC {
            return None;
        }
        (custom
            .payload
            .get("callback")
            .and_then(serde_json::Value::as_str)
            == Some("responseReceived"))
        .then_some(custom.payload.as_ref())
    })
}

fn assert_no_duplicate_child_keys(engine: &crate::app::AppEngine, start: NodeId) {
    let mut stack = vec![start];
    while let Some(parent) = stack.pop() {
        let mut counts = HashMap::<String, usize>::new();
        let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);

        while let Some(child_id) = child {
            let node = engine.nodes.get(child_id).expect("child id should exist");
            let key = child_key(node.node_data());
            *counts.entry(key).or_default() += 1;
            stack.push(child_id);
            child = node.node_data().next_sibling;
        }

        let duplicates = counts
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .collect::<Vec<_>>();
        assert!(
            duplicates.is_empty(),
            "node {:?} should not have duplicate child keys: {:?}",
            parent,
            duplicates
        );
    }
}

fn direct_child_count(engine: &crate::app::AppEngine, parent: NodeId) -> usize {
    let mut count = 0;
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        count += 1;
        child = engine.nodes.get(child_id).and_then(|node| node.node_data().next_sibling);
    }
    count
}

fn param_value(engine: &crate::app::AppEngine, node: NodeId) -> ParamValue {
    engine
        .nodes
        .get(node)
        .and_then(|node| node.engine_param_snapshot())
        .map(|snapshot| snapshot.value)
        .expect("parameter value should exist")
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        let meta = &node.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }

    None
}

fn child_key(node_data: &golden_core::node::NodeData) -> String {
    node_data
        .meta
        .decl_id
        .0
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or({
            if !node_data.meta.short_name.is_empty() {
                node_data.meta.short_name.as_str()
            } else {
                node_data.meta.label.as_str()
            }
        })
        .to_string()
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    let mut remaining = path.trim_matches('/');

    loop {
        if remaining.is_empty() {
            return Some(current);
        }

        if let Some(found) = find_child_by_key(engine, current, remaining) {
            return Some(found);
        }

        let Some((segment, tail)) = remaining.split_once('/') else {
            return find_child_by_key(engine, current, remaining);
        };
        current = find_child_by_key(engine, current, segment)?;
        remaining = tail;
    }
}
