use std::fmt::Write;

pub(super) fn canonical_edid(bytes: &[u8]) -> Option<String> {
    let base = bytes.get(..128)?;
    if base[..8] != [0, 255, 255, 255, 255, 255, 255, 0]
        || base.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)) != 0
    {
        return None;
    }
    // Keep the complete base block, including both numeric and descriptor serial fields.
    // Equal blocks from panels without unique serials remain equal; matching is app policy.
    let mut encoded = String::with_capacity(256);
    for byte in base {
        write!(encoded, "{byte:02x}").expect("formatting into String cannot fail");
    }
    Some(encoded)
}

pub(super) fn display_index(gdi_name: &str) -> u32 {
    gdi_name
        .to_ascii_uppercase()
        .strip_prefix(r"\\.\DISPLAY")
        .and_then(|number| number.parse::<u32>().ok())
        .unwrap_or(0)
}
