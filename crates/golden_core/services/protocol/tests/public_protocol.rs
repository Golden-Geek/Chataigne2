use golden_protocol::{UI_PROTOCOL_VERSION, UiClientMessage};
use golden_protocol::{UiAck, UiAppOperationResult, UiEditIntent};

#[test]
fn public_protocol_serializes_without_an_engine_runtime() {
    let message = UiClientMessage::Hello {
        protocol_version: UI_PROTOCOL_VERSION.to_string(),
        client_instance_id: Some("fixture-client".to_string()),
    };

    let encoded = serde_json::to_string(&message).expect("public protocol message should encode");

    assert!(encoded.contains(UI_PROTOCOL_VERSION));
    assert!(encoded.contains("fixture-client"));
}

#[test]
fn app_operation_requests_and_exact_results_round_trip_without_an_engine_runtime() {
    let request = UiEditIntent::InvokeAppOperation {
        namespace: "fixture".into(),
        operation: "paste".into(),
        payload: serde_json::json!({"source":99,"position":[1.5,2.5]}),
    };
    let wire = serde_json::to_value(&request).unwrap();
    assert_eq!(wire["kind"], "invokeAppOperation");
    assert_eq!(serde_json::from_value::<UiEditIntent>(wire).unwrap(), request);
    let baseline = serde_json::json!({
        "success":true,"status":"applied",
        "history":{"can_undo":false,"can_redo":false,"undo_len":0,"redo_len":0,
            "active_edit_session":false,"current_history_state_id":0}
    });
    let mut ack: UiAck = serde_json::from_value(baseline).unwrap();
    assert!(
        !serde_json::to_value(&ack)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("result")
    );
    for result in [
        serde_json::Value::Null,
        serde_json::json!({"source":99,"created":123,"uuid":"authoritative"}),
    ] {
        ack.result = Some(UiAppOperationResult { value: result.clone() });
        let encoded = serde_json::to_value(&ack).unwrap();
        assert!(encoded.as_object().unwrap().contains_key("result"));
        assert_eq!(encoded["result"]["value"], result);
        assert_eq!(
            serde_json::from_value::<UiAck>(encoded).unwrap().result.unwrap().value,
            result
        );
    }
}
