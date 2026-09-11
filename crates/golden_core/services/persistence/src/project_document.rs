//! Typed, app-agnostic Golden project document contract.

use std::fmt;

use golden_model::{NodeUuid, UserNodeRole};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Current Golden project document format version.
pub const PROJECT_FILE_VERSION: &str = "1.0";

fn default_project_file_version() -> String {
    PROJECT_FILE_VERSION.to_string()
}

fn is_default_user_node_role(value: &UserNodeRole) -> bool {
    *value == UserNodeRole::Regular
}

/// Metadata stored with each project node record.
///
/// Applications may extend the neutral hierarchy with a typed metadata record.
/// Keeping this contract in persistence lets codecs operate without importing
/// an engine, host, or desktop runtime.
pub trait ProjectMetadata {
    /// Returns whether the record can be omitted from the serialized document.
    fn is_empty(&self) -> bool;
}

impl ProjectMetadata for serde_json::Value {
    fn is_empty(&self) -> bool {
        self.is_null() || self.as_object().is_some_and(serde_json::Map::is_empty)
    }
}

/// Serialized project document containing one rooted node hierarchy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "M: Serialize + ProjectMetadata",
    deserialize = "M: Deserialize<'de> + Default"
))]
pub struct ProjectDocument<M = serde_json::Value> {
    /// File format version.
    #[serde(default = "default_project_file_version")]
    pub version: String,
    /// Optional project-owned UI state carried verbatim by hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_state: Option<serde_json::Value>,
    /// Root node record.
    pub root: ProjectNodeRecord<M>,
}

/// Serialized node record for full-snapshot persistence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "M: Serialize + ProjectMetadata",
    deserialize = "M: Deserialize<'de> + Default"
))]
pub struct ProjectNodeRecord<M = serde_json::Value> {
    /// Persistent identity.
    pub uuid: NodeUuid,
    /// Runtime node type identifier.
    #[serde(rename = "type")]
    pub node_type: String,
    /// User-facing curation role for this node.
    #[serde(default, skip_serializing_if = "is_default_user_node_role")]
    pub user_role: UserNodeRole,
    /// Persisted metadata fields.
    #[serde(default, skip_serializing_if = "ProjectMetadata::is_empty")]
    pub meta: M,
    /// Node-specific payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Ordered child records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ProjectNodeRecord<M>>,
}

/// Failure while validating or encoding the neutral project document.
#[derive(Debug)]
pub enum ProjectDocumentCodecError {
    /// The document version is not supported by this codec.
    UnsupportedVersion {
        /// Version read from the document.
        found: String,
        /// Version required by the current codec.
        expected: &'static str,
    },
    /// JSON decoding or encoding failed.
    Json(serde_json::Error),
}

impl fmt::Display for ProjectDocumentCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { found, expected } => {
                write!(
                    formatter,
                    "unsupported project version {found:?}; expected {expected:?}"
                )
            }
            Self::Json(error) => write!(formatter, "project document JSON error: {error}"),
        }
    }
}

impl std::error::Error for ProjectDocumentCodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::UnsupportedVersion { .. } => None,
        }
    }
}

impl From<serde_json::Error> for ProjectDocumentCodecError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Validates that a project document uses the current format version.
pub fn validate_project_document_version<M>(document: &ProjectDocument<M>) -> Result<(), ProjectDocumentCodecError> {
    if document.version == PROJECT_FILE_VERSION {
        return Ok(());
    }

    Err(ProjectDocumentCodecError::UnsupportedVersion {
        found: document.version.clone(),
        expected: PROJECT_FILE_VERSION,
    })
}

/// Decodes and version-validates a typed project document.
pub fn decode_project_document<M>(source: &str) -> Result<ProjectDocument<M>, ProjectDocumentCodecError>
where
    M: DeserializeOwned + Default,
{
    let document = serde_json::from_str(source)?;
    validate_project_document_version(&document)?;
    Ok(document)
}

/// Encodes a typed project document as stable, human-readable JSON.
pub fn encode_project_document<M>(document: &ProjectDocument<M>) -> Result<String, ProjectDocumentCodecError>
where
    M: Serialize + ProjectMetadata,
{
    validate_project_document_version(document)?;
    Ok(serde_json::to_string_pretty(document)?)
}

/// Encodes a typed project document as compact JSON.
pub fn encode_project_document_compact<M>(document: &ProjectDocument<M>) -> Result<String, ProjectDocumentCodecError>
where
    M: Serialize + ProjectMetadata,
{
    validate_project_document_version(document)?;
    Ok(serde_json::to_string(document)?)
}
