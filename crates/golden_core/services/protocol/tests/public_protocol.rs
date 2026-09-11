use golden_protocol::{UI_PROTOCOL_VERSION, UiClientMessage};

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
