//! Shared bounded JSON lines for owned diagnostic children, not a product transport.

use crate::ProbeResult;
use std::io::{BufRead, Read, Write};

pub const MAX_LINE_BYTES: u64 = 4096;

pub fn read_message<T: serde::de::DeserializeOwned>(reader: &mut impl BufRead) -> ProbeResult<Option<T>> {
    let mut bytes = Vec::new();
    let count = reader.take(MAX_LINE_BYTES + 1).read_until(b'\n', &mut bytes)?;
    if count == 0 {
        return Ok(None);
    }
    if count as u64 > MAX_LINE_BYTES || bytes.last() != Some(&b'\n') {
        return Err("oversized or incomplete child protocol line".into());
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
}

pub fn line(value: &impl serde::Serialize) -> ProbeResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_LINE_BYTES {
        return Err("oversized outgoing child message".into());
    }
    Ok(bytes)
}

pub fn respond(value: &impl serde::Serialize) -> ProbeResult<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&line(value)?)?;
    stdout.flush()?;
    Ok(())
}
