use super::*;

#[test]
fn script_source_admission_precedes_string_conversion_and_counts_retained_capacity() {
    let mut oversized = String::with_capacity(super::super::bounds::MAX_SOURCE_BYTES + 1);
    oversized.push('1');
    let args = [ParamValue::Str(oversized)];
    assert!(streaming_script_send_request("sendBytes", &args).unwrap().is_err());
    assert!(streaming_script_send_request("unknown", &args).is_none());
    let values = vec![ParamValue::Int(1); MAX_COMMAND_VALUES + 1];
    assert!(streaming_script_send_request("sendBytes", &values).unwrap().is_err());
}

#[test]
fn encoded_output_limits_include_escape_expansion_separators_and_json_escaping() {
    assert!(string_request(&"a".repeat(MAX_COMMAND_BYTES + 1), LINE_ENDING_NONE).is_err());
    let values = vec![ParamValue::Int(1); MAX_COMMAND_VALUES];
    assert!(values_request(&values, "", "", &"a".repeat(1024)).is_err());
    assert!(values_json_request(&serde_json::Value::String("\"".repeat(MAX_COMMAND_BYTES))).is_err());
    assert!(hex_string_request(&"00".repeat(MAX_COMMAND_BYTES + 1)).is_err());
    let full = ParamValue::Str("0 ".repeat(MAX_COMMAND_BYTES));
    assert!(script_bytes_from_args(&[full, ParamValue::Int(1)]).is_err());
}

#[test]
fn byte_token_feedback_and_json_depth_are_bounded_before_encoding() {
    let error = bytes_request(&"9".repeat(1024 * 1024)).unwrap_err();
    assert!(error.len() < 256);
    let mut value = serde_json::json!(1);
    for _ in 0..40 {
        value = serde_json::Value::Array(vec![value]);
    }
    assert!(values_json_request(&value).is_err());
    let value = serde_json::Value::Array(Vec::with_capacity(100_000));
    assert!(super::super::bounds::json_weight(&value, 4096, 1024, 16).is_err());
}
