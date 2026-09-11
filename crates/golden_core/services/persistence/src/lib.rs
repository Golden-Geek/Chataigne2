//! Durable, app-agnostic file persistence primitives for Golden projects.

#![warn(missing_docs)]

mod file_store;
mod project_document;
mod save_coordinator;

pub use file_store::{
    RecoveryCandidates, RecoveryJournal, RecoveryPaths, clear_recovery_journal, read_recovery_candidates,
    restore_primary_from_backup, write_file_atomically_with_recovery,
};
pub use project_document::{
    PROJECT_FILE_VERSION, ProjectDocument, ProjectDocumentCodecError, ProjectMetadata, ProjectNodeRecord,
    decode_project_document, encode_project_document, encode_project_document_compact,
    validate_project_document_version,
};
pub use save_coordinator::{
    CoordinatedSave, DestinationIdentity, GenerationReplacementFence, PersistenceCoordinator,
    PersistenceCoordinatorError, SaveTicket, SaveTicketInfo, normalize_destination_identity,
};

#[cfg(test)]
mod tests;
