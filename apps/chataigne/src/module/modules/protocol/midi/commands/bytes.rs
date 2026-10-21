const MAX_BYTES: usize = 64 * 1024;
const MAX_TEXT: usize = 256 * 1024;

#[cfg(test)]
mod tests;

pub(crate) fn parse_midi_bytes(text: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    append_midi_bytes(&mut output, text)?;
    Ok(output)
}
pub(crate) fn append_midi_bytes(output: &mut Vec<u8>, text: &str) -> Result<(), String> {
    if output.capacity() > MAX_BYTES {
        return Err("MIDI output retains more than 64 KiB".into());
    }
    if text.len() > MAX_TEXT {
        return Err("MIDI byte source exceeds 256 KiB".into());
    }
    let mut tokens = text
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|s| !s.is_empty());
    let Some(first) = tokens.next() else { return Ok(()) };
    let second = tokens.next();
    if second.is_none()
        && first.len() > 2
        && first.len().is_multiple_of(2)
        && first.bytes().all(|b| b.is_ascii_hexdigit())
        && first.bytes().any(|b| b.is_ascii_alphabetic())
    {
        for pair in first.as_bytes().chunks_exact(2) {
            push(
                output,
                u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16).expect("two hex digits"),
            )?;
        }
    } else {
        for token in std::iter::once(first).chain(second).chain(tokens) {
            let value = if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
                u8::from_str_radix(hex, 16)
            } else if token.bytes().any(|b| b.is_ascii_alphabetic() && b.is_ascii_hexdigit()) {
                u8::from_str_radix(token, 16)
            } else {
                token.parse::<u8>()
            }
            .map_err(|_| "MIDI byte token must be a decimal or hexadecimal value in 0-255".to_string())?;
            push(output, value)?;
        }
    }
    Ok(())
}
fn push(output: &mut Vec<u8>, value: u8) -> Result<(), String> {
    if output.len() >= MAX_BYTES {
        return Err("MIDI byte output exceeds 64 KiB".into());
    }
    if output.len() == output.capacity() {
        output.reserve_exact(output.capacity().max(8).min(MAX_BYTES - output.len()));
    }
    output.push(value);
    Ok(())
}
pub(crate) fn normalize_midi_sysex(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let start = bytes.first() != Some(&0xF0);
    let end = bytes.last() != Some(&0xF7);
    let length = bytes.len().saturating_add(usize::from(start) + usize::from(end));
    if length > MAX_BYTES {
        return Err("MIDI SysEx output including F0/F7 exceeds 64 KiB".into());
    }
    let mut output = Vec::with_capacity(length);
    if start {
        output.push(0xF0);
    }
    output.extend_from_slice(bytes);
    if end {
        output.push(0xF7);
    }
    Ok(output)
}
