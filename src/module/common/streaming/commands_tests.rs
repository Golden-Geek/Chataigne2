use golden_core::parameter::ParamValue;

#[test]
fn streaming_script_send_text_decodes_to_string_request() {
    let request = super::streaming_script_send_request("sendText", &[ParamValue::Str("hello".to_string())])
        .expect("sendText should be a streaming script method")
        .expect("sendText args should decode");

    assert_eq!(request.description, "string");
    assert_eq!(request.bytes, b"hello".to_vec());
}

#[test]
fn streaming_script_send_bytes_accepts_numeric_arguments() {
    let request = super::streaming_script_send_request(
        "sendBytes",
        &[
            ParamValue::Int(0xF0),
            ParamValue::Int(0x7D),
            ParamValue::Int(0x01),
            ParamValue::Int(0xF7),
        ],
    )
    .expect("sendBytes should be a streaming script method")
    .expect("sendBytes args should decode");

    assert_eq!(request.description, "bytes");
    assert_eq!(request.bytes, vec![0xF0, 0x7D, 0x01, 0xF7]);
}
