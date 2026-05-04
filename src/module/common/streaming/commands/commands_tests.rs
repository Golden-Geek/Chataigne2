use super::*;

#[test]
fn bytes_request_accepts_decimal_and_hex_tokens() {
    let request = bytes_request("1 0x02,255").expect("bytes should parse");

    assert_eq!(request.bytes, vec![1, 2, 255]);
    assert_eq!(request.frame_kind, StreamingSendFrameKind::Binary);
}

#[test]
fn hex_string_request_ignores_spacing() {
    let request = hex_string_request("48 65 6c 6c 6f").expect("hex should parse");

    assert_eq!(request.bytes, b"Hello");
    assert_eq!(request.frame_kind, StreamingSendFrameKind::Text);
}

#[test]
fn string_request_uses_text_frames() {
    let request = string_request("hello", LINE_ENDING_NONE);

    assert_eq!(request.bytes, b"hello");
    assert_eq!(request.frame_kind, StreamingSendFrameKind::Text);
}

#[test]
fn values_request_uses_binary_frames() {
    let request = values_request(&[ParamValue::Int(1), ParamValue::Bool(true)], "", "", ",");

    assert_eq!(request.bytes, b"1,true");
    assert_eq!(request.frame_kind, StreamingSendFrameKind::Binary);
}

#[test]
fn values_json_request_uses_text_frames() {
    let request = values_json_request(&serde_json::json!({ "answer": 42 }))
        .expect("json values request should encode");

    assert_eq!(request.bytes, br#"{"answer":42}"#);
    assert_eq!(request.frame_kind, StreamingSendFrameKind::Text);
}