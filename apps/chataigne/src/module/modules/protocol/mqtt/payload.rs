pub(super) fn mqtt_payload_script_arg(payload: &[u8]) -> serde_json::Value {
    std::str::from_utf8(payload)
        .map(|text| serde_json::json!(text))
        .unwrap_or_else(|_| crate::app::module::script_api::bytes_arg(payload))
}

pub(super) fn format_mqtt_payload_for_log(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "<empty>".to_string();
    }

    match std::str::from_utf8(payload) {
        Ok(text) if !text.chars().any(char::is_control) => format!("\"{text}\""),
        _ => payload
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}
