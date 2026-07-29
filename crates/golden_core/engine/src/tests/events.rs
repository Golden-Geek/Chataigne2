use std::sync::Arc;

use crate::events::CustomEvent;

#[test]
fn custom_event_clones_share_json_payload_storage() {
    let event = CustomEvent::new(
        "test.large_payload",
        None,
        serde_json::json!({
            "executions": (0..256)
                .map(|index| serde_json::json!({
                    "target": index,
                    "arguments": [index, index + 1, index + 2],
                }))
                .collect::<Vec<_>>(),
        }),
    );

    let cloned = event.clone();

    assert!(Arc::ptr_eq(&event.payload, &cloned.payload));
    assert_eq!(event.payload, cloned.payload);
}
