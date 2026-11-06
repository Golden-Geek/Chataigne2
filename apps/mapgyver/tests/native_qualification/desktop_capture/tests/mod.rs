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
