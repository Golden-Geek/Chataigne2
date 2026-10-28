//! Application identity wraps full project files without changing generic graph documents.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{ProjectDocument, ProjectDocumentCodecError, ProjectMetadata, validate_project_document_version};

/// Full application project file, distinct from reusable graph and subtree documents.
///
/// The nested graph keeps its own format version. The application envelope requires
/// an explicit identity; untagged files are not application projects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "M: Serialize + ProjectMetadata",
    deserialize = "M: Deserialize<'de> + Default"
))]
pub struct ApplicationProjectDocument<M = serde_json::Value> {
    /// Stable application identifier whose node codecs own this project.
    pub application_id: String,
    /// Generic project graph and optional project-owned UI state.
    #[serde(flatten)]
    pub project: ProjectDocument<M>,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "M: Deserialize<'de> + Default"))]
struct DecodedApplicationProjectDocument<M> {
    application_id: Option<String>,
    #[serde(flatten)]
    project: ProjectDocument<M>,
}

fn validate_application_id(found: &str, expected: &str) -> Result<(), ProjectDocumentCodecError> {
    if found == expected {
        Ok(())
    } else {
        Err(ProjectDocumentCodecError::ApplicationMismatch {
            found: found.to_owned(),
            expected: expected.to_owned(),
        })
    }
}

/// Decodes a full application project and rejects missing or foreign identity before use.
pub fn decode_application_project_document<M>(
    source: &str,
    expected_application_id: &str,
) -> Result<ApplicationProjectDocument<M>, ProjectDocumentCodecError>
where
    M: DeserializeOwned + Default,
{
    let decoded: DecodedApplicationProjectDocument<M> = serde_json::from_str(source)?;
    let application_id = decoded
        .application_id
        .ok_or(ProjectDocumentCodecError::MissingApplicationId)?;
    validate_application_id(&application_id, expected_application_id)?;
    validate_project_document_version(&decoded.project)?;
    Ok(ApplicationProjectDocument {
        application_id,
        project: decoded.project,
    })
}

/// Encodes a full application project only when its identity matches the caller's application.
pub fn encode_application_project_document<M>(
    document: &ApplicationProjectDocument<M>,
    expected_application_id: &str,
) -> Result<String, ProjectDocumentCodecError>
where
    M: Serialize + ProjectMetadata,
{
    validate_application_id(&document.application_id, expected_application_id)?;
    validate_project_document_version(&document.project)?;
    Ok(serde_json::to_string_pretty(document)?)
}
