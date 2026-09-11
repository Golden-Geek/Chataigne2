use serde_json::json;

use super::QuickJsRuntime;
use crate::ScriptEvent;

#[test]
fn custom_event_payload_can_target_named_script_callback() {
    let event = ScriptEvent {
        kind: "custom".to_string(),
        origin: None,
        old_value: None,
        payload: json!({
            "Custom": {
                "topic": "test.module.script.callback",
                "payload": {
                    "callback": "messageReceived",
                    "args": ["/address", [1, 2, 3]]
                }
            }
        }),
    };

    let invocation = event
        .custom_callback_invocation()
        .expect("custom module callback payload should decode");

    assert_eq!(invocation.name, "messageReceived");
    assert_eq!(invocation.args, vec![json!("/address"), json!([1, 2, 3])]);
}

#[test]
fn custom_event_payload_ignores_empty_callback_name() {
    let event = ScriptEvent {
        kind: "custom".to_string(),
        origin: None,
        old_value: None,
        payload: json!({
            "Custom": {
                "payload": {
                    "callback": "  ",
                    "args": ["ignored"]
                }
            }
        }),
    };

    assert!(event.custom_callback_invocation().is_none());
}

#[test]
fn callback_node_argument_marker_decodes_node_id() {
    assert_eq!(
        QuickJsRuntime::script_callback_arg_node_id(&json!({
            "kind": "node",
            "id": 42
        })),
        Some(42)
    );
    assert_eq!(
        QuickJsRuntime::script_callback_arg_node_id(&json!({
            "kind": "value",
            "id": 42
        })),
        None
    );
}
