use super::*;

fn request() -> ScriptAppOperationRequest {
    ScriptAppOperationRequest::reserve("copy-1", "fixture", "copy")
        .unwrap()
        .finish(json!({ "source": 42 }))
        .unwrap()
}

#[test]
fn exact_callback_schema_matches_the_rust_dto_and_preserves_explicit_null() {
    let admitted = ScriptAppOperationRequest::admit_result(Value::Null).unwrap();
    let callback = request().into_callback(Ok(admitted));
    assert_eq!(callback.arc()["callback"], "onAppOperationResult");
    let reply = &callback.arc()["args"][0];
    let expected = ScriptAppOperationReply::Applied {
        request_id: "copy-1".into(),
        value: Value::Null,
    };
    assert_eq!(*reply, serde_json::to_value(&expected).unwrap());
    assert_eq!(
        serde_json::from_value::<ScriptAppOperationReply>(reply.clone()).unwrap(),
        expected
    );
}

#[test]
fn callback_moves_large_result_allocations_without_cloning_the_authored_value() {
    let text = "retained-content".repeat(1000);
    let value = json!({ "content": text });
    let pointer = value["content"].as_str().unwrap().as_ptr();
    let admitted = ScriptAppOperationRequest::admit_result(value).unwrap();
    let callback = request().into_callback(Ok(admitted));
    assert_eq!(
        callback.arc()["args"][0]["value"]["content"].as_str().unwrap().as_ptr(),
        pointer
    );
}

#[test]
fn rejected_callback_bounds_utf8_errors_and_cannot_be_mistaken_for_success() {
    let callback = request().into_callback(Err("é".repeat(10_000)));
    let reply = &callback.arc()["args"][0];
    assert_eq!(reply["status"], "rejected");
    assert!(reply.get("value").is_none());
    assert_eq!(reply["message"].as_str().unwrap().len(), 4096);
    assert!(serde_json::from_value::<ScriptAppOperationReply>(reply.clone()).is_ok());
}

#[test]
fn app_json_has_its_own_bounds_instead_of_the_small_method_argument_budget() {
    let wide = Value::Array(vec![Value::Null; 5000]);
    assert!(
        ScriptAppOperationRequest::reserve("wide", "fixture", "paste")
            .unwrap()
            .finish(wide)
            .is_ok()
    );
    for name in ["".into(), " ".into(), "x".repeat(129)] {
        assert!(ScriptAppOperationRequest::reserve(&name, "fixture", "paste").is_err());
    }
    let mut nested = Value::Null;
    for _ in 0..SCRIPT_APP_OPERATION_DEPTH {
        nested = Value::Array(vec![nested]);
    }
    assert!(ScriptAppOperationRequest::admit_result(nested).is_err());
}

#[test]
fn reservations_include_response_capacity_and_release_on_failed_preparation() {
    let mut held = Vec::new();
    while let Ok(reservation) = ScriptAppOperationRequest::reserve("held", "fixture", "copy") {
        held.push(reservation);
        assert!(held.len() <= 8);
    }
    assert!(!held.is_empty());
    drop(held);
    assert!(ScriptAppOperationRequest::reserve("recovered", "fixture", "copy").is_ok());
}
