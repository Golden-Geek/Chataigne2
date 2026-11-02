//! Intrinsic construction policy; this is not an authored product setting.
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FolderRecord {
    schema_owned: bool,
}

pub(super) fn encode(schema_owned: bool) -> Result<serde_json::Value, String> {
    if !schema_owned {
        return Ok(serde_json::Value::Null);
    }
    serde_json::to_value(FolderRecord { schema_owned }).map_err(|error| error.to_string())
}

pub(super) fn decode(data: &serde_json::Value) -> Result<bool, String> {
    if data.is_null() {
        return Ok(false);
    }
    serde_json::from_value::<FolderRecord>(data.clone())
        .map(|record| record.schema_owned)
        .map_err(|error| error.to_string())
}
