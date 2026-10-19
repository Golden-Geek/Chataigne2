use golden_core::parameter::ParamValue;

use super::*;

#[test]
fn oversized_line_rejection_preserves_surrounding_frames_and_split_crlf() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig::default();
    let mut bytes = b"before 1\n".to_vec();
    bytes.extend(std::iter::repeat_n(b'x', MAX_PENDING_LINE_BYTES + 1));
    let first = parser.push_bytes(&bytes, &config).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].path_segments, ["before"]);
    assert_eq!(parser.take_rejected_line_count(), 1);
    assert!(parser.push_bytes(b"discarded suffix\r", &config).unwrap().is_empty());
    assert!(parser.push_bytes(b"", &config).unwrap().is_empty());
    let after = parser.push_bytes(b"\nafter 2\n", &config).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].path_segments, ["after"]);
    assert_eq!(parser.take_rejected_line_count(), 0, "reject each overlong frame once");
}

#[test]
fn parser_configuration_change_rejects_partial_old_grammar() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig::default();
    assert!(parser.push_bytes(b"old partial", &config).unwrap().is_empty());
    let changed = StreamingParseConfig {
        name_separator: Some(StreamingSeparator::Colon),
        ..config
    };
    let messages = parser.push_bytes(b"new:42\n", &changed).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].path_segments, ["new"]);
    assert_eq!(messages[0].payload, ReceivedValuePayload::Single(ParamValue::Int(42)));
    assert_eq!(parser.take_rejected_line_count(), 1);
}

#[test]
fn parser_accepts_a_line_at_the_exact_retained_limit() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig {
        name_separator: None,
        value_separator: None,
        ..Default::default()
    };
    for _ in 0..1024 {
        assert!(parser.push_bytes(&[b'x'; 1024], &config).unwrap().is_empty());
    }
    let messages = parser.push_bytes(b"\n", &config).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(
        matches!(&messages[0].payload, ReceivedValuePayload::Single(ParamValue::Str(value)) if value.len() == MAX_PENDING_LINE_BYTES)
    );
    assert_eq!(parser.pending_line_bytes.capacity(), MAX_PENDING_LINE_BYTES);
    assert_eq!(parser.take_rejected_line_count(), 0);
}

#[test]
fn stream_parser_bounds_an_unterminated_line_across_input_chunks() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig::default();
    let chunk = vec![b'x'; 65_536];
    for _ in 0..32 {
        let _ = parser.push_bytes(&chunk, &config);
    }
    assert!(
        parser.pending_line_bytes.capacity() <= 1024 * 1024,
        "unterminated input must have a retained-byte bound"
    );
}

#[test]
fn line_parser_splits_named_hierarchy() {
    let config = StreamingParseConfig::default();
    let message = parse_line(b"my.received.value 42\n", &config).expect("line should parse");

    assert_eq!(message.path_segments, vec!["my", "received", "value"]);
    assert_eq!(message.payload, ReceivedValuePayload::Single(ParamValue::Int(42)));
}

#[test]
fn line_parser_supports_separate_name_and_value_separators() {
    let config = StreamingParseConfig {
        value_separator: Some(StreamingSeparator::Comma),
        ..StreamingParseConfig::default()
    };
    let message = parse_line(b"my.value cool,3\n", &config).expect("line should parse");

    assert_eq!(message.path_segments, vec!["my", "value"]);
    assert_eq!(
        message.payload,
        ReceivedValuePayload::Multi(vec![ParamValue::Str("cool".to_string()), ParamValue::Int(3),])
    );
}

#[test]
fn line_parser_uses_default_name_when_name_separator_is_disabled() {
    let config = StreamingParseConfig {
        name_separator: None,
        ..StreamingParseConfig::default()
    };
    let message = parse_line(b"1.5:2.5\n", &config).expect("line should parse");

    assert_eq!(message.path_segments, vec![DEFAULT_LINE_VALUE_NAME]);
    assert_eq!(
        message.payload,
        ReceivedValuePayload::Single(ParamValue::Vec2(1.5, 2.5))
    );
}

#[test]
fn line_parser_keeps_value_text_whole_when_value_separator_is_disabled() {
    let config = StreamingParseConfig {
        value_separator: None,
        ..StreamingParseConfig::default()
    };
    let message = parse_line(b"message hello,3\n", &config).expect("line should parse");

    assert_eq!(message.path_segments, vec!["message"]);
    assert_eq!(
        message.payload,
        ReceivedValuePayload::Single(ParamValue::Str("hello,3".to_string()))
    );
}

#[test]
fn raw_mode_emits_one_message_per_byte() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig {
        mode: STREAMING_INPUT_MODE_RAW.to_string(),
        ..StreamingParseConfig::default()
    };

    let messages = parser
        .push_bytes(&[0x01, 0xFF], &config)
        .expect("raw bytes should parse");

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].payload, ReceivedValuePayload::Single(ParamValue::Int(1)));
    assert_eq!(messages[1].payload, ReceivedValuePayload::Single(ParamValue::Int(255)));
}

#[test]
fn json_mode_flattens_nested_objects_into_message_paths() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig {
        mode: STREAMING_INPUT_MODE_JSON.to_string(),
        ..StreamingParseConfig::default()
    };

    let messages = parser
        .push_bytes(br#"{"transport":{"connected":true,"latency":12},"name":"ws"}"#, &config)
        .expect("json payload should parse");

    assert_eq!(messages.len(), 3);
    assert!(messages.iter().any(|message| {
        message.path_segments == vec!["transport", "connected"]
            && message.payload == ReceivedValuePayload::Single(ParamValue::Bool(true))
    }));
    assert!(messages.iter().any(|message| {
        message.path_segments == vec!["transport", "latency"]
            && message.payload == ReceivedValuePayload::Single(ParamValue::Int(12))
    }));
    assert!(messages.iter().any(|message| {
        message.path_segments == vec!["name"]
            && message.payload == ReceivedValuePayload::Single(ParamValue::Str("ws".to_string()))
    }));
}

#[test]
fn json_mode_maps_root_scalar_to_received() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig {
        mode: STREAMING_INPUT_MODE_JSON.to_string(),
        ..StreamingParseConfig::default()
    };

    let messages = parser.push_bytes(b"42", &config).expect("json scalar should parse");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].path_segments, vec![DEFAULT_LINE_VALUE_NAME]);
    assert_eq!(messages[0].payload, ReceivedValuePayload::Single(ParamValue::Int(42)));
}

#[test]
fn json_mode_keeps_scalar_arrays_as_multi_value_payloads() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig {
        mode: STREAMING_INPUT_MODE_JSON.to_string(),
        ..StreamingParseConfig::default()
    };

    let messages = parser
        .push_bytes(br#"{"levels":[1,2,3,4,5]}"#, &config)
        .expect("json array payload should parse");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].path_segments, vec!["levels"]);
    assert_eq!(
        messages[0].payload,
        ReceivedValuePayload::Multi(vec![
            ParamValue::Int(1),
            ParamValue::Int(2),
            ParamValue::Int(3),
            ParamValue::Int(4),
            ParamValue::Int(5),
        ])
    );
}

#[test]
fn line_parser_accepts_cr_lf_and_crlf_line_breaks() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig::default();

    let messages = parser
        .push_bytes(b"first 1\r\nsecond 2\nthird 3\r", &config)
        .expect("lines should parse");

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].path_segments, vec!["first"]);
    assert_eq!(messages[1].path_segments, vec!["second"]);
    assert_eq!(messages[2].path_segments, vec!["third"]);
}

#[test]
fn line_parser_treats_split_crlf_as_one_line_break() {
    let mut parser = StreamingParser::default();
    let config = StreamingParseConfig::default();

    let first = parser
        .push_bytes(b"first 1\r", &config)
        .expect("first line should parse");
    let second = parser
        .push_bytes(b"\nsecond 2", &config)
        .expect("split LF should be consumed");
    let third = parser.push_bytes(b"\n", &config).expect("second line should parse");

    assert_eq!(first.len(), 1);
    assert!(second.is_empty());
    assert_eq!(third.len(), 1);
    assert_eq!(first[0].path_segments, vec!["first"]);
    assert_eq!(third[0].path_segments, vec!["second"]);
}
