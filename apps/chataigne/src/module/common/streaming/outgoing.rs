use super::module_helpers::format_bytes_for_log;

/// Format on the native owner only after a cursor confirms its effect. The fixed
/// prefix keeps feedback independent of the admitted payload size.
pub(crate) fn sent_log(transport: &str, bytes: &[u8]) -> String {
    format!(
        "Sent {transport} {} ({} bytes)",
        format_bytes_for_log(&bytes[..bytes.len().min(1024)]),
        bytes.len()
    )
}

pub(crate) fn broadcast_log(transport: &str, bytes: &[u8], confirmed: usize, original: usize) -> String {
    format!(
        "{} to {confirmed}/{original} original client(s)",
        sent_log(transport, bytes)
    )
}
