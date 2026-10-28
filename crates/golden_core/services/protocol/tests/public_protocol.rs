use golden_protocol::{UI_PROTOCOL_VERSION, UiClientMessage, UiDiscoveryDto, UiSnapshot};
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
fn discovery_identity_round_trips_without_an_engine_or_host_runtime() {
    let wire = serde_json::json!({
        "version": 1,
        "service": "golden-ui",
        "health_path": "/api/ui/health",
        "websocket_path": "/api/ui/ws",
        "relative_endpoints": true,
        "application": {
            "id": "fixture-app",
            "display_name": "Fixture Application",
            "version": "2.3.4",
            "data_directory_name": "FixtureData",
        },
        "protocol_version": UI_PROTOCOL_VERSION,
    });
    let discovery: UiDiscoveryDto = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(discovery).unwrap(), wire);

    for required in ["application", "protocol_version"] {
        let mut incomplete = wire.clone();
        incomplete.as_object_mut().unwrap().remove(required);
        assert!(serde_json::from_value::<UiDiscoveryDto>(incomplete).is_err());
    }
}

#[test]
fn snapshot_requires_authoritative_application_project_file_metadata() {
    let mut wire = serde_json::json!({
        "protocol_version": UI_PROTOCOL_VERSION,
        "scope": "wholeGraph",
        "at": {"tick": 0, "micro": 0, "seq": 0},
        "project_generation": 1,
        "nodes": [],
        "schema": {},
        "history": {
            "can_undo": false, "can_redo": false, "undo_len": 0, "redo_len": 0,
            "active_edit_session": false, "current_history_state_id": 0,
        },
        "logger": {"max_entries": 100},
        "project_file": {"display_name": "Fixture Project", "extension": "fixture"},
    });
    let snapshot: UiSnapshot = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(snapshot.project_file.display_name, "Fixture Project");
    assert_eq!(snapshot.project_file.extension, "fixture");

    wire.as_object_mut().unwrap().remove("project_file");
    let error = serde_json::from_value::<UiSnapshot>(wire).unwrap_err();
    assert!(error.to_string().contains("project_file"), "{error}");
}

#[test]
fn dashboard_layout_requests_round_trip_without_an_engine_runtime() {
    for wire in [
        serde_json::json!({"kind":"moveDashboardWidget","widget":7,"parent":2,"target_index":0}),
        serde_json::json!({"kind":"resizeDashboardWidget","widget":7,"width":{"value":25.0,"unit":"percent"}}),
    ] {
        let intent: UiEditIntent = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(intent).unwrap(), wire);
    }
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
