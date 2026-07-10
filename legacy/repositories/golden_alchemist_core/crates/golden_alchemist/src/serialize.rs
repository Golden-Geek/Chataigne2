use crate::{ALCHEMIST_SCHEMA_VERSION, AlchemistGraph};

#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    #[error("failed to encode Alchemist graph: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("failed to decode Alchemist graph: {0}")]
    Decode(#[source] serde_json::Error),
    #[error("Alchemist graph schema {found} is newer than the supported schema {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
}

pub fn to_json_pretty(graph: &AlchemistGraph) -> Result<String, SerializationError> {
    serde_json::to_string_pretty(graph).map_err(SerializationError::Encode)
}

pub fn from_json(source: &str) -> Result<AlchemistGraph, SerializationError> {
    let graph: AlchemistGraph = serde_json::from_str(source).map_err(SerializationError::Decode)?;
    if graph.schema_version > ALCHEMIST_SCHEMA_VERSION {
        return Err(SerializationError::UnsupportedSchema {
            found: graph.schema_version,
            supported: ALCHEMIST_SCHEMA_VERSION,
        });
    }
    Ok(graph)
}
