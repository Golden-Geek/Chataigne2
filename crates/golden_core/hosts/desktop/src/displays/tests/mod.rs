use super::metadata::{canonical_edid, display_index};

fn edid() -> [u8; 128] {
    let mut bytes = [0_u8; 128];
    bytes[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    bytes[8..16].copy_from_slice(&[0x4c, 0x2d, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12]);
    bytes[18..20].copy_from_slice(&[1, 4]);
    bytes[54..59].copy_from_slice(&[0, 0, 0, 0xff, 0]);
    bytes[59..72].copy_from_slice(b"SERIAL-12345\n");
    checksum(&mut bytes);
    bytes
}
fn checksum(bytes: &mut [u8; 128]) {
    bytes[127] = 0_u8.wrapping_sub(bytes[..127].iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)));
}

#[test]
fn valid_edid_retains_numeric_and_text_serials_in_a_canonical_base_block() {
    let base = edid();
    let encoded = canonical_edid(&base).unwrap();
    assert_eq!(encoded.len(), 256);
    assert!(encoded.starts_with("00ffffffffffff004c2d341278563412"));
    assert!(encoded.contains("53455249414c2d31323334350a"));
    assert!(
        encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let mut extended = base.to_vec();
    extended.extend_from_slice(&[0xcc; 128]);
    assert_eq!(canonical_edid(&extended), Some(encoded));
}

#[test]
fn malformed_or_incomplete_edid_is_optional_and_never_becomes_a_fabricated_identity() {
    let base = edid();
    assert_eq!(canonical_edid(&base[..127]), None);
    assert_eq!(canonical_edid(&[]), None);
    let mut corrupt = base;
    corrupt[9] ^= 1;
    assert_eq!(canonical_edid(&corrupt), None);
    corrupt[0] = 1;
    checksum(&mut corrupt);
    assert_eq!(canonical_edid(&corrupt), None);
}

#[test]
fn equal_panel_blocks_remain_ambiguous_while_real_serial_differences_are_preserved() {
    let first = edid();
    assert_eq!(canonical_edid(&first), canonical_edid(&first));
    let mut second = first;
    second[12] += 1;
    checksum(&mut second);
    assert_ne!(canonical_edid(&first), canonical_edid(&second));
}

#[test]
fn display_number_comes_only_from_a_recognized_os_gdi_source_name() {
    assert_eq!(display_index(r"\\.\DISPLAY12"), 12);
    assert_eq!(display_index(r"\\.\display3"), 3);
    assert_eq!(display_index("DISPLAY1"), 0);
    assert_eq!(display_index(r"\\.\DISPLAY1\Monitor0"), 0);
    assert_eq!(display_index(r"\\.\DISPLAY4294967296"), 0);
}
