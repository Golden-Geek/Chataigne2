use super::*;

fn config(mode: &str) -> StreamingParseConfig {
    StreamingParseConfig {
        mode: mode.into(),
        ..Default::default()
    }
}

#[test]
fn raw_expansion_rejects_an_oversized_frame_before_preparing_messages() {
    let mut parser = StreamingParser::default();
    assert!(parser
        .push_bytes(&vec![1; 65_537], &config(STREAMING_INPUT_MODE_RAW))
        .is_err());
    assert_eq!(
        parser
            .push_bytes(&[2, 3], &config(STREAMING_INPUT_MODE_RAW))
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn json_input_size_is_checked_before_whitespace_or_decode() {
    let mut parser = StreamingParser::default();
    assert!(parser
        .push_bytes(&vec![b' '; 1024 * 1024 + 1], &config(STREAMING_INPUT_MODE_JSON))
        .is_err());
}

#[test]
fn json_paths_beyond_the_graph_service_depth_are_rejected_as_one_frame() {
    let mut parser = StreamingParser::default();
    let input = format!("{}0{}", "{\"a\":".repeat(65), "}".repeat(65));
    assert!(parser
        .push_bytes(input.as_bytes(), &config(STREAMING_INPUT_MODE_JSON))
        .is_err());
    let messages = parser
        .push_bytes(br#"{"next":1}"#, &config(STREAMING_INPUT_MODE_JSON))
        .unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].path_segments, ["next"]);
}

#[test]
fn json_path_amplification_is_rejected_before_publishing_a_partial_frame() {
    let children: serde_json::Map<String, serde_json::Value> = (0..2200)
        .map(|index| (format!("child{index}"), serde_json::json!(index)))
        .collect();
    let input = serde_json::to_vec(&serde_json::json!({ "p".repeat(4000): children })).unwrap();
    assert!(input.len() < 1024 * 1024);
    let mut parser = StreamingParser::default();
    assert!(parser.push_bytes(&input, &config(STREAMING_INPUT_MODE_JSON)).is_err());
}

#[test]
fn json_large_lists_that_fit_the_byte_budget_remain_valid() {
    let input = format!("[{}0]", "0,".repeat(99_999));
    let messages = StreamingParser::default()
        .push_bytes(input.as_bytes(), &config(STREAMING_INPUT_MODE_JSON))
        .unwrap();
    assert_eq!(messages.len(), 1);
    assert!(messages.retained_bytes() <= 16 * 1024 * 1024);
    assert!(matches!(&messages[0].payload, ReceivedValuePayload::Multi(values) if values.len() == 100_000));
}

#[test]
fn json_too_many_list_values_are_rejected_before_value_vector_allocation() {
    let count = crate::app::module::common::streaming::admission::MAX_VALUE_UNITS + 1;
    let input = format!("[{}0]", "0,".repeat(count - 1));
    assert!(input.len() <= 1024 * 1024);
    assert!(StreamingParser::default()
        .push_bytes(input.as_bytes(), &config(STREAMING_INPUT_MODE_JSON))
        .is_err());
}

#[test]
fn json_path_bytes_and_provenance_are_bounded_without_cutting_utf8() {
    let key = "é".repeat(2048);
    let input = serde_json::to_vec(&serde_json::json!({ key.clone(): 42 })).unwrap();
    let messages = StreamingParser::default()
        .push_bytes(&input, &config(STREAMING_INPUT_MODE_JSON))
        .unwrap();
    assert_eq!(messages[0].path_segments, [key]);
    assert!(messages[0].source_description.len() <= 4096);
    assert!(messages[0].source_description.ends_with('\''));
    let overlong_key = "é".repeat(2049);
    let invalid = serde_json::to_vec(&serde_json::json!({ overlong_key: 42 })).unwrap();
    assert!(StreamingParser::default()
        .push_bytes(&invalid, &config(STREAMING_INPUT_MODE_JSON))
        .is_err());
}

#[test]
fn line_preparation_rejects_one_invalid_path_and_keeps_surrounding_frames() {
    let mut parser = StreamingParser::default();
    let invalid_path = format!("{}leaf", "a/".repeat(64));
    let input = format!("before 1\n{invalid_path} 2\nafter 3\n");
    let messages = parser
        .push_bytes(input.as_bytes(), &StreamingParseConfig::default())
        .unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].path_segments, ["before"]);
    assert_eq!(messages[1].path_segments, ["after"]);
    assert_eq!(parser.take_rejected_line_count(), 1);
}

#[test]
fn line_batch_expansion_is_bounded_and_a_later_input_turn_recovers() {
    let mut parser = StreamingParser::default();
    let messages = parser
        .push_bytes("pulse\n".repeat(65_537).as_bytes(), &StreamingParseConfig::default())
        .unwrap();
    assert!(messages.len() <= 65_536);
    assert!(messages.retained_bytes() <= 16 * 1024 * 1024);
    assert_eq!(messages.len() as u64 + parser.take_rejected_line_count(), 65_537);
    let next = parser
        .push_bytes(b"next 1\n", &StreamingParseConfig::default())
        .unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].path_segments, ["next"]);
}

#[test]
fn line_preparation_counts_all_fields_before_converting_a_large_list() {
    let count = crate::app::module::common::streaming::admission::MAX_VALUE_UNITS + 1;
    let input = format!("list {}0\nnext 1\n", "0:".repeat(count - 1));
    let mut parser = StreamingParser::default();
    let messages = parser
        .push_bytes(input.as_bytes(), &StreamingParseConfig::default())
        .unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].path_segments, ["next"]);
    assert_eq!(parser.take_rejected_line_count(), 1);
}

#[test]
fn prepared_messages_include_spare_vector_and_string_capacity() {
    use crate::app::module::common::streaming::admission::PreparedMessages;
    let mut values = Vec::with_capacity(8192);
    values.push(ReceivedValueMessage {
        path_segments: vec!["value".into()],
        payload: ReceivedValuePayload::Single(ParamValue::Int(1)),
        source_description: String::new(),
    });
    let prepared = PreparedMessages::try_from(values).unwrap();
    assert!(prepared.retained_bytes() >= 8192 * std::mem::size_of::<ReceivedValueMessage>());
    let mut text = String::with_capacity(16 * 1024 * 1024);
    text.push('x');
    let value = ReceivedValueMessage {
        path_segments: vec!["value".into()],
        payload: ReceivedValuePayload::Single(ParamValue::Str(text)),
        source_description: String::new(),
    };
    assert!(PreparedMessages::try_from(vec![value]).is_err());
}
