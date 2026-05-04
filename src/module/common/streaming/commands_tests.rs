use super::*;

#[test]
fn bytes_request_accepts_decimal_and_hex_tokens() {
    let request = bytes_request("1 0x02,255").expect("bytes should parse");

    assert_eq!(request.bytes, vec![1, 2, 255]);
}

#[test]
fn hex_string_request_ignores_spacing() {
    let request = hex_string_request("48 65 6c 6c 6f").expect("hex should parse");

    assert_eq!(request.bytes, b"Hello");
}
