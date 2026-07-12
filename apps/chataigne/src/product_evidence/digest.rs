use serde_json::Value;

const FNV_OFFSET_BASIS_64: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME_64: u64 = 0x0000_0100_0000_01b3;

pub(super) fn semantic_digest(value: &Value) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| format!("failed to encode canonical semantic evidence: {error}"))?;
    Ok(digest_bytes(&bytes))
}

pub(super) fn digest_bytes(bytes: &[u8]) -> String {
    let mut hash = FNV_OFFSET_BASIS_64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME_64);
    }
    format!("fnv1a64:{hash:016x}")
}
