use super::*;

#[test]
fn commands_reject_zero_identity_unknown_fields_and_unknown_operations() {
    for value in [
        r#"{"kind":"Stamp","id":0}"#,
        r#"{"kind":"Stamp","id":1,"extra":true}"#,
        r#"{"kind":"Input"}"#,
    ] {
        assert!(serde_json::from_str::<Request>(value).is_err());
    }
    assert!(matches!(
        serde_json::from_str::<Request>(r#"{"kind":"Stamp","id":1}"#).unwrap(),
        Request::Stamp { .. }
    ));
}

#[test]
fn qpc_values_cross_json_as_exact_decimal_strings_not_javascript_numbers() {
    let event = Event::Stamped {
        id: 1,
        qpc: u64::MAX.to_string(),
    };
    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["qpc"], u64::MAX.to_string());
}

#[test]
fn line_reader_stops_at_the_fixed_command_bound_before_deserializing() {
    let bytes = vec![b'x'; 10_000];
    let mut reader = std::io::Cursor::new(bytes);
    assert!(read_request(&mut reader).is_err());
    assert_eq!(reader.position(), 257);
}

#[test]
fn desktop_mask_notice_crosses_the_capture_protocol_without_erasing_the_owned_pixels() {
    let event = Event::Frame {
        serial: 1,
        acquire_begin_qpc: "1".into(),
        readback_end_qpc: "3".into(),
        desktop_update_qpc: "2".into(),
        accumulated_frames: 1,
        protected_content_masked_out: true,
        bgra: vec![0, 0, 255, 255],
    };
    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["protected_content_masked_out"], true);
    assert_eq!(value["bgra"], serde_json::json!([0, 0, 255, 255]));
}
