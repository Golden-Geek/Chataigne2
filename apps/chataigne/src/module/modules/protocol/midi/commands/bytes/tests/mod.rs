use super::*;

#[test]
fn decimal_hex_separators_and_compact_hex_share_one_bounded_parser() {
    assert_eq!(parse_midi_bytes("144,60;127").unwrap(), [0x90, 60, 127]);
    assert_eq!(parse_midi_bytes("0xF0 01 f7").unwrap(), [0xF0, 1, 0xF7]);
    assert_eq!(parse_midi_bytes("F001F7").unwrap(), [0xF0, 1, 0xF7]);
    assert!(parse_midi_bytes("0xGG").is_err());
}
#[test]
fn text_and_aggregate_output_are_bounded_before_further_growth() {
    assert!(parse_midi_bytes(&" ".repeat(MAX_TEXT + 1)).is_err());
    let mut output = vec![0; MAX_BYTES - 1];
    append_midi_bytes(&mut output, "1").unwrap();
    assert_eq!(output.capacity(), MAX_BYTES);
    assert!(append_midi_bytes(&mut output, "2").is_err());
    assert_eq!(output.len(), MAX_BYTES);
    let result = parse_midi_bytes(&"FF".repeat(MAX_BYTES));
    assert_eq!(result.unwrap().len(), MAX_BYTES);
}
#[test]
fn sysex_reserves_exact_storage_and_admits_the_complete_framed_message() {
    let bytes = normalize_midi_sysex(&vec![1; MAX_BYTES - 2]).unwrap();
    assert_eq!((bytes.len(), bytes.capacity()), (MAX_BYTES, MAX_BYTES));
    assert_eq!((bytes[0], bytes[MAX_BYTES - 1]), (0xF0, 0xF7));
    assert!(normalize_midi_sysex(&vec![1; MAX_BYTES - 1]).is_err());
    assert_eq!(normalize_midi_sysex(&[0xF0, 1, 0xF7]).unwrap(), [0xF0, 1, 0xF7]);
}
