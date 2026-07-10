//! Single-source multi-plane protocol and binary high-rate value frames.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::{Config, TS};

pub const PROTOCOL_VERSION: u16 = 1;
const VALUE_FRAME_MAGIC: [u8; 4] = *b"GVF1";
const VALUE_FRAME_HEADER_BYTES: usize = 4 + 2 + 4 + 4 + 4;
const VALUE_SAMPLE_BYTES: usize = 4 + 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ProtocolPlane {
    Control,
    Authoring,
    Observation,
    Values,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, TS)]
pub struct ClientId(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, TS)]
pub struct ViewId(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, TS)]
pub struct ScopeId(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, TS)]
pub struct PreviewKey {
    pub scope: ScopeId,
    pub entity: String,
    pub field: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[ts(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProtocolValue {
    Bool(bool),
    Integer(i32),
    Float(f64),
    String(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum ControlRequest {
    Ping { nonce: u32 },
    LoadProject { path: String },
    SaveProject { path: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum ControlResponse {
    Pong { nonce: u32 },
    Accepted { request_id: u32 },
    Rejected { code: String, message: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
pub struct AuthoringChange {
    pub entity: String,
    pub operation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
pub struct AuthoringEvent {
    pub revision: u32,
    pub changes: Vec<AuthoringChange>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
pub struct ObservationInterest {
    pub client: ClientId,
    pub view: ViewId,
    pub scopes: Vec<ScopeId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
pub struct CatalogEntry {
    pub id: String,
    pub label: String,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
pub struct CatalogSnapshot {
    pub revision: u32,
    pub entries: Vec<CatalogEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct PreviewChange {
    pub key: PreviewKey,
    pub value: ProtocolValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct PreviewDelta {
    pub sequence: u32,
    pub changes: Vec<PreviewChange>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
#[ts(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ObservationMessage {
    Catalog(CatalogSnapshot),
    Preview(PreviewDelta),
    ResyncRequired { scope: ScopeId, after_sequence: u32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "plane", content = "payload", rename_all = "snake_case")]
#[ts(tag = "plane", content = "payload", rename_all = "snake_case")]
pub enum ServerMessage {
    Control(ControlResponse),
    Authoring(AuthoringEvent),
    Observation(ObservationMessage),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueSample {
    pub slot: u32,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValueFrame {
    pub sequence: u32,
    pub generation: u32,
    pub samples: Vec<ValueSample>,
}

impl ValueFrame {
    pub fn encode(&self) -> Result<Vec<u8>, ValueFrameError> {
        let sample_count = u32::try_from(self.samples.len()).map_err(|_| ValueFrameError::TooManySamples)?;
        let mut bytes = Vec::with_capacity(VALUE_FRAME_HEADER_BYTES + self.samples.len() * VALUE_SAMPLE_BYTES);
        bytes.extend_from_slice(&VALUE_FRAME_MAGIC);
        bytes.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes.extend_from_slice(&self.generation.to_le_bytes());
        bytes.extend_from_slice(&sample_count.to_le_bytes());
        for sample in &self.samples {
            if !sample.value.is_finite() {
                return Err(ValueFrameError::NonFiniteValue);
            }
            bytes.extend_from_slice(&sample.slot.to_le_bytes());
            bytes.extend_from_slice(&sample.value.to_le_bytes());
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], maximum_samples: usize) -> Result<Self, ValueFrameError> {
        if bytes.len() < VALUE_FRAME_HEADER_BYTES {
            return Err(ValueFrameError::Truncated);
        }
        if bytes[..4] != VALUE_FRAME_MAGIC {
            return Err(ValueFrameError::Magic);
        }
        let version = u16::from_le_bytes(bytes[4..6].try_into().expect("fixed range"));
        if version != PROTOCOL_VERSION {
            return Err(ValueFrameError::Version(version));
        }
        let sequence = u32::from_le_bytes(bytes[6..10].try_into().expect("fixed range"));
        let generation = u32::from_le_bytes(bytes[10..14].try_into().expect("fixed range"));
        let sample_count = u32::from_le_bytes(bytes[14..18].try_into().expect("fixed range")) as usize;
        if sample_count > maximum_samples {
            return Err(ValueFrameError::SampleLimit {
                count: sample_count,
                maximum: maximum_samples,
            });
        }
        let expected = VALUE_FRAME_HEADER_BYTES
            .checked_add(
                sample_count
                    .checked_mul(VALUE_SAMPLE_BYTES)
                    .ok_or(ValueFrameError::TooManySamples)?,
            )
            .ok_or(ValueFrameError::TooManySamples)?;
        if bytes.len() != expected {
            return Err(ValueFrameError::Truncated);
        }
        let mut samples = Vec::with_capacity(sample_count);
        for chunk in bytes[18..].chunks_exact(VALUE_SAMPLE_BYTES) {
            let slot = u32::from_le_bytes(chunk[..4].try_into().expect("fixed range"));
            let value = f64::from_le_bytes(chunk[4..12].try_into().expect("fixed range"));
            if !value.is_finite() {
                return Err(ValueFrameError::NonFiniteValue);
            }
            samples.push(ValueSample { slot, value });
        }
        Ok(Self {
            sequence,
            generation,
            samples,
        })
    }
}

pub fn typescript_declarations() -> String {
    let config = Config::default();
    let declarations = [
        ProtocolPlane::decl(&config),
        ClientId::decl(&config),
        ViewId::decl(&config),
        ScopeId::decl(&config),
        PreviewKey::decl(&config),
        ProtocolValue::decl(&config),
        ControlRequest::decl(&config),
        ControlResponse::decl(&config),
        AuthoringChange::decl(&config),
        AuthoringEvent::decl(&config),
        ObservationInterest::decl(&config),
        CatalogEntry::decl(&config),
        CatalogSnapshot::decl(&config),
        PreviewChange::decl(&config),
        PreviewDelta::decl(&config),
        ObservationMessage::decl(&config),
        ServerMessage::decl(&config),
    ]
    .map(|declaration| format!("export {declaration}"));
    format!(
        "// Generated from golden-protocol. Do not edit.\n\nexport const PROTOCOL_VERSION = {PROTOCOL_VERSION} as const;\n\n{}\n",
        declarations.join("\n\n")
    )
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ValueFrameError {
    #[error("value frame is truncated or has trailing bytes")]
    Truncated,
    #[error("value frame magic is invalid")]
    Magic,
    #[error("value frame protocol version is unsupported: {0}")]
    Version(u16),
    #[error("value frame contains too many samples")]
    TooManySamples,
    #[error("value frame contains {count} samples, exceeding limit {maximum}")]
    SampleLimit { count: usize, maximum: usize },
    #[error("value frame contains a non-finite value")]
    NonFiniteValue,
}

#[cfg(test)]
mod tests;
