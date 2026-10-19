use super::GraphEditError;
use crate::app::ProjectGeneration;
use crate::engine::EngineRuntimeError;
use golden_runtime::ControlError;

/// Failure at the production application boundary, retaining actor and domain causes.
#[derive(Debug)]
pub enum ApplicationError {
    /// Admission or completion failed before an authoritative receipt was available.
    Control(ControlError),
    /// The durable file exists, but actor admission/completion prevented metadata publication.
    SavePublication {
        /// Durable identity of the file that already exists.
        committed: CommittedSave,
        /// Actor failure that prevented metadata acknowledgement.
        source: ControlError,
    },
    /// The engine returned a rejected graph acknowledgement.
    Rejected(GraphEditError),
    /// An admitted runtime tick failed.
    Runtime(EngineRuntimeError),
    /// A domain operation returned its actionable diagnostic.
    Operation(String),
    /// Starting an owned runtime service failed.
    Io(std::io::Error),
}

/// Durable save identity retained when publication cannot acknowledge the committed file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommittedSave {
    /// Normalized path already committed to disk.
    pub path: String,
    /// Save coordinator request identifier.
    pub request_id: u64,
    /// Project epoch captured by this save.
    pub project_generation: ProjectGeneration,
    /// Authored revision written to disk.
    pub document_revision: u64,
}

impl ApplicationError {
    /// Returns the typed actor failure without inferring it from a diagnostic string.
    pub fn control_error(&self) -> Option<&ControlError> {
        match self {
            Self::Control(error) | Self::SavePublication { source: error, .. } => Some(error),
            _ => None,
        }
    }

    /// Returns the authoritative rejection when the operation reached the engine.
    pub fn graph_rejection(&self) -> Option<&GraphEditError> {
        match self {
            Self::Rejected(error) => Some(error),
            _ => None,
        }
    }
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Control(error) => error.fmt(f),
            Self::SavePublication { committed, source } => write!(
                f,
                "project file '{}' was committed (request {}, generation {}, revision {}), but metadata publication failed: {source}",
                committed.path,
                committed.request_id,
                committed.project_generation.get(),
                committed.document_revision
            ),
            Self::Rejected(error) => error.fmt(f),
            Self::Runtime(error) => error.fmt(f),
            Self::Operation(message) => f.write_str(message),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ApplicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Control(error) | Self::SavePublication { source: error, .. } => Some(error),
            Self::Rejected(error) => Some(error),
            Self::Runtime(error) => Some(error),
            Self::Operation(_) => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<ControlError> for ApplicationError {
    fn from(error: ControlError) -> Self {
        Self::Control(error)
    }
}
impl From<GraphEditError> for ApplicationError {
    fn from(error: GraphEditError) -> Self {
        Self::Rejected(error)
    }
}
impl From<EngineRuntimeError> for ApplicationError {
    fn from(error: EngineRuntimeError) -> Self {
        Self::Runtime(error)
    }
}
impl From<String> for ApplicationError {
    fn from(error: String) -> Self {
        Self::Operation(error)
    }
}
impl From<std::io::Error> for ApplicationError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
